#![allow(dead_code)]

pub mod fixed_gas_cost;
pub mod variable_gas_cost;

use std::cell::Cell;
use std::cell::RefCell;
use std::cell::Ref;

use patch::Patch;
use util::opcode::Opcode;
use inkwell::values::CallSiteValue;
use inkwell::values::IntValue;
use inkwell::module::Linkage::*;
use inkwell::IntPredicate;
use inkwell::values::FunctionValue;
use evmjit::compiler::runtime::RuntimeManager;
use evmjit::compiler::exceptions::ExceptionManager;
use self::fixed_gas_cost::FixedGasCostCalculator;
use self::variable_gas_cost::VariableGasCostCalculator;
use evmjit::compiler::intrinsics::LLVMIntrinsic;
use evmjit::compiler::intrinsics::LLVMIntrinsicManager;

use super::JITContext;

pub trait InstructionGasCost {
    fn count_fixed_instruction_cost(&mut self, inst_opcode: Opcode, exc_mgr: &ExceptionManager) ;


    // Variable cost methods

    fn count_exp_cost(&self, cost: IntValue, exc_mgr: &ExceptionManager);
    fn count_log_data_cost(&self, log_data_length: IntValue, exc_mgr: &ExceptionManager);
    fn count_sha3_data_cost(&self, sha3_data_length: IntValue, exc_mgr: &ExceptionManager);
    fn copy_data_cost(&self, copy_length: IntValue, exc_mgr: &ExceptionManager);
    //fn refund_gas(&self, gas_to_refund: IntValue);
}

pub struct GasCheckFunctionCreator {
    m_gas_func: FunctionValue
}

impl GasCheckFunctionCreator {
    pub fn new(name : &str, jitctx: &JITContext) -> GasCheckFunctionCreator {
        let context = jitctx.llvm_context();
        let module = jitctx.module();

        let types_instance = jitctx.evm_types();

        let gas_func_ret_type = context.void_type();

        // Set up gas check function arguments:
        // Arg1 is pointer to gas type (Pointer to current gas)
        // Arg2 is gas type (gas cost)
        // Arg3 is pointer type (buffer containing address to jump to in case we run out of gas

        let arg1 = types_instance.get_gas_ptr_type();
        let arg2 = types_instance.get_gas_type();
        let arg3 = types_instance.get_byte_ptr_type();

        let gas_func_type = gas_func_ret_type.fn_type(&[arg1.into(), arg2.into(), arg3.into()], false);
        let gas_func = module.add_function(name, gas_func_type, Some(Private));

        let attr_factory = jitctx.attributes();
        gas_func.add_attribute(0, *attr_factory.attr_nounwind());
        gas_func.add_attribute(1, *attr_factory.attr_nocapture());

        let check_bb = context.append_basic_block(&gas_func, "Check");
        let update_bb = context.append_basic_block(&gas_func, "Update");
        let out_of_gas_bb = context.append_basic_block(&gas_func, "OutOfGas");

        let temp_builder = context.create_builder();
        temp_builder.position_at_end(&check_bb);

        assert!(gas_func.get_nth_param(0) != None);
        let gas_ptr = gas_func.get_nth_param(0).unwrap();
        gas_ptr.into_pointer_value().set_name("gasPtr");

        assert!(gas_func.get_nth_param(1) != None);
        let gas_cost = gas_func.get_nth_param(1).unwrap();
        gas_cost.into_int_value().set_name("cost");

        assert!(gas_func.get_nth_param(2) != None);
        let jmp_buf = gas_func.get_nth_param(2).unwrap();
        jmp_buf.into_pointer_value().set_name("jmpBuf");

        // Create new builder for gas metering function

        let gas_func_builder = context.create_builder();
        gas_func_builder.position_at_end(&check_bb);

        let current_gas = gas_func_builder.build_load(gas_ptr.into_pointer_value(), "gas");
        let updated_gas:IntValue = gas_func_builder.build_int_nsw_sub(current_gas.into_int_value(), gas_cost.into_int_value(), "updatedGas");
        let zero_val64 = context.i64_type().const_zero();

        let gas_ok = gas_func_builder.build_int_compare(IntPredicate::SGE, updated_gas, zero_val64, "");
        gas_func_builder.build_conditional_branch(gas_ok, &update_bb, &out_of_gas_bb);

        gas_func_builder.position_at_end(&update_bb);
        gas_func_builder.build_store(gas_ptr.into_pointer_value(), updated_gas);
        gas_func_builder.build_return(None);

        gas_func_builder.position_at_end(&out_of_gas_bb);

        // If we enter this basic block, we ran out ouf gas.
        // Use longjmp to trigger an exception

        let func_decl = LLVMIntrinsic::LongJmp.get_intrinsic_declaration(jitctx,
                                                                         None);

        gas_func_builder.build_call (func_decl, &[jmp_buf.into_pointer_value().into()], "longJmp");

        gas_func_builder.build_unreachable();

        GasCheckFunctionCreator {
            m_gas_func: gas_func
        }
    }

