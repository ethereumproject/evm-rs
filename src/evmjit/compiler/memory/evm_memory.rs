#![allow(dead_code)]

use inkwell::values::FunctionValue;
use inkwell::module::Linkage::*;
use std::cell::RefCell;
use evmjit::compiler::gas_cost::BasicBlockGasManager;
use evmjit::compiler::runtime::RuntimeManager;
use evmjit::compiler::external_declarations::ExternalFunctionManager;
use patch::Patch;
use super::super::JITContext;
use super::mem_representation::MemoryRepresentation;
use evmjit::compiler::byte_order::byte_order_swap;

struct MemoryFuncDeclarationManager<'a> {
    m_context: &'a JITContext,
    m_evm_mem_load_func: RefCell<Option<FunctionValue>>,
    m_evm_mem_store8_func: RefCell<Option<FunctionValue>>,
    m_evm_mem_store_func: RefCell<Option<FunctionValue>>,
    m_linear_memory: &'a MemoryRepresentation<'a>
}

impl<'a> MemoryFuncDeclarationManager<'a> {
    pub fn new(context: &'a JITContext, linear_memory: &'a MemoryRepresentation) -> MemoryFuncDeclarationManager<'a> {
        MemoryFuncDeclarationManager {
            m_context: context,
            m_evm_mem_load_func: RefCell::new(None),
            m_evm_mem_store8_func: RefCell::new(None),
            m_evm_mem_store_func: RefCell::new(None),
            m_linear_memory: linear_memory
        }
    }

    pub fn get_mstore8_func(&self) -> FunctionValue {
        if self.m_evm_mem_store8_func.borrow().is_none() {
            let types_instance = self.m_context.evm_types();
            let arg1 = self.m_context.memrep().get_ptr_type();
            let arg2 = types_instance.get_word_type();
            let arg3 = types_instance.get_byte_type();

            let context = self.m_context.llvm_context();
            let module = self.m_context.module();

            let ret_type = context.void_type();
            let mstore8_fn_type = ret_type.fn_type(&[arg1.into(), arg2.into(), arg3.into()], false);
            let mstore8_func = module.add_function("mstore8", mstore8_fn_type, Some(Private));

            assert!(mstore8_func.get_nth_param(0).is_some());
            let evm_mem_ptr = mstore8_func.get_nth_param(0).unwrap();
            evm_mem_ptr.into_pointer_value().set_name("mem");

            assert!(mstore8_func.get_nth_param(1).is_some());
            let index_in_mem = mstore8_func.get_nth_param(1).unwrap();
            index_in_mem.into_int_value().set_name("index");

            assert!(mstore8_func.get_nth_param(2).is_some());
            let value_to_store = mstore8_func.get_nth_param(1).unwrap();
            value_to_store.into_int_value().set_name("value");

            let temp_builder = context.create_builder();
            let entry_bb = context.append_basic_block(&mstore8_func, "mstore8_entry");
            temp_builder.position_at_end(&entry_bb);

            let size_t = types_instance.get_size_type();
            let trunc_index = temp_builder.build_int_truncate(index_in_mem.into_int_value(), size_t, "");
            let mem_ptr_value = self.m_linear_memory.get_mem_ptr(evm_mem_ptr.into_pointer_value(), trunc_index);
            let val_ptr = temp_builder.build_bitcast(mem_ptr_value, types_instance.get_byte_ptr_type(), "valuePtr");

            temp_builder.build_store(val_ptr.into_pointer_value(), value_to_store.into_int_value());
            temp_builder.build_return(None);

            // Return instance of function with IR built out

            *self.m_evm_mem_store8_func.borrow_mut() = Some(mstore8_func);
            mstore8_func
        }
        else {
            let func = self.m_evm_mem_store8_func.borrow().unwrap();
            func
        }
    }

    pub fn get_mstore_func(&self) -> FunctionValue {
        if self.m_evm_mem_store_func.borrow().is_none() {
            let types_instance = self.m_context.evm_types();
            let arg1 = self.m_context.memrep().get_ptr_type();
            let arg2 = types_instance.get_word_type();
            let arg3 = types_instance.get_word_type();

            let context = self.m_context.llvm_context();
            let module = self.m_context.module();

            let ret_type = context.void_type();
            let mstore_fn_type = ret_type.fn_type(&[arg1.into(), arg2.into(), arg3.into()], false);
            let mstore_func = module.add_function("mstore", mstore_fn_type, Some(Private));

            assert!(mstore_func.get_nth_param(0).is_some());
            let evm_mem_ptr = mstore_func.get_nth_param(0).unwrap();
            evm_mem_ptr.into_pointer_value().set_name("mem");

            assert!(mstore_func.get_nth_param(1).is_some());
            let index_in_mem = mstore_func.get_nth_param(1).unwrap();
            index_in_mem.into_int_value().set_name("index");

            assert!(mstore_func.get_nth_param(2).is_some());
            let value_to_store_arg = mstore_func.get_nth_param(1).unwrap();
            value_to_store_arg.into_int_value().set_name("value");

            let temp_builder = context.create_builder();
            let entry_bb = context.append_basic_block(&mstore_func, "mstore_entry");
            temp_builder.position_at_end(&entry_bb);

            // Value in memory are stored in big-endian order. Convert to big-endian if necessary
            let value_to_store = byte_order_swap (self.m_context, &temp_builder, value_to_store_arg.into_int_value());
            let size_t = types_instance.get_size_type();
            let trunc_index = temp_builder.build_int_truncate(index_in_mem.into_int_value(), size_t, "");
            let mem_ptr_value = self.m_linear_memory.get_mem_ptr(evm_mem_ptr.into_pointer_value(), trunc_index);
            let val_ptr = temp_builder.build_bitcast(mem_ptr_value, types_instance.get_word_ptr_type(), "valuePtr");

            temp_builder.build_store(val_ptr.into_pointer_value(), value_to_store);
            temp_builder.build_return(None);

            // Return instance of function with IR built out

            *self.m_evm_mem_store_func.borrow_mut() = Some(mstore_func);
            mstore_func
        }
        else {
            let func = self.m_evm_mem_store_func.borrow().unwrap();
            func
        }
    }

    pub fn get_mload_func(&self) -> FunctionValue {
        if self.m_evm_mem_load_func.borrow().is_none() {
            let types_instance = self.m_context.evm_types();
            let arg1 = self.m_context.memrep().get_ptr_type();
            let arg2 = types_instance.get_word_type();
            let ret_type = types_instance.get_word_type();
            let mload_fn_type = ret_type.fn_type(&[arg1.into(), arg2.into()], false);

            let context = self.m_context.llvm_context();
            let module = self.m_context.module();

            let mload_func = module.add_function("mload", mload_fn_type, Some(Private));

            assert!(mload_func.get_nth_param(0).is_some());
            let evm_mem_ptr = mload_func.get_nth_param(0).unwrap();
            evm_mem_ptr.into_pointer_value().set_name("mem");

            assert!(mload_func.get_nth_param(1).is_some());
            let index_in_mem = mload_func.get_nth_param(1).unwrap();
            index_in_mem.into_int_value().set_name("index");

            let temp_builder = context.create_builder();
            let entry_bb = context.append_basic_block(&mload_func, "mload_entry");
            temp_builder.position_at_end(&entry_bb);

            let size_t = types_instance.get_size_type();

            // Truncate 256-bit index into 64-bit index
            let trunc_index = temp_builder.build_int_truncate(index_in_mem.into_int_value(), size_t, "");
            let mem_ptr_value = self.m_linear_memory.get_mem_ptr(evm_mem_ptr.into_pointer_value(), trunc_index);
            let ret_val = temp_builder.build_load(mem_ptr_value, "");

            // Value returned from evm memory is in big-endia order, swap if necessary on little-endian
            let native_order_val = byte_order_swap (self.m_context, &temp_builder, ret_val.into_int_value());
            temp_builder.build_return (Some (&native_order_val));

            // Return instance of function with IR built out

            *self.m_evm_mem_load_func.borrow_mut() = Some(mload_func);
            mload_func
        }
        else {
            let func = self.m_evm_mem_load_func.borrow().unwrap();
            func
        }
    }
}
pub struct EvmMemory<'a, P: Patch + 'a> {
    m_context: &'a JITContext,
    m_gas_mgr: &'a BasicBlockGasManager<'a, P>,
    m_linear_memory: MemoryRepresentation<'a>
}

