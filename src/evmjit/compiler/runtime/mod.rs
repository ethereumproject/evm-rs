#![allow(dead_code)]

pub mod env;
pub mod txctx;
pub mod stack_init;
pub mod rt_data_type;
pub mod rt_type;

use inkwell::context::Context;
use inkwell::builder::Builder;
use inkwell::module::Module;
use inkwell::types::StructType;
use inkwell::types::PointerType;
use inkwell::values::BasicValueEnum;
use inkwell::values::PointerValue;
use inkwell::values::FunctionValue;
use inkwell::basic_block::BasicBlock;
use inkwell::module::Linkage::*;
use singletonum::Singleton;
use self::rt_data_type::RuntimeDataType;
use self::rt_type::RuntimeType;
use self::rt_type::RuntimeTypeManager;
use self::rt_data_type::RuntimeDataTypeFields::Gas;
use self::rt_data_type::RuntimeDataFieldToIndex;
use self::txctx::TransactionContextManager;
use self::stack_init::StackAllocator;
use evmjit::compiler::evmtypes::EvmTypes;
use evmjit::compiler::evmconstants::EvmConstants;
use llvm_sys::LLVMCallConv::*;
use evmjit::ModuleLookup;
use evmjit::LLVMAttributeFactory;

#[derive(PartialEq)]
pub enum TransactionContextTypeFields {
    GasPrice,
    Origin,
    CoinBase,
    Number,
    TimeStamp,
    GasLimit,
    Difficulty
}

trait TransactionContextTypeFieldToIndex {
    fn to_index(&self) -> usize;
}

impl TransactionContextTypeFieldToIndex for TransactionContextTypeFields {
    fn to_index(&self) -> usize {
        match self {
            TransactionContextTypeFields::GasPrice => 0,
            TransactionContextTypeFields::Origin => 1,
            TransactionContextTypeFields::CoinBase => 2,
            TransactionContextTypeFields::Number => 3,
            TransactionContextTypeFields::TimeStamp => 4,
            TransactionContextTypeFields::GasLimit => 5,
            TransactionContextTypeFields::Difficulty => 6,
        }
    }
}

struct GasPtrManager<'a> {
    m_gas_ptr: PointerValue,
    m_builder: &'a Builder,
}

impl<'a> GasPtrManager<'a> {
    pub fn new(context: &Context, builder: &'a Builder, gas_value: BasicValueEnum) -> GasPtrManager<'a> {
        let types_instance = EvmTypes::get_instance(context);
        let gas_p = builder.build_alloca(types_instance.get_gas_type(), "gas.ptr");
        builder.build_store(gas_p, gas_value);

        GasPtrManager {
            m_gas_ptr: gas_p,
            m_builder: builder
        }
    }

    pub fn get_gas_ptr(&self) -> &PointerValue {
        &self.m_gas_ptr
    }

    pub fn get_gas(&self) -> BasicValueEnum {
        self.m_builder.build_load(*self.get_gas_ptr(), "gas")
    }
}

#[derive(Debug, Copy, Clone)]
struct ReturnBufferManager<'a> {
    m_return_buf_data_ptr: PointerValue,
    m_return_buf_size_ptr: PointerValue,
    m_context: &'a Context,
    m_builder: &'a Builder,
}

impl<'a> ReturnBufferManager<'a> {
    pub fn new(context: &'a Context, builder: &'a Builder) -> ReturnBufferManager<'a> {
        let types_instance = EvmTypes::get_instance(context);
        let return_buf_data_p = builder.build_alloca(types_instance.get_byte_ptr_type(), "returndata.ptr");
        let return_buf_size_p = builder.build_alloca(types_instance.get_size_type(), "returndatasize.ptr");

        ReturnBufferManager {
            m_return_buf_data_ptr: return_buf_data_p,
            m_return_buf_size_ptr: return_buf_size_p,
            m_context: context,
            m_builder: builder
        }
    }

    pub fn get_return_buf_data_p(&self) -> &PointerValue {
        &self.m_return_buf_data_ptr
    }

    pub fn get_return_buf_size_p(&self) -> &PointerValue {
        &self.m_return_buf_size_ptr
    }

    pub fn reset_return_buf(&self) {
        let const_factory = EvmConstants::get_instance(self.m_context);
        self.m_builder.build_store(self.m_return_buf_size_ptr, const_factory.get_i64_zero());
    }
}

struct MainPrologue {
    m_exit_bb: BasicBlock,
}

impl MainPrologue {
    pub fn new(context: &Context, module: &Module,
               rt_type_mgr: &RuntimeTypeManager, gas_mgr: &GasPtrManager,
               main_func: FunctionValue, stack_base: BasicValueEnum) -> MainPrologue {
        let exit_bb = context.append_basic_block(&main_func, "Exit");
        let temp_builder = context.create_builder();
        temp_builder.position_at_end(&exit_bb);

        let types_instance = EvmTypes::get_instance(context);
        let phi = temp_builder.build_phi(types_instance.get_contract_return_type(), "ret");
        let free_func_opt = module.get_function("free");

        let free_func: FunctionValue;

        if free_func_opt == None {
            let free_ret_type = context.void_type();
            let arg1 = types_instance.get_word_ptr_type();
            let free_func_type = free_ret_type.fn_type(&[arg1.into()], false);
            free_func = module.add_function("free", free_func_type, Some(External));

            let attr_factory = LLVMAttributeFactory::get_instance(&context);
            free_func.add_attribute(0, *attr_factory.attr_nounwind());
            free_func.add_attribute(0, *attr_factory.attr_nocapture());
        } else {
            free_func = free_func_opt.unwrap();
        }

        temp_builder.build_call(free_func, &[stack_base.into()], "");
        let index = Gas.to_index() as u32;
        unsafe {
            let ext_gas_ptr = temp_builder.build_struct_gep(rt_type_mgr.get_data_ptr().into_pointer_value(),
                                                            index, "msg.gas.ptr");
            temp_builder.build_store(ext_gas_ptr, gas_mgr.get_gas());
            temp_builder.build_return(Some(&phi.as_basic_value()));
        }

        MainPrologue {
            m_exit_bb: exit_bb
        }
    }