    pub fn get_gas_check_func_decl(&self) -> FunctionValue {
        self.m_gas_func
    }
}

pub struct BasicBlockGasManager<'a, P: Patch> {
    m_context: &'a JITContext,
    m_runtime: &'a RuntimeManager<'a>,
    m_variable_cost: VariableGasCostCalculator<'a, P>,
    m_block_gas_cost: Cell<i64>,
    m_gas_check_call: RefCell<Option<CallSiteValue>>,
    m_gas_check_creator: GasCheckFunctionCreator
}


impl<'a, P: Patch> BasicBlockGasManager<'a, P> {
    pub fn new(jitctx: &'a JITContext,
               runtime: &'a RuntimeManager<'a>) -> BasicBlockGasManager<'a, P> {

        let variable_cost_calculator: VariableGasCostCalculator<P> = VariableGasCostCalculator::new(&jitctx);
        let func_creator = GasCheckFunctionCreator::new("gas.check", &jitctx);

        BasicBlockGasManager {
            m_context: jitctx,
            m_runtime: runtime,
            m_variable_cost: variable_cost_calculator,
            m_block_gas_cost: Cell::new(0),
            m_gas_check_call: RefCell::new(None),
            m_gas_check_creator: func_creator
        }
    }

    fn count_variable_cost(&self, cost: IntValue, exc_mgr: &ExceptionManager) {
        let types_instance = self.m_context.evm_types();
        let word_type = types_instance.get_word_type();

        let arg1 = *self.m_runtime.get_gas_ptr();
        let arg3 = exc_mgr.get_exception_dest();

        if cost.get_type() == word_type {
            let builder = self.m_context.builder();
            let const_factory = self.m_context.evm_constants();
            let gas_max = const_factory.get_gas_max();
            let gas_max_256 = builder.build_int_z_extend(gas_max, word_type, "");
            let too_high = builder.build_int_compare(IntPredicate::UGT, cost, gas_max_256, "");
            let cost64 = builder.build_int_truncate(cost, types_instance.get_gas_type(), "");
            let cost_to_use = builder.build_select(too_high, gas_max, cost64, "cost");


            let arg2 = cost_to_use;
            builder.build_call(self.m_gas_check_creator.get_gas_check_func_decl(), &[arg1.into(), arg2.into(), arg3.into()], "");
        }
        else {
            assert!(cost.get_type() == types_instance.get_gas_type());
            self.m_context.builder().build_call(self.m_gas_check_creator.get_gas_check_func_decl(), &[arg1.into(), cost.into(), arg3.into()], "");
        }
    }

    pub fn get_block_gas_cost(&self) -> i64 {
        self.m_block_gas_cost.get()
    }

    fn update_block_gas_cost(&self, cost: i64) {
        self.m_block_gas_cost.set(cost);
    }

    pub fn has_gas_check_call(&self) -> bool {
        self.get_gas_check_call().is_some()
    }

    fn get_gas_check_call(&self) -> Ref<Option<CallSiteValue>> {
        self.m_gas_check_call.borrow()
    }

    pub fn update_gas_check_call(&self, callsite: CallSiteValue) {
        *self.m_gas_check_call.borrow_mut() = Some(callsite);
    }

    pub fn reset_gas_check_call(&self) {
        *self.m_gas_check_call.borrow_mut() = None;
    }

    pub fn finalize_block_cost(&self) {
        if self.has_gas_check_call() {
            if self.get_block_gas_cost() == 0 {

                if let Some(ref mock_call_site) = *self.m_gas_check_call.borrow() {
                    let inst = mock_call_site.try_as_basic_value().right().unwrap();
                    assert!(inst.get_parent().is_some());
                    inst.erase_from_basic_block();
                }

                self.reset_gas_check_call();
            }
            else {
                let types_instance = self.m_context.evm_types();
                let val = types_instance.get_gas_type().const_int(self.get_block_gas_cost() as u64, false);

                // Update mocked gas check call with calculated gas of basic block

                if let Some(ref mut mock_call_site) = *self.m_gas_check_call.borrow_mut() {
                    mock_call_site.try_as_basic_value().right().unwrap().set_operand(1, val);
                }

                self.reset_gas_check_call();
                self.update_block_gas_cost(0);
            }
        }
    }
}