impl<'a, P: Patch> EvmMemory<'a, P> {
    pub fn new(context: &'a JITContext,
               gas_manager: &'a BasicBlockGasManager<'a, P>, rt_manager: &RuntimeManager<'a>,
               external_func_mgr: &'a ExternalFunctionManager) -> EvmMemory<'a, P> {

        let mem = MemoryRepresentation::new(rt_manager.get_mem_ptr(), context,
                                                                external_func_mgr);

        EvmMemory {
            m_context: context,
            m_gas_mgr: gas_manager,
            m_linear_memory: mem
        }

    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use evmjit::compiler::evm_compiler::MainFuncCreator;
    use evmjit::compiler::external_declarations::ExternalFunctionManager;
    use patch::EmbeddedPatch;

    #[test]
    fn test_memory_creation() {
        let jitctx = JITContext::new();
        let module = jitctx.module();
        let decl_factory = ExternalFunctionManager::new(&jitctx);

        //let attr_factory = LLVMAttributeFactory::get_instance(&context);

        // Generate outline of main function needed by 'RuntimeTypeManager
        MainFuncCreator::new("main", &jitctx);

        let rt_manager = RuntimeManager::new(&jitctx, &decl_factory);

        let gas_manager : BasicBlockGasManager<EmbeddedPatch> = BasicBlockGasManager::new(&jitctx, &rt_manager);
        let _memory:EvmMemory<EmbeddedPatch> = EvmMemory::new(&jitctx, &gas_manager, &rt_manager, &decl_factory);
        module.print_to_stderr()
    }
}
