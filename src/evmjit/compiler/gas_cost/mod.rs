#![allow(dead_code)]

pub mod fixed_gas_cost;
pub mod variable_gas_cost;

use std::cell::Cell;
use std::cell::RefCell;
use std::cell::Ref;

use patch::Patch;
use util::opcode::Opcode;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::builder::Builder;
use inkwell::values::CallSiteValue;
use inkwell::values::IntValue;
use inkwell::module::Linkage::*;
use inkwell::IntPredicate;
use inkwell::values::FunctionValue;
use evmjit::LLVMAttributeFactory;
use evmjit::compiler::runtime::RuntimeManager;
use singletonum::Singleton;
use evmjit::compiler::evmtypes::EvmTypes;
use evmjit::compiler::evmconstants::EvmConstants;
use evmjit::compiler::exceptions::ExceptionManager;
use self::fixed_gas_cost::FixedGasCostCalculator;
use self::variable_gas_cost::VariableGasCostCalculator;

pub trait InstructionGasCost {
    fn count_fixed_instruction_cost(&self, inst_opcode: Opcode, exc_mgr: &ExceptionManager) ;


    // Variable cost methods

    fn count_exp_cost(&self, cost: IntValue, exc_mgr: &ExceptionManager);
    fn count_log_data_cost(&self, log_data_length: IntValue, exc_mgr: &ExceptionManager);
    fn count_sha3_data_cost(&self, sha3_data_length: IntValue, exc_mgr: &ExceptionManager);
    fn copy_data_cost(&self, copy_length: IntValue, exc_mgr: &ExceptionManager);
    //fn refund_gas(&self, gas_to_refund: IntValue);
}


pub trait BasicBlockGasCost: InstructionGasCost {
    fn finalize_block_cost(&self);
}


pub struct BasicBlockGasManager<'a, P: Patch> {
    m_context: &'a Context,
    m_builder: &'a Builder,
    m_runtime: &'a RuntimeManager<'a>,
    m_variable_cost: VariableGasCostCalculator<'a, P>,
    m_block_gas_cost: Cell<i64>,
    m_gas_check_call: RefCell<Option<CallSiteValue>>,
    m_gas_func: FunctionValue
}


impl<'a, P: Patch> BasicBlockGasManager<'a, P> {
    pub fn new(context: &'a Context, builder: &'a Builder, module: &'a Module,
               runtime: &'a RuntimeManager<'a>) -> BasicBlockGasManager<'a, P> {

        let variable_cost_calculator: VariableGasCostCalculator<P> = VariableGasCostCalculator::new(&context, &builder, &module);
        let types_instance = EvmTypes::get_instance(context);

        let gas_func_ret_type = context.void_type();

        // Set up gas check function arguments:
        // Arg1 is pointer to gas type
        // Arg2 is gas type
        // Arg3 is pointer type (buffer containing address to jump to in case we run out of gas

        let arg1 = types_instance.get_gas_ptr_type();
        let arg2 = types_instance.get_gas_type();
        let arg3 = types_instance.get_byte_ptr_type();

        let gas_func_type = gas_func_ret_type.fn_type(&[arg1.into(), arg2.into(), arg3.into()], false);
        let gas_func = module.add_function("gas.check", gas_func_type, Some(Internal));

        let attr_factory = LLVMAttributeFactory::get_instance(&context);
        gas_func.add_attribute(0, *attr_factory.attr_nounwind());
        gas_func.add_attribute(0, *attr_factory.attr_nocapture());

        let check_bb = context.append_basic_block(&gas_func, "Check");
        let update_bb = context.append_basic_block(&gas_func, "Update");
        let out_of_gas_bb = context.append_basic_block(&gas_func, "OutOfGas");

        let temp_builder = context.create_builder();
        temp_builder.position_at_end(&check_bb);

        let gas_ptr = gas_func.get_nth_param(0).unwrap();
        gas_ptr.into_pointer_value().set_name("gasPtr");

        let gas_cost = gas_func.get_nth_param(1).unwrap();
        gas_cost.into_pointer_value().set_name("cost");

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
        runtime.abort(jmp_buf.into_pointer_value());
        gas_func_builder.build_unreachable();

        BasicBlockGasManager {
            m_context: context,
            m_builder: builder,
            m_runtime: runtime,
            m_variable_cost: variable_cost_calculator,
            m_block_gas_cost: Cell::new(0),
            m_gas_check_call: RefCell::new(None),
            m_gas_func: gas_func
        }
    }

    fn count_variable_cost(&self, cost: IntValue, exc_mgr: &ExceptionManager) {
        let types_instance = EvmTypes::get_instance(self.m_context);
        let word_type = types_instance.get_word_type();

        let arg1 = *self.m_runtime.get_gas_ptr();
        let arg3 = exc_mgr.get_exception_dest();

        if cost.get_type() == word_type {
            let const_factory = EvmConstants::get_instance(self.m_context);
            let gas_max = const_factory.get_gas_max();
            let gas_max_256 = self.m_builder.build_int_z_extend(gas_max, word_type, "");
            let too_high = self.m_builder.build_int_compare(IntPredicate::UGT, cost, gas_max_256, "");
            let cost64 = self.m_builder.build_int_truncate(cost, types_instance.get_gas_type(), "");
            let cost_to_use = self.m_builder.build_select(too_high, gas_max, cost64, "cost");


            let arg2 = cost_to_use;
            self.m_builder.build_call(self.m_gas_func, &[arg1.into(), arg2.into(), arg3.into()], "");
        }
        else {
            assert!(cost.get_type() == types_instance.get_gas_type());
            self.m_builder.build_call(self.m_gas_func, &[arg1.into(), cost.into(), arg3.into()], "");
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

    pub fn update_gas_check_call(&mut self, callsite: CallSiteValue) {
        *self.m_gas_check_call.borrow_mut() = Some(callsite);
    }

    pub fn reset_gas_check_call(&mut self) {
        *self.m_gas_check_call.borrow_mut() = None;
    }
}

impl<'a, P: Patch> InstructionGasCost for BasicBlockGasManager<'a, P> {
    fn count_fixed_instruction_cost(&mut self, inst_opcode: Opcode, exc_mgr: &ExceptionManager) {
        // If we have not generated a call to the gas check function for this block do it now
        if self.has_gas_check_call() == false {
            let types_instance = EvmTypes::get_instance(self.m_context);

            let arg1 = *self.m_runtime.get_gas_ptr();
            let arg2 = types_instance.get_gas_type().get_undef();
            let arg3 = exc_mgr.get_exception_dest();

            let gas_call = self.m_builder.build_call(self.m_gas_func, &[arg1.into(), arg2.into(), arg3.into()], "");
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
        // pub fn exp_cost(&self, current_block: &BasicBlock, exponent: IntValue)
        assert!(self.m_builder.get_insert_block() != None);
        let current_block = self.m_builder.get_insert_block().unwrap();
        let cost = self.m_variable_cost.exp_cost(&current_block, exponent);
        self.count_variable_cost(cost, exc_mgr);
    }

}

impl<'a, P: Patch> BasicBlockGasCost for BasicBlockGasManager<'a, P> {
    fn finalize_block_cost(&mut self) {
        if self.has_gas_check_call() {
            if self.get_block_gas_cost() == 0 {
                let inst = self.m_gas_check_call.borrow().unwrap().try_as_basic_value().right().unwrap();
                assert!(inst.get_parent().is_some());
                inst.erase_from_basic_block();
            }
            else {
                self.reset_gas_check_call();
                self.update_block_gas_cost(0);
            }
        }
    }
}

//impl InstructionGasCost for BasicBlockManager