impl<'a, P: Patch> InstructionGasCost for BasicBlockGasManager<'a, P> {
    fn count_fixed_instruction_cost(&mut self, inst_opcode: Opcode, exc_mgr: &ExceptionManager) {
        // If we have not generated a call to the gas check function for this block do it now
        if self.has_gas_check_call() == false {
            let types_instance = self.m_context.evm_types();
            let builder = self.m_context.builder();

            let arg1 = *self.m_runtime.get_gas_ptr();
            let arg2 = types_instance.get_gas_type().get_undef();
            let arg3 = exc_mgr.get_exception_dest();

            let gas_call = builder.build_call(self.m_gas_check_creator.get_gas_check_func_decl(),
                                                            &[arg1.into(), arg2.into(), arg3.into()], "");
            self.update_gas_check_call(gas_call);
            assert!(self.has_gas_check_call());
        }

        let instruction_cost = FixedGasCostCalculator::<P>::gas_cost(inst_opcode);
        let current_block_cost = self.get_block_gas_cost();
        let new_block_cost = current_block_cost + instruction_cost as i64;
        self.update_block_gas_cost(new_block_cost as i64);
    }

    fn count_sha3_data_cost(&self, sha3_data_length: IntValue, exc_mgr: &ExceptionManager) {
        let cost = self.m_variable_cost.sha3_data_cost(sha3_data_length);
        self.count_variable_cost(cost, exc_mgr);
    }

    fn count_log_data_cost(&self, log_data_length: IntValue, exc_mgr: &ExceptionManager) {
        let cost = self.m_variable_cost.log_data_cost(log_data_length);
        self.count_variable_cost(cost, exc_mgr);
    }

    fn copy_data_cost(&self, copy_length: IntValue, exc_mgr: &ExceptionManager) {
        let cost = self.m_variable_cost.copy_data_cost(copy_length);
        self.count_variable_cost(cost, exc_mgr);
    }