    pub fn get_exit_bb(&self) -> &BasicBlock {
        &self.m_exit_bb
    }
}

pub struct RuntimeManager<'a> {
    m_context: &'a Context,
    m_builder: &'a Builder,
    m_module: &'a Module,
    m_txctx_manager:  TransactionContextManager<'a>,
    m_rt_type_manager: RuntimeTypeManager<'a>,
    m_stack_allocator: StackAllocator,
    m_gas_ptr_manager: GasPtrManager<'a>,
    m_return_buf_manager: ReturnBufferManager<'a>,
    m_prologue_manager: MainPrologue,
}

impl<'a> RuntimeManager<'a> {
    pub fn new(context: &'a Context, builder: &'a Builder, module: &'a Module) -> RuntimeManager<'a> {

        let main_func_opt = module.get_main_function(builder);
        assert!(main_func_opt != None);

        // Generate IR for transaction context related items
        let txctx_manager = TransactionContextManager::new (&context, &builder, &module);

        // Generate IR for runtime type related items
        let rt_type_manager = RuntimeTypeManager::new (&context, &builder, &module);

        let stack_allocator = StackAllocator::new (&context, &builder, &module);

        let gas_ptr_mgr = GasPtrManager::new(context, builder, rt_type_manager.get_gas());

        let return_buf_mgr = ReturnBufferManager::new(context, builder);
        return_buf_mgr.reset_return_buf();

        let prologue_manager = MainPrologue::new(context, module, &rt_type_manager, &gas_ptr_mgr,
                                                 main_func_opt.unwrap(), stack_allocator.get_stack_base_as_ir_value());

        RuntimeManager {
            m_context: context,
            m_builder: builder,
            m_module: module,
            m_txctx_manager: txctx_manager,
            m_rt_type_manager: rt_type_manager,
            m_stack_allocator: stack_allocator,
            m_gas_ptr_manager: gas_ptr_mgr,
            m_return_buf_manager: return_buf_mgr,
            m_prologue_manager: prologue_manager,
        }
    }

    pub fn gen_tx_ctx_item_ir(&self, field : TransactionContextTypeFields) -> BasicValueEnum {
        let call = self.m_builder.build_call (self.m_txctx_manager.get_tx_ctx_fn_ssa_var(),
                                              &[self.m_txctx_manager.get_tx_ctx_loaded_ssa_var().into(),
                                                self.m_txctx_manager.get_tx_ctx_ssa_var().into(),
                                                self.m_rt_type_manager.get_env_ptr().into()], "");
        call.set_call_convention(LLVMFastCallConv as u32);
        let index = field.to_index();

        unsafe {
            let mut ptr = self.m_builder.build_struct_gep(self.m_txctx_manager.get_tx_ctx_ssa_var(),
                                                          index as u32, "");

            // Origin and Coinbase are declared as arrays of 20 bytes (160 bits) to deal with alignment issues
            // Cast back to i160 pointer here

            if field ==  TransactionContextTypeFields::Origin || field == TransactionContextTypeFields::CoinBase {
                let types_instance = EvmTypes::get_instance(self.m_context);
                ptr = self.m_builder.build_pointer_cast (ptr, types_instance.get_address_ptr_type(), "");
            }

            self.m_builder.build_load(ptr, "")
        }
    }

    pub fn get_runtime_data_type(&self) -> StructType {
        RuntimeDataType::get_instance(self.m_context).get_type()
    }

    pub fn get_runtime_type(&self) -> StructType {
        RuntimeType::get_instance(self.m_context).get_type()
    }

    pub fn get_runtime_ptr_type(&self) -> PointerType {
        RuntimeType::get_instance(self.m_context).get_ptr_type()
    }

    pub fn get_runtime_ptr(&self) -> BasicValueEnum {
        self.m_rt_type_manager.get_runtime_ptr()
    }

    pub fn get_data_ptr(&self) -> BasicValueEnum {
        self.m_rt_type_manager.get_data_ptr()
    }

    pub fn get_gas_ptr(&self) -> &PointerValue {
        assert!(self.m_module.get_main_function(self.m_builder) != None);
        self.m_gas_ptr_manager.get_gas_ptr()
    }

    pub fn get_gas(&self) -> BasicValueEnum {
        self.m_gas_ptr_manager.get_gas()
    }

    pub fn get_return_buf_data_p(&self) -> &PointerValue {
        self.m_return_buf_manager.get_return_buf_data_p()
    }

    pub fn get_return_buf_size_p(&self) -> &PointerValue {
        self.m_return_buf_manager.get_return_buf_size_p()
    }

    pub fn reset_return_buf(self) {
        self.m_return_buf_manager.reset_return_buf()
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;
    use super::*;
    use inkwell::values::InstructionOpcode;
    use self::txctx::TransactionContextType;
    use self::env::EnvDataType;
    use evmjit::compiler::evm_compiler::MainFuncCreator;
    use evmjit::GetOperandValue;

    #[test]
    fn test_data_field_to_index() {
        assert_eq!(TransactionContextTypeFields::GasPrice.to_index(), 0);
        assert_eq!(TransactionContextTypeFields::Origin.to_index(), 1);
        assert_eq!(TransactionContextTypeFields::CoinBase.to_index(), 2);
        assert_eq!(TransactionContextTypeFields::Number.to_index(), 3);
        assert_eq!(TransactionContextTypeFields::TimeStamp.to_index(), 4);
        assert_eq!(TransactionContextTypeFields::GasLimit.to_index(), 5);
        assert_eq!(TransactionContextTypeFields::Difficulty.to_index(), 6);
    }

    #[test]
    fn test_runtime_manager() {
        let context = Context::create();
        let module = context.create_module("my_module");
        let builder = context.create_builder();

        // Generate outline of main function needed by 'RuntimeTypeManager
        MainFuncCreator::new ("main", &context, &builder, &module);

        //let manager = RuntimeManager::new("main", &context, &builder, &module);
        let manager = RuntimeManager::new(&context, &builder, &module);

        assert!(RuntimeDataType::is_rt_data_type(&manager.get_runtime_data_type()));
        assert!(RuntimeType::is_runtime_type(&manager.get_runtime_type()));

        let rt_ptr = manager.get_runtime_ptr_type();
        assert!(rt_ptr.get_element_type().is_struct_type());
        assert!(RuntimeType::is_runtime_type(rt_ptr.get_element_type().as_struct_type()));
    }

    #[test]
    fn test_gas_ptr_manager() {
        let context = Context::create();
        let module = context.create_module("my_module");
        let builder = context.create_builder();

        // Generate outline of main function needed by 'RuntimeTypeManager
        MainFuncCreator::new ("main", &context, &builder, &module);

        // Generate IR for runtime type related items
        let rt_type_manager = RuntimeTypeManager::new (&context, &builder, &module);

        // Create dummy function

        let main_fn_optional = module.get_function ("main");
        assert!(main_fn_optional != None);

        let main_fn = main_fn_optional.unwrap();
        let gas_bb = context.append_basic_block(&main_fn, "gas_ptr_bb");

        builder.position_at_end(&gas_bb);

        GasPtrManager::new(&context, &builder, rt_type_manager.get_gas());

        assert!(gas_bb.get_first_instruction() != None);
        let first_insn = gas_bb.get_first_instruction().unwrap();
        assert_eq!(first_insn.get_opcode(), InstructionOpcode::Alloca);

        assert!(first_insn.get_next_instruction() != None);
        let second_insn = first_insn.get_next_instruction().unwrap();
        assert_eq!(second_insn.get_opcode(), InstructionOpcode::Store);
        assert_eq!(second_insn.get_num_operands(), 2);

        let store_operand0 = second_insn.get_operand_value(0).unwrap();
        assert!(store_operand0.is_int_value());
        assert_eq!(store_operand0.get_type().as_int_type().get_bit_width(), 64);

        let store_operand1 = second_insn.get_operand_value(1).unwrap();
        assert!(store_operand1.is_pointer_value());
        let store_operand1_ptr_elt_t = store_operand1.into_pointer_value().get_type().get_element_type();

        assert!(store_operand1_ptr_elt_t.is_int_type());
        assert_eq!(store_operand1_ptr_elt_t.as_int_type().get_bit_width(), 64);

        assert!(second_insn.get_next_instruction() == None);
    }


    #[test]
    fn test_return_buffer_manager() {
        let context = Context::create();
        let module = context.create_module("my_module");
        let builder = context.create_builder();

        // Create dummy function

        let fn_type = context.void_type().fn_type(&[], false);
        let my_fn = module.add_function("my_fn", fn_type, Some(External));
        let entry_bb = context.append_basic_block(&my_fn, "entry");
        builder.position_at_end(&entry_bb);

        let return_buf_mgr = ReturnBufferManager::new(&context, &builder);
        return_buf_mgr.reset_return_buf();

        let entry_block_optional = my_fn.get_first_basic_block();
        assert!(entry_block_optional != None);
        let entry_block = entry_block_optional.unwrap();
        assert_eq!(*entry_block.get_name(), *CString::new("entry").unwrap());

        assert!(entry_block.get_first_instruction() != None);
        let first_insn = entry_block.get_first_instruction().unwrap();
        assert_eq!(first_insn.get_opcode(), InstructionOpcode::Alloca);

        assert!(first_insn.get_next_instruction() != None);
        let second_insn = first_insn.get_next_instruction().unwrap();
        assert_eq!(second_insn.get_opcode(), InstructionOpcode::Alloca);
        assert!(second_insn.get_first_use() != None);

        assert!(second_insn.get_next_instruction() != None);
        let third_insn = second_insn.get_next_instruction().unwrap();
        assert_eq!(third_insn.get_opcode(), InstructionOpcode::Store);

        let store_operand0 = third_insn.get_operand_value(0).unwrap();
        assert!(store_operand0.is_int_value());
        let store_operand0_value = store_operand0.into_int_value();
        assert_eq!(store_operand0_value, context.i64_type().const_int(0, false));

        // Make sure we are storing a zero into the alloca area
        let store_operand_use1 = third_insn.get_operand_use(1).unwrap();
        assert_eq!(second_insn.get_first_use().unwrap(), store_operand_use1);
    }

    #[test]
    fn test_get_tx_ctx_item_gasprice() {
        let context = Context::create();
        let module = context.create_module("my_module");
        let builder = context.create_builder();

        // Need to create main function before TransactionConextManager otherwise we will crash
        MainFuncCreator::new ("main", &context, &builder, &module);

        let manager = RuntimeManager::new(&context, &builder, &module);

        let main_fn_optional = module.get_function ("main");
        assert!(main_fn_optional != None);

        let main_fn = main_fn_optional.unwrap();
        let gas_bb = context.append_basic_block(&main_fn, "gasprice");

        builder.position_at_end(&gas_bb);

        // This call will generate some ir code for us to test
        manager.gen_tx_ctx_item_ir(TransactionContextTypeFields::GasPrice);

        assert!(gas_bb.get_first_instruction() != None);
        let first_insn = gas_bb.get_first_instruction().unwrap();
        assert_eq!(first_insn.get_opcode(), InstructionOpcode::Call);
        assert_eq!(first_insn.get_num_operands(), 4);

        let call_operand0 = first_insn.get_operand_value(0).unwrap();

        assert!(call_operand0.is_pointer_value());   // should be i1 *
        let call_operand0_ptr_elt_t = call_operand0.into_pointer_value().get_type().get_element_type();
        assert!(call_operand0_ptr_elt_t.is_int_type());
        assert!(call_operand0_ptr_elt_t.into_int_type().get_bit_width() == 1);

        let call_operand1 = first_insn.get_operand_value(1).unwrap();

        assert!(call_operand1.is_pointer_value());   // should be evm.txctx *

        let call_operand1_ptr_elt_t = call_operand1.into_pointer_value().get_type().get_element_type();
        assert!(call_operand1_ptr_elt_t.is_struct_type());
        assert!(TransactionContextType::is_transaction_context_type(&call_operand1_ptr_elt_t.as_struct_type()));

        let call_operand2 = first_insn.get_operand_value(2).unwrap();
        assert!(call_operand2.is_pointer_value());   // should be Env *

        let call_operand2_ptr_elt_t = call_operand2.into_pointer_value().get_type().get_element_type();
        assert!(call_operand2_ptr_elt_t.is_struct_type());

        assert!(EnvDataType::is_env_data_type(&call_operand2_ptr_elt_t.as_struct_type()));

        assert!(first_insn.get_next_instruction() != None);
        let second_insn = first_insn.get_next_instruction().unwrap();
        assert_eq!(second_insn.get_opcode(), InstructionOpcode::GetElementPtr);

        assert_eq!(second_insn.get_num_operands(), 3);

        let gep_operand0 = second_insn.get_operand_value(0).unwrap();
        assert!(gep_operand0.is_pointer_value());
        let gep_operand0_ptr_elt_t = gep_operand0.into_pointer_value().get_type().get_element_type();
        assert!(gep_operand0_ptr_elt_t.is_struct_type());
        let gep_operand0_type = gep_operand0_ptr_elt_t.into_struct_type();
        assert!(TransactionContextType::is_transaction_context_type(&gep_operand0_type));

        let gep_operand1 = second_insn.get_operand_value(1).unwrap();
        assert!(gep_operand1.is_int_value());
        assert_eq!(gep_operand1.into_int_value(), context.i32_type().const_int(0, false));

        let gep_operand2 = second_insn.get_operand_value(2).unwrap();
        assert!(gep_operand2.is_int_value());
        assert_eq!(gep_operand2.into_int_value(), context.i32_type().const_int(0, false));

        assert!(second_insn.get_next_instruction() != None);
        let third_insn = second_insn.get_next_instruction().unwrap();
        assert_eq!(third_insn.get_opcode(), InstructionOpcode::Load);

        let load_operand0 = third_insn.get_operand_value(0).unwrap();
        assert!(load_operand0.is_pointer_value());

        let load_operand0_ptr_elt_t = load_operand0.into_pointer_value().get_type().get_element_type();
        assert!(load_operand0_ptr_elt_t.is_int_type());
        assert!(load_operand0_ptr_elt_t.into_int_type().get_bit_width() == 256);

        assert!(third_insn.get_next_instruction() == None);
    }

    #[test]
    fn test_get_tx_ctx_item_origin() {
        let context = Context::create();
        let module = context.create_module("my_module");
        let builder = context.create_builder();

        // Need to create main function before TransactionConextManager otherwise we will crash
        MainFuncCreator::new ("main", &context, &builder, &module);
        let manager = RuntimeManager::new(&context, &builder, &module);

        let main_fn_optional = module.get_function ("main");
        assert!(main_fn_optional != None);

        let main_fn = main_fn_optional.unwrap();
        let origin_bb = context.append_basic_block(&main_fn, "Origin");

        builder.position_at_end(&origin_bb);

        // This call will generate some ir code for us to test
        manager.gen_tx_ctx_item_ir(TransactionContextTypeFields::Origin);

        assert!(origin_bb.get_first_instruction() != None);
        let first_insn = origin_bb.get_first_instruction().unwrap();
        assert_eq!(first_insn.get_opcode(), InstructionOpcode::Call);
        assert_eq!(first_insn.get_num_operands(), 4);

        let call_operand0 = first_insn.get_operand_value(0).unwrap();

        assert!(call_operand0.is_pointer_value());   // should be i1 *
        let call_operand0_ptr_elt_t = call_operand0.into_pointer_value().get_type().get_element_type();
        assert!(call_operand0_ptr_elt_t.is_int_type());
        assert!(call_operand0_ptr_elt_t.into_int_type().get_bit_width() == 1);

        let call_operand1 = first_insn.get_operand_value(1).unwrap();

        assert!(call_operand1.is_pointer_value());   // should be evm.txctx *

        let call_operand1_ptr_elt_t = call_operand1.into_pointer_value().get_type().get_element_type();
        assert!(call_operand1_ptr_elt_t.is_struct_type());
        assert!(TransactionContextType::is_transaction_context_type(&call_operand1_ptr_elt_t.as_struct_type()));

        let call_operand2 = first_insn.get_operand_value(2).unwrap();
        assert!(call_operand2.is_pointer_value());   // should be Env *

        let call_operand2_ptr_elt_t = call_operand2.into_pointer_value().get_type().get_element_type();
        assert!(call_operand2_ptr_elt_t.is_struct_type());

        assert!(EnvDataType::is_env_data_type(&call_operand2_ptr_elt_t.as_struct_type()));

        assert!(first_insn.get_next_instruction() != None);
        let second_insn = first_insn.get_next_instruction().unwrap();
        assert_eq!(second_insn.get_opcode(), InstructionOpcode::GetElementPtr);

        assert_eq!(second_insn.get_num_operands(), 3);

        let gep_operand0 = second_insn.get_operand_value(0).unwrap();
        assert!(gep_operand0.is_pointer_value());
        let gep_operand0_ptr_elt_t = gep_operand0.into_pointer_value().get_type().get_element_type();
        assert!(gep_operand0_ptr_elt_t.is_struct_type());
        let gep_operand0_type = gep_operand0_ptr_elt_t.into_struct_type();
        assert!(TransactionContextType::is_transaction_context_type(&gep_operand0_type));

        let gep_operand1 = second_insn.get_operand_value(1).unwrap();
        assert!(gep_operand1.is_int_value());
        assert_eq!(gep_operand1.into_int_value(), context.i32_type().const_int(0, false));

        let gep_operand2 = second_insn.get_operand_value(2).unwrap();
        assert!(gep_operand2.is_int_value());
        assert_eq!(gep_operand2.into_int_value(), context.i32_type().const_int(1, false));

        assert!(second_insn.get_next_instruction() != None);
        let third_insn = second_insn.get_next_instruction().unwrap();
        assert_eq!(third_insn.get_opcode(), InstructionOpcode::BitCast);

        assert!(third_insn.get_next_instruction() != None);
        let fourth_insn = third_insn.get_next_instruction().unwrap();
        assert_eq!(fourth_insn.get_opcode(), InstructionOpcode::Load);

        let load_operand0 = fourth_insn.get_operand_value(0).unwrap();
        assert!(load_operand0.is_pointer_value());

        let load_operand0_ptr_elt_t = load_operand0.into_pointer_value().get_type().get_element_type();
        assert!(load_operand0_ptr_elt_t.is_int_type());
        assert!(load_operand0_ptr_elt_t.into_int_type().get_bit_width() == 160);

        assert!(fourth_insn.get_next_instruction() == None);
    }

    #[test]
    fn test_get_tx_ctx_item_coinbase() {
        let context = Context::create();
        let module = context.create_module("my_module");
        let builder = context.create_builder();

        // Need to create main function before TransactionConextManager otherwise we will crash
        MainFuncCreator::new ("main", &context, &builder, &module);

        let manager = RuntimeManager::new(&context, &builder, &module);

        let main_fn_optional = module.get_function ("main");
        assert!(main_fn_optional != None);

        let main_fn = main_fn_optional.unwrap();
        let coinbase_bb = context.append_basic_block(&main_fn, "Coinbase");

        builder.position_at_end(&coinbase_bb);

        // This call will generate some ir code for us to test
        manager.gen_tx_ctx_item_ir(TransactionContextTypeFields::CoinBase);

        assert!(coinbase_bb.get_first_instruction() != None);
        let first_insn = coinbase_bb.get_first_instruction().unwrap();
        assert_eq!(first_insn.get_opcode(), InstructionOpcode::Call);
        assert_eq!(first_insn.get_num_operands(), 4);

        let call_operand0 = first_insn.get_operand_value(0).unwrap();

        assert!(call_operand0.is_pointer_value());   // should be i1 *
        let call_operand0_ptr_elt_t = call_operand0.into_pointer_value().get_type().get_element_type();
        assert!(call_operand0_ptr_elt_t.is_int_type());
        assert!(call_operand0_ptr_elt_t.into_int_type().get_bit_width() == 1);

        let call_operand1 = first_insn.get_operand_value(1).unwrap();

        assert!(call_operand1.is_pointer_value());   // should be evm.txctx *

        let call_operand1_ptr_elt_t = call_operand1.into_pointer_value().get_type().get_element_type();
        assert!(call_operand1_ptr_elt_t.is_struct_type());
        assert!(TransactionContextType::is_transaction_context_type(&call_operand1_ptr_elt_t.as_struct_type()));

        let call_operand2 = first_insn.get_operand_value(2).unwrap();
        assert!(call_operand2.is_pointer_value());   // should be Env *

        let call_operand2_ptr_elt_t = call_operand2.into_pointer_value().get_type().get_element_type();
        assert!(call_operand2_ptr_elt_t.is_struct_type());

        assert!(EnvDataType::is_env_data_type(&call_operand2_ptr_elt_t.as_struct_type()));

        assert!(first_insn.get_next_instruction() != None);
        let second_insn = first_insn.get_next_instruction().unwrap();
        assert_eq!(second_insn.get_opcode(), InstructionOpcode::GetElementPtr);

        assert_eq!(second_insn.get_num_operands(), 3);

        let gep_operand0 = second_insn.get_operand_value(0).unwrap();
        assert!(gep_operand0.is_pointer_value());
        let gep_operand0_ptr_elt_t = gep_operand0.into_pointer_value().get_type().get_element_type();
        assert!(gep_operand0_ptr_elt_t.is_struct_type());
        let gep_operand0_type = gep_operand0_ptr_elt_t.into_struct_type();
        assert!(TransactionContextType::is_transaction_context_type(&gep_operand0_type));

        let gep_operand1 = second_insn.get_operand_value(1).unwrap();
        assert!(gep_operand1.is_int_value());
        assert_eq!(gep_operand1.into_int_value(), context.i32_type().const_int(0, false));

        let gep_operand2 = second_insn.get_operand_value(2).unwrap();
        assert!(gep_operand2.is_int_value());
        assert_eq!(gep_operand2.into_int_value(), context.i32_type().const_int(2, false));

        assert!(second_insn.get_next_instruction() != None);
        let third_insn = second_insn.get_next_instruction().unwrap();
        assert_eq!(third_insn.get_opcode(), InstructionOpcode::BitCast);

        assert!(third_insn.get_next_instruction() != None);
        let fourth_insn = third_insn.get_next_instruction().unwrap();
        assert_eq!(fourth_insn.get_opcode(), InstructionOpcode::Load);

        let load_operand0 = fourth_insn.get_operand_value(0).unwrap();
        assert!(load_operand0.is_pointer_value());

        let load_operand0_ptr_elt_t = load_operand0.into_pointer_value().get_type().get_element_type();
        assert!(load_operand0_ptr_elt_t.is_int_type());
        assert!(load_operand0_ptr_elt_t.into_int_type().get_bit_width() == 160);

        assert!(fourth_insn.get_next_instruction() == None);
    }

    #[test]
    fn test_get_tx_ctx_item_number() {
        let context = Context::create();
        let module = context.create_module("my_module");
        let builder = context.create_builder();

        // Need to create main function before TransactionConextManager otherwise we will crash
        MainFuncCreator::new ("main", &context, &builder, &module);

        let manager = RuntimeManager::new(&context, &builder, &module);

        let main_fn_optional = module.get_function ("main");
        assert!(main_fn_optional != None);

        let main_fn = main_fn_optional.unwrap();
        let number_bb = context.append_basic_block(&main_fn, "number");

        builder.position_at_end(&number_bb);

        // This call will generate some ir code for us to test
        manager.gen_tx_ctx_item_ir(TransactionContextTypeFields::Number);

        assert!(number_bb.get_first_instruction() != None);
        let first_insn = number_bb.get_first_instruction().unwrap();
        assert_eq!(first_insn.get_opcode(), InstructionOpcode::Call);
        assert_eq!(first_insn.get_num_operands(), 4);

        let call_operand0 = first_insn.get_operand_value(0).unwrap();

        assert!(call_operand0.is_pointer_value());   // should be i1 *
        let call_operand0_ptr_elt_t = call_operand0.into_pointer_value().get_type().get_element_type();
        assert!(call_operand0_ptr_elt_t.is_int_type());
        assert!(call_operand0_ptr_elt_t.into_int_type().get_bit_width() == 1);

        let call_operand1 = first_insn.get_operand_value(1).unwrap();

        assert!(call_operand1.is_pointer_value());   // should be evm.txctx *

        let call_operand1_ptr_elt_t = call_operand1.into_pointer_value().get_type().get_element_type();
        assert!(call_operand1_ptr_elt_t.is_struct_type());
        assert!(TransactionContextType::is_transaction_context_type(&call_operand1_ptr_elt_t.as_struct_type()));

        let call_operand2 = first_insn.get_operand_value(2).unwrap();
        assert!(call_operand2.is_pointer_value());   // should be Env *

        let call_operand2_ptr_elt_t = call_operand2.into_pointer_value().get_type().get_element_type();
        assert!(call_operand2_ptr_elt_t.is_struct_type());

        assert!(EnvDataType::is_env_data_type(&call_operand2_ptr_elt_t.as_struct_type()));

        assert!(first_insn.get_next_instruction() != None);
        let second_insn = first_insn.get_next_instruction().unwrap();
        assert_eq!(second_insn.get_opcode(), InstructionOpcode::GetElementPtr);

        assert_eq!(second_insn.get_num_operands(), 3);

        let gep_operand0 = second_insn.get_operand_value(0).unwrap();
        assert!(gep_operand0.is_pointer_value());
        let gep_operand0_ptr_elt_t = gep_operand0.into_pointer_value().get_type().get_element_type();
        assert!(gep_operand0_ptr_elt_t.is_struct_type());
        let gep_operand0_type = gep_operand0_ptr_elt_t.into_struct_type();
        assert!(TransactionContextType::is_transaction_context_type(&gep_operand0_type));

        let gep_operand1 = second_insn.get_operand_value(1).unwrap();
        assert!(gep_operand1.is_int_value());
        assert_eq!(gep_operand1.into_int_value(), context.i32_type().const_int(0, false));

        let gep_operand2 = second_insn.get_operand_value(2).unwrap();
        assert!(gep_operand2.is_int_value());
        assert_eq!(gep_operand2.into_int_value(), context.i32_type().const_int(3, false));

        assert!(second_insn.get_next_instruction() != None);
        let third_insn = second_insn.get_next_instruction().unwrap();
        assert_eq!(third_insn.get_opcode(), InstructionOpcode::Load);

        let load_operand0 = third_insn.get_operand_value(0).unwrap();
        assert!(load_operand0.is_pointer_value());

        let load_operand0_ptr_elt_t = load_operand0.into_pointer_value().get_type().get_element_type();
        assert!(load_operand0_ptr_elt_t.is_int_type());
        assert!(load_operand0_ptr_elt_t.into_int_type().get_bit_width() == 64);

        assert!(third_insn.get_next_instruction() == None);
    }

    #[test]
    fn test_get_tx_ctx_item_timestamp() {
        let context = Context::create();
        let module = context.create_module("my_module");
        let builder = context.create_builder();

        // Need to create main function before TransactionConextManager otherwise we will crash
        MainFuncCreator::new ("main", &context, &builder, &module);

        let manager = RuntimeManager::new(&context, &builder, &module);

        let main_fn_optional = module.get_function ("main");
        assert!(main_fn_optional != None);

        let main_fn = main_fn_optional.unwrap();
        let timestamp_bb = context.append_basic_block(&main_fn, "timestamp");

        builder.position_at_end(&timestamp_bb);

        // This call will generate some ir code for us to test
        manager.gen_tx_ctx_item_ir(TransactionContextTypeFields::TimeStamp);

        assert!(timestamp_bb.get_first_instruction() != None);
        let first_insn = timestamp_bb.get_first_instruction().unwrap();
        assert_eq!(first_insn.get_opcode(), InstructionOpcode::Call);
        assert_eq!(first_insn.get_num_operands(), 4);

        let call_operand0 = first_insn.get_operand_value(0).unwrap();

        assert!(call_operand0.is_pointer_value());   // should be i1 *
        let call_operand0_ptr_elt_t = call_operand0.into_pointer_value().get_type().get_element_type();
        assert!(call_operand0_ptr_elt_t.is_int_type());
        assert!(call_operand0_ptr_elt_t.into_int_type().get_bit_width() == 1);

        let call_operand1 = first_insn.get_operand_value(1).unwrap();

        assert!(call_operand1.is_pointer_value());   // should be evm.txctx *

        let call_operand1_ptr_elt_t = call_operand1.into_pointer_value().get_type().get_element_type();
        assert!(call_operand1_ptr_elt_t.is_struct_type());
        assert!(TransactionContextType::is_transaction_context_type(&call_operand1_ptr_elt_t.as_struct_type()));

        let call_operand2 = first_insn.get_operand_value(2).unwrap();
        assert!(call_operand2.is_pointer_value());   // should be Env *

        let call_operand2_ptr_elt_t = call_operand2.into_pointer_value().get_type().get_element_type();
        assert!(call_operand2_ptr_elt_t.is_struct_type());

        assert!(EnvDataType::is_env_data_type(&call_operand2_ptr_elt_t.as_struct_type()));

        assert!(first_insn.get_next_instruction() != None);
        let second_insn = first_insn.get_next_instruction().unwrap();
        assert_eq!(second_insn.get_opcode(), InstructionOpcode::GetElementPtr);

        assert_eq!(second_insn.get_num_operands(), 3);

        let gep_operand0 = second_insn.get_operand_value(0).unwrap();
        assert!(gep_operand0.is_pointer_value());
        let gep_operand0_ptr_elt_t = gep_operand0.into_pointer_value().get_type().get_element_type();
        assert!(gep_operand0_ptr_elt_t.is_struct_type());
        let gep_operand0_type = gep_operand0_ptr_elt_t.into_struct_type();
        assert!(TransactionContextType::is_transaction_context_type(&gep_operand0_type));

        let gep_operand1 = second_insn.get_operand_value(1).unwrap();
        assert!(gep_operand1.is_int_value());
        assert_eq!(gep_operand1.into_int_value(), context.i32_type().const_int(0, false));

        let gep_operand2 = second_insn.get_operand_value(2).unwrap();
        assert!(gep_operand2.is_int_value());
        assert_eq!(gep_operand2.into_int_value(), context.i32_type().const_int(4, false));

        assert!(second_insn.get_next_instruction() != None);
        let third_insn = second_insn.get_next_instruction().unwrap();
        assert_eq!(third_insn.get_opcode(), InstructionOpcode::Load);

        let load_operand0 = third_insn.get_operand_value(0).unwrap();
        assert!(load_operand0.is_pointer_value());

        let load_operand0_ptr_elt_t = load_operand0.into_pointer_value().get_type().get_element_type();
        assert!(load_operand0_ptr_elt_t.is_int_type());
        assert!(load_operand0_ptr_elt_t.into_int_type().get_bit_width() == 64);

        assert!(third_insn.get_next_instruction() == None);
    }

    #[test]
    fn test_get_tx_ctx_item_gaslimit() {
        let context = Context::create();
        let module = context.create_module("my_module");
        let builder = context.create_builder();

        // Need to create main function before TransactionConextManager otherwise we will crash
        MainFuncCreator::new ("main", &context, &builder, &module);

        let manager = RuntimeManager::new(&context, &builder, &module);

        let main_fn_optional = module.get_function ("main");
        assert!(main_fn_optional != None);

        let main_fn = main_fn_optional.unwrap();
        let gaslimit_bb = context.append_basic_block(&main_fn, "gaslimit");

        builder.position_at_end(&gaslimit_bb);

        // This call will generate some ir code for us to test
        manager.gen_tx_ctx_item_ir(TransactionContextTypeFields::GasLimit);

        assert!(gaslimit_bb.get_first_instruction() != None);
        let first_insn = gaslimit_bb.get_first_instruction().unwrap();
        assert_eq!(first_insn.get_opcode(), InstructionOpcode::Call);
        assert_eq!(first_insn.get_num_operands(), 4);

        let call_operand0 = first_insn.get_operand_value(0).unwrap();

        assert!(call_operand0.is_pointer_value());   // should be i1 *
        let call_operand0_ptr_elt_t = call_operand0.into_pointer_value().get_type().get_element_type();
        assert!(call_operand0_ptr_elt_t.is_int_type());
        assert!(call_operand0_ptr_elt_t.into_int_type().get_bit_width() == 1);

        let call_operand1 = first_insn.get_operand_value(1).unwrap();

        assert!(call_operand1.is_pointer_value());   // should be evm.txctx *

        let call_operand1_ptr_elt_t = call_operand1.into_pointer_value().get_type().get_element_type();
        assert!(call_operand1_ptr_elt_t.is_struct_type());
        assert!(TransactionContextType::is_transaction_context_type(&call_operand1_ptr_elt_t.as_struct_type()));

        let call_operand2 = first_insn.get_operand_value(2).unwrap();
        assert!(call_operand2.is_pointer_value());   // should be Env *

        let call_operand2_ptr_elt_t = call_operand2.into_pointer_value().get_type().get_element_type();
        assert!(call_operand2_ptr_elt_t.is_struct_type());

        assert!(EnvDataType::is_env_data_type(&call_operand2_ptr_elt_t.as_struct_type()));

        assert!(first_insn.get_next_instruction() != None);
        let second_insn = first_insn.get_next_instruction().unwrap();
        assert_eq!(second_insn.get_opcode(), InstructionOpcode::GetElementPtr);

        assert_eq!(second_insn.get_num_operands(), 3);

        let gep_operand0 = second_insn.get_operand_value(0).unwrap();
        assert!(gep_operand0.is_pointer_value());
        let gep_operand0_ptr_elt_t = gep_operand0.into_pointer_value().get_type().get_element_type();
        assert!(gep_operand0_ptr_elt_t.is_struct_type());
        let gep_operand0_type = gep_operand0_ptr_elt_t.into_struct_type();
        assert!(TransactionContextType::is_transaction_context_type(&gep_operand0_type));

        let gep_operand1 = second_insn.get_operand_value(1).unwrap();
        assert!(gep_operand1.is_int_value());
        assert_eq!(gep_operand1.into_int_value(), context.i32_type().const_int(0, false));

        let gep_operand2 = second_insn.get_operand_value(2).unwrap();
        assert!(gep_operand2.is_int_value());
        assert_eq!(gep_operand2.into_int_value(), context.i32_type().const_int(5, false));

        assert!(second_insn.get_next_instruction() != None);
        let third_insn = second_insn.get_next_instruction().unwrap();
        assert_eq!(third_insn.get_opcode(), InstructionOpcode::Load);

        let load_operand0 = third_insn.get_operand_value(0).unwrap();
        assert!(load_operand0.is_pointer_value());

        let load_operand0_ptr_elt_t = load_operand0.into_pointer_value().get_type().get_element_type();
        assert!(load_operand0_ptr_elt_t.is_int_type());
        assert!(load_operand0_ptr_elt_t.into_int_type().get_bit_width() == 64);

        assert!(third_insn.get_next_instruction() == None);
    }

    #[test]
    fn test_get_tx_ctx_item_difficulty() {
        let context = Context::create();
        let module = context.create_module("my_module");
        let builder = context.create_builder();

        // Need to create main function before TransactionConextManager otherwise we will crash
        MainFuncCreator::new ("main", &context, &builder, &module);

        let manager = RuntimeManager::new(&context, &builder, &module);

        let main_fn_optional = module.get_function ("main");
        assert!(main_fn_optional != None);

        let main_fn = main_fn_optional.unwrap();
        let difficulty_bb = context.append_basic_block(&main_fn, "difficulty");

        builder.position_at_end(&difficulty_bb);

        // This call will generate some ir code for us to test
        manager.gen_tx_ctx_item_ir(TransactionContextTypeFields::Difficulty);

        assert!(difficulty_bb.get_first_instruction() != None);
        let first_insn = difficulty_bb.get_first_instruction().unwrap();
        assert_eq!(first_insn.get_opcode(), InstructionOpcode::Call);
        assert_eq!(first_insn.get_num_operands(), 4);

        let call_operand0 = first_insn.get_operand_value(0).unwrap();

        assert!(call_operand0.is_pointer_value());   // should be i1 *
        let call_operand0_ptr_elt_t = call_operand0.into_pointer_value().get_type().get_element_type();
        assert!(call_operand0_ptr_elt_t.is_int_type());
        assert!(call_operand0_ptr_elt_t.into_int_type().get_bit_width() == 1);

        let call_operand1 = first_insn.get_operand_value(1).unwrap();

        assert!(call_operand1.is_pointer_value());   // should be evm.txctx *

        let call_operand1_ptr_elt_t = call_operand1.into_pointer_value().get_type().get_element_type();
        assert!(call_operand1_ptr_elt_t.is_struct_type());
        assert!(TransactionContextType::is_transaction_context_type(&call_operand1_ptr_elt_t.as_struct_type()));

        let call_operand2 = first_insn.get_operand_value(2).unwrap();
        assert!(call_operand2.is_pointer_value());   // should be Env *

        let call_operand2_ptr_elt_t = call_operand2.into_pointer_value().get_type().get_element_type();
        assert!(call_operand2_ptr_elt_t.is_struct_type());

        assert!(EnvDataType::is_env_data_type(&call_operand2_ptr_elt_t.as_struct_type()));

        assert!(first_insn.get_next_instruction() != None);
        let second_insn = first_insn.get_next_instruction().unwrap();
        assert_eq!(second_insn.get_opcode(), InstructionOpcode::GetElementPtr);

        assert_eq!(second_insn.get_num_operands(), 3);

        let gep_operand0 = second_insn.get_operand_value(0).unwrap();
        assert!(gep_operand0.is_pointer_value());
        let gep_operand0_ptr_elt_t = gep_operand0.into_pointer_value().get_type().get_element_type();
        assert!(gep_operand0_ptr_elt_t.is_struct_type());
        let gep_operand0_type = gep_operand0_ptr_elt_t.into_struct_type();
        assert!(TransactionContextType::is_transaction_context_type(&gep_operand0_type));

        let gep_operand1 = second_insn.get_operand_value(1).unwrap();
        assert!(gep_operand1.is_int_value());
        assert_eq!(gep_operand1.into_int_value(), context.i32_type().const_int(0, false));

        let gep_operand2 = second_insn.get_operand_value(2).unwrap();
        assert!(gep_operand2.is_int_value());
        assert_eq!(gep_operand2.into_int_value(), context.i32_type().const_int(6, false));

        assert!(second_insn.get_next_instruction() != None);
        let third_insn = second_insn.get_next_instruction().unwrap();
        assert_eq!(third_insn.get_opcode(), InstructionOpcode::Load);

        let load_operand0 = third_insn.get_operand_value(0).unwrap();
        assert!(load_operand0.is_pointer_value());

        let load_operand0_ptr_elt_t = load_operand0.into_pointer_value().get_type().get_element_type();
        assert!(load_operand0_ptr_elt_t.is_int_type());
        assert!(load_operand0_ptr_elt_t.into_int_type().get_bit_width() == 256);

        assert!(third_insn.get_next_instruction() == None);
    }

}