    fn count_exp_cost(&self, exponent: IntValue, exc_mgr: &ExceptionManager) {
        let builder = self.m_context.builder();
        assert!(builder.get_insert_block() != None);
        let current_block = builder.get_insert_block().unwrap();
        let cost = self.m_variable_cost.exp_cost(&current_block, exponent);
        self.count_variable_cost(cost, exc_mgr);
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use evmjit::compiler::evm_compiler::MainFuncCreator;
    use inkwell::attributes::Attribute;
    use inkwell::module::Linkage;
    use std::ffi::CString;
    use inkwell::values::InstructionOpcode;
    use evmjit::{GetOperandValue, BasicTypeEnumCompare};
    use evmjit::GetOperandBasicBlock;
    use evmjit::compiler::external_declarations::ExternalFunctionManager;

    #[test]
    fn test_gas_check_func_creator() {
        let jitctx = JITContext::new();
        let context = jitctx.llvm_context();
        let module = jitctx.module();
        let decl_factory = ExternalFunctionManager::new(&jitctx);

        let attr_factory = jitctx.attributes();

        // Generate outline of main function needed by 'RuntimeTypeManager
        MainFuncCreator::new("main", &jitctx);

        //let manager = RuntimeManager::new("main", &context, &builder, &module);
        let _manager = RuntimeManager::new(&jitctx, &decl_factory);

        GasCheckFunctionCreator::new("gas.check", &jitctx);

        //module.print_to_stderr();

        let gas_check_fn_optional = module.get_function ("gas.check");
        assert!(gas_check_fn_optional != None);
        let gas_check_func = gas_check_fn_optional.unwrap();
        assert_eq!(gas_check_func.count_params(), 3);
        assert_eq!(gas_check_func.count_basic_blocks(), 3);
        assert_eq!(gas_check_func.get_linkage(), Linkage::Private);

        // Verify gas function has nounwind attribute
        assert_eq!(gas_check_func.count_attributes(0), 1);
        let nounwind_attr = gas_check_func.get_enum_attribute(0, Attribute::get_named_enum_kind_id("nounwind"));
        assert!(nounwind_attr != None);

        assert_eq!(gas_check_func.count_attributes(1), 1);
        // Verify first operand has nocapture attribute
        let nocapture_attr = gas_check_func.get_enum_attribute(1, Attribute::get_named_enum_kind_id("nocapture"));
        assert!(nocapture_attr != None);

        assert_eq!(nounwind_attr.unwrap(), *attr_factory.attr_nounwind());
        assert_eq!(nocapture_attr.unwrap(), *attr_factory.attr_nocapture());

        let entry_block_optional = gas_check_func.get_first_basic_block();
        assert!(entry_block_optional != None);
        let entry_block = entry_block_optional.unwrap();
        assert_eq!(*entry_block.get_name(), *CString::new("Check").unwrap());

        assert!(entry_block.get_first_instruction() != None);
        let first_insn = entry_block.get_first_instruction().unwrap();
        assert_eq!(first_insn.get_opcode(), InstructionOpcode::Load);

        let load_operand0 = first_insn.get_operand_value(0).unwrap();
        assert!(load_operand0.is_pointer_value());

        let load_operand0_ptr_elt_t = load_operand0.into_pointer_value().get_type().get_element_type();
        assert!(load_operand0_ptr_elt_t.is_int_type());
        assert!(load_operand0_ptr_elt_t.into_int_type().get_bit_width() == 64);
        assert!(first_insn.get_next_instruction() != None);
        let second_insn = first_insn.get_next_instruction().unwrap();
        assert_eq!(second_insn.get_opcode(), InstructionOpcode::Sub);
        assert_eq!(second_insn.get_num_operands(), 2);

        let sub_operand0 = second_insn.get_operand_value(0).unwrap();
        assert!(sub_operand0.is_int_value());
        assert!(sub_operand0.get_type().is_int64());

        // Verify that gas loaded from memory is first operand of subtract
        let gas_val_use = first_insn.get_first_use().unwrap().get_used_value().left().unwrap();
        assert_eq!(sub_operand0, gas_val_use);

        let sub_operand1 = second_insn.get_operand_value(1).unwrap();
        assert!(sub_operand1.is_int_value());
        assert!(sub_operand1.get_type().is_int64());

        // Verify that the second operand of the subtract is the cost
        let cost_arg = gas_check_func.get_nth_param(1).unwrap();
        assert_eq! (cost_arg, sub_operand1);

        assert!(second_insn.get_next_instruction() != None);
        let third_insn = second_insn.get_next_instruction().unwrap();
        assert_eq!(third_insn.get_opcode(), InstructionOpcode::ICmp);
        assert_eq!(third_insn.get_num_operands(), 2);

        let icmp_operand0 = third_insn.get_operand_value(0).unwrap();
        let updated_gas_val = second_insn.get_first_use().unwrap().get_used_value().left().unwrap();
        assert_eq!(icmp_operand0, updated_gas_val);

        let icmp_operand1 = third_insn.get_operand_value(1).unwrap();
        assert_eq!(icmp_operand1, context.i64_type().const_zero());

        assert!(third_insn.get_next_instruction() != None);
        let fourth_insn = third_insn.get_next_instruction().unwrap();
        assert_eq!(fourth_insn.get_opcode(), InstructionOpcode::Br);
        assert_eq!(fourth_insn.get_num_operands(), 3);

        let cond_br_operand0 = fourth_insn.get_operand_value(0).unwrap();
        let icmp_val_use = third_insn.get_first_use().unwrap().get_used_value().left().unwrap();
        assert_eq!(icmp_val_use, cond_br_operand0);
        //let cond_br_operand1 = fourth_insn.get_operand_as_bb(1).unwrap();
        assert!(entry_block.get_next_basic_block() != None);

        let update_block = entry_block.get_next_basic_block().unwrap();
        assert_eq!(*update_block.get_name(), *CString::new("Update").unwrap());

        let out_of_gas_block = update_block.get_next_basic_block().unwrap();
        assert_eq!(*out_of_gas_block.get_name(), *CString::new("OutOfGas").unwrap());

        // Why do the operands come back in the opposite order
        let cond_br_operand1 = fourth_insn.get_operand_as_bb(1).unwrap();
        assert_eq!(cond_br_operand1, out_of_gas_block);

        let cond_br_operand2 = fourth_insn.get_operand_as_bb(2).unwrap();
        assert_eq!(cond_br_operand2, update_block);

        assert!(fourth_insn.get_next_instruction().is_none());

        let first_update_insn = update_block.get_first_instruction().unwrap();
        assert_eq!(first_update_insn.get_opcode(), InstructionOpcode::Store);
        assert_eq!(first_update_insn.get_num_operands(), 2);
        let store_operand0 = first_update_insn.get_operand_value(0).unwrap();
        assert_eq!(store_operand0, updated_gas_val);

        // Verify that the second operand of the subtract is the cost
        let gas_ptr_arg = gas_check_func.get_nth_param(0).unwrap();
        let store_operand1 = first_update_insn.get_operand_value(1).unwrap();
        assert_eq!(store_operand1, gas_ptr_arg);

        assert!(first_update_insn.get_next_instruction() != None);
        let second_update_insn = first_update_insn.get_next_instruction().unwrap();
        assert_eq!(second_update_insn.get_opcode(), InstructionOpcode::Return);
        assert_eq!(second_update_insn.get_num_operands(), 0);
        assert!(second_update_insn.get_next_instruction().is_none());

        let first_out_of_gas_block_insn = out_of_gas_block.get_first_instruction().unwrap();
        assert_eq!(first_out_of_gas_block_insn.get_opcode(), InstructionOpcode::Call);
        assert_eq!(first_out_of_gas_block_insn.get_num_operands(), 2);

        let first_out_of_gas_insn_operand0 = first_out_of_gas_block_insn.get_operand_value(0).unwrap();
        assert!(first_out_of_gas_insn_operand0.is_pointer_value());
        let first_out_of_gas_insn_operand0_ptr_elt_t = first_out_of_gas_insn_operand0.into_pointer_value().get_type().get_element_type();
        assert!(first_out_of_gas_insn_operand0_ptr_elt_t.is_int_type());
        assert_eq!(first_out_of_gas_insn_operand0_ptr_elt_t.into_int_type(), context.i8_type());


        assert!(first_out_of_gas_block_insn.get_next_instruction().is_some());
        let second_out_of_gas_block_insn = first_out_of_gas_block_insn.get_next_instruction().unwrap();
        assert_eq!(second_out_of_gas_block_insn.get_opcode(), InstructionOpcode::Unreachable);

    }
}

