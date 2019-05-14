#![allow(dead_code)]

use super::super::JITContext;
use super::super::util::funcbuilder::*;
use super::mem_representation::MemoryRepresentation;
use evmjit::compiler::byte_order::byte_order_swap;
use evmjit::compiler::external_declarations::ExternalFunctionManager;
use evmjit::compiler::gas_cost::BasicBlockGasManager;
use evmjit::compiler::runtime::RuntimeManager;
use evmjit::compiler::exceptions::ExceptionManager;
use inkwell::module::Linkage::*;
use inkwell::values::FunctionValue;
use inkwell::values::PointerValue;
use inkwell::values::IntValue;
use inkwell::IntPredicate;
use inkwell::AddressSpace;
use patch::Patch;
use std::cell::RefCell;
use std::marker::PhantomData;

struct MemoryFuncDeclarationManager<'a, P: Patch + 'a> {
    m_context: &'a JITContext,
    m_evm_mem_load_func: RefCell<Option<FunctionValue>>,
    m_evm_mem_store8_func: RefCell<Option<FunctionValue>>,
    m_evm_mem_store_func: RefCell<Option<FunctionValue>>,
    m_allocate_mem_func: RefCell<Option<FunctionValue>>,
    _marker: PhantomData<P>,
}

impl<'a, P: Patch> MemoryFuncDeclarationManager<'a, P> {
    pub fn new(context: &'a JITContext) -> MemoryFuncDeclarationManager<'a, P> {
        MemoryFuncDeclarationManager {
            m_context: context,
            m_evm_mem_load_func: RefCell::new(None),
            m_evm_mem_store8_func: RefCell::new(None),
            m_evm_mem_store_func: RefCell::new(None),
            m_allocate_mem_func: RefCell::new(None),
            _marker: PhantomData,
        }
    }

    pub fn get_alloc_mem_func(&self, gas_manager: &'a BasicBlockGasManager<'a, P>, exc_manager: &'a ExceptionManager, linear_memory: &'a MemoryRepresentation) -> FunctionValue {
        if self.m_allocate_mem_func.borrow().is_none() {
            let types_instance = self.m_context.evm_types();
            let arg1 = self.m_context.memrep().get_ptr_type();
            let arg2 = types_instance.get_word_type();
            let arg3 = types_instance.get_word_type();
            let arg4 = types_instance.get_byte_ptr_type();
            let arg5 = types_instance.get_gas_ptr_type();

            let context = self.m_context.llvm_context();
            let module = self.m_context.module();
            let attr_factory = self.m_context.attributes();

            let ret_type = context.void_type();
            let alloc_mem_fn_type = FunctionTypeBuilder::new(context)
                .returns(ret_type)
                .arg(arg1)
                .arg(arg2)
                .arg(arg3)
                .arg(arg4)
                .arg(arg5)
                .build()
                .unwrap();

            let alloc_mem_func = module.add_function("mem.allocate", alloc_mem_fn_type, Some(Private));

            // Function does not throw
            alloc_mem_func.add_attribute(0, *attr_factory.attr_nounwind());

            assert!(alloc_mem_func.get_nth_param(0).is_some());
            let evm_mem_ptr = alloc_mem_func.get_nth_param(0).unwrap();
            evm_mem_ptr.into_pointer_value().set_name("mem");

            assert!(alloc_mem_func.get_nth_param(1).is_some());
            let offset_in_mem = alloc_mem_func.get_nth_param(1).unwrap();
            offset_in_mem.into_int_value().set_name("memOffset");
            let offset_in_mem_val = offset_in_mem.into_int_value();

            assert!(alloc_mem_func.get_nth_param(2).is_some());
            let size_in_mem = alloc_mem_func.get_nth_param(2).unwrap();
            size_in_mem.into_int_value().set_name("memSize");

            let size_in_mem_val = size_in_mem.into_int_value();

            assert!(alloc_mem_func.get_nth_param(3).is_some());
            let long_jmp_buf = alloc_mem_func.get_nth_param(3).unwrap();
            long_jmp_buf.into_pointer_value().set_name("longJumpBuf");

            assert!(alloc_mem_func.get_nth_param(4).is_some());
            let gas = alloc_mem_func.get_nth_param(4).unwrap();
            gas.into_pointer_value().set_name("gas");

            let temp_builder = context.create_builder();
            let entry_bb = context.append_basic_block(&alloc_mem_func, "alloc_entry");
            let check_bb = context.append_basic_block(&alloc_mem_func, "alloc_check");
            let resize_bb = context.append_basic_block(&alloc_mem_func, "alloc_resize");
            let return_bb = context.append_basic_block(&alloc_mem_func, "alloc_return");

            let size_t = types_instance.get_size_type();

            temp_builder.position_at_end(&entry_bb);

            let zero_256_t = context.custom_width_int_type(256).const_zero();
            let cmp_ne = temp_builder.build_int_compare(IntPredicate::NE, size_in_mem_val,
                                                        zero_256_t, "");

            // Return immediately if requested size is zero
            temp_builder.build_conditional_branch(cmp_ne, &check_bb, &return_bb);

            // Check to see if the memory is already of sufficient size and whether it needs to be extended

            temp_builder.position_at_end(&check_bb);
            let max_mem_offset = 1u64 << 33;
            let max_offset_64_t = context.i64_type().const_int(max_mem_offset, false);
            let max_offset_256_t = context.custom_width_int_type(256).const_int(max_mem_offset, false);
            let mem_offset_ok = temp_builder.build_int_compare(IntPredicate::ULE, offset_in_mem_val, max_offset_256_t, "memoffsetOk");

            let mem_offset_64_t = temp_builder.build_int_truncate(offset_in_mem_val, size_t, "");
            let mem_offset = temp_builder.build_select(mem_offset_ok, mem_offset_64_t, max_offset_64_t, "memOffset");

            let mem_size_ok = temp_builder.build_int_compare(IntPredicate::ULE, size_in_mem_val, max_offset_256_t, "memsizeOk");
            let mem_size_64_t = temp_builder.build_int_truncate(size_in_mem_val, size_t, "");
            let mem_size = temp_builder.build_select(mem_size_ok, mem_size_64_t, max_offset_64_t, "memSize");

            let size_req_temp = temp_builder.build_int_nuw_add(mem_offset.into_int_value(), mem_size.into_int_value(), "");
            let all_ones = 0xffffffffffffffffu64;
            let mask_64 = context.i64_type().const_int(all_ones << 5, false);
            let size_required = temp_builder.build_and(size_req_temp, mask_64, "sizeReq");
            let current_size = linear_memory.get_mem_size(evm_mem_ptr.into_pointer_value());

            let is_size_ok = temp_builder.build_int_compare(IntPredicate::ULE, size_required, current_size.into_int_value(), "bSizeOk");
            temp_builder.build_conditional_branch(is_size_ok, &return_bb, &resize_bb);

            // The size exceeds our current memory size, extend the memory size
            // We have to check the gas first
            // Gas cost of memory is:
            //
            // a = num of 256-bit words to allocate
            // 3 * a + ((a ^2)/512)

            // The final cost will be the difference between the cost of what we have allocated and
            // what is required

            temp_builder.position_at_end(&resize_bb);

            let const_five = context.i64_type().const_int(5, false);
            let const_three = context.i64_type().const_int(3, false);
            let const_nine = context.i64_type().const_int(9, false);

            // Calculate cost of new size
            let size_required_in_words = temp_builder.build_right_shift(size_required, const_five, false, "");
            let size_required_squared = temp_builder.build_int_nuw_mul(size_required_in_words, size_required_in_words, "");
            let size_required_cost_expr1 = temp_builder.build_int_nuw_mul(size_required_in_words, const_three, "");

            // We can divide by 9 with logical right shift

            let size_required_cost_expr2 = temp_builder.build_right_shift(size_required_squared, const_nine, false, "");
            let size_required_cost = temp_builder.build_int_add(size_required_cost_expr1, size_required_cost_expr2, "");

            // Now calculate cost of current size

            let size_current_in_words = temp_builder.build_right_shift(current_size.into_int_value(), const_five, false, "");
            let size_current_squared = temp_builder.build_int_nuw_mul(size_current_in_words, size_current_in_words, "");
            let size_current_cost_expr1 = temp_builder.build_int_nuw_mul(size_current_in_words, const_three, "");
            let size_current_cost_expr2 = temp_builder.build_right_shift(size_current_squared, const_nine, false, "");
            let size_current_cost = temp_builder.build_int_add(size_current_cost_expr1, size_current_cost_expr2, "");

            let true_cost = temp_builder.build_int_nuw_sub(size_required_cost, size_current_cost, "");

            // Validate that the memory offset and memory size request are okay
            let mem_cost_ok = temp_builder.build_and(mem_offset_ok, mem_size_ok, "bMemCostOk");
            let max_int64 = self.m_context.evm_constants().get_gas_max();
            let mem_cost = temp_builder.build_select(mem_cost_ok, true_cost, max_int64, "");

            // Check the gas cost before we expand the memory. If the gas is used up we will get an exception

            gas_manager.count_variable_cost(mem_cost.into_int_value(), exc_manager);
            linear_memory.extend_memory_size(evm_mem_ptr.into_pointer_value(),size_required);
            temp_builder.build_unconditional_branch(&return_bb);

            // Normal return path for generated code

            temp_builder.position_at_end(&return_bb);
            temp_builder.build_return(None);

            alloc_mem_func
        }
        else {
            let func = self.m_allocate_mem_func.borrow().unwrap();
            func
        }
    }

    pub fn get_mstore8_func(&self, linear_memory: &'a MemoryRepresentation) -> FunctionValue {
        if self.m_evm_mem_store8_func.borrow().is_none() {
            let types_instance = self.m_context.evm_types();
            let arg1 = self.m_context.memrep().get_ptr_type();
            let arg2 = types_instance.get_word_type();
            let arg3 = types_instance.get_byte_type();

            let context = self.m_context.llvm_context();
            let module = self.m_context.module();

            let ret_type = context.void_type();
            let mstore8_fn_type = FunctionTypeBuilder::new(context)
                .returns(ret_type)
                .arg(arg1)
                .arg(arg2)
                .arg(arg3)
                .build()
                .unwrap();
            let mstore8_func = module.add_function("mstore8", mstore8_fn_type, Some(Private));

            assert!(mstore8_func.get_nth_param(0).is_some());
            let evm_mem_ptr = mstore8_func.get_nth_param(0).unwrap();
            evm_mem_ptr.into_pointer_value().set_name("mem");

            assert!(mstore8_func.get_nth_param(1).is_some());
            let index_in_mem = mstore8_func.get_nth_param(1).unwrap();
            index_in_mem.into_int_value().set_name("index");

            assert!(mstore8_func.get_nth_param(2).is_some());
            let value_to_store = mstore8_func.get_nth_param(2).unwrap();
            value_to_store.into_int_value().set_name("value");

            let temp_builder = context.create_builder();
            let entry_bb = context.append_basic_block(&mstore8_func, "mstore8_entry");
            temp_builder.position_at_end(&entry_bb);

            let size_t = types_instance.get_size_type();
            let trunc_index = temp_builder.build_int_truncate(index_in_mem.into_int_value(), size_t, "");
            let mem_ptr_value = linear_memory.get_mem_ptr(evm_mem_ptr.into_pointer_value(), trunc_index);
            let val_ptr = temp_builder.build_bitcast(mem_ptr_value, types_instance.get_byte_ptr_type(), "valuePtr");

            temp_builder.build_store(val_ptr.into_pointer_value(), value_to_store.into_int_value());
            temp_builder.build_return(None);

            // Return instance of function with IR built out

            *self.m_evm_mem_store8_func.borrow_mut() = Some(mstore8_func);
            mstore8_func
        } else {
            let func = self.m_evm_mem_store8_func.borrow().unwrap();
            func
        }
    }

    pub fn get_mstore_func(&self, linear_memory: &'a MemoryRepresentation) -> FunctionValue {
        if self.m_evm_mem_store_func.borrow().is_none() {
            let types_instance = self.m_context.evm_types();
            let arg1 = self.m_context.memrep().get_ptr_type();
            let arg2 = types_instance.get_word_type();
            let arg3 = types_instance.get_word_type();

            let context = self.m_context.llvm_context();
            let module = self.m_context.module();

            let ret_type = context.void_type();
            let mstore_fn_type = FunctionTypeBuilder::new(context)
                .returns(ret_type)
                .arg(arg1)
                .arg(arg2)
                .arg(arg3)
                .build()
                .unwrap();
            let mstore_func = module.add_function("mstore", mstore_fn_type, Some(Private));

            assert!(mstore_func.get_nth_param(0).is_some());
            let evm_mem_ptr = mstore_func.get_nth_param(0).unwrap();
            evm_mem_ptr.into_pointer_value().set_name("mem");

            assert!(mstore_func.get_nth_param(1).is_some());
            let index_in_mem = mstore_func.get_nth_param(1).unwrap();
            index_in_mem.into_int_value().set_name("index");

            assert!(mstore_func.get_nth_param(2).is_some());
            let value_to_store_arg = mstore_func.get_nth_param(2).unwrap();
            value_to_store_arg.into_int_value().set_name("value");

            let temp_builder = context.create_builder();
            let entry_bb = context.append_basic_block(&mstore_func, "mstore_entry");
            temp_builder.position_at_end(&entry_bb);

            // Value in memory are stored in big-endian order. Convert to big-endian if necessary
            let value_to_store = byte_order_swap(self.m_context, &temp_builder, value_to_store_arg.into_int_value());
            let size_t = types_instance.get_size_type();
            let trunc_index = temp_builder.build_int_truncate(index_in_mem.into_int_value(), size_t, "");
            let mem_ptr_value = linear_memory.get_mem_ptr(evm_mem_ptr.into_pointer_value(), trunc_index);
            let val_ptr = temp_builder.build_bitcast(mem_ptr_value, types_instance.get_word_ptr_type(), "valuePtr");

            temp_builder.build_store(val_ptr.into_pointer_value(), value_to_store);
            temp_builder.build_return(None);

            // Return instance of function with IR built out

            *self.m_evm_mem_store_func.borrow_mut() = Some(mstore_func);
            mstore_func
        } else {
            let func = self.m_evm_mem_store_func.borrow().unwrap();
            func
        }
    }

    pub fn get_mload_func(&self, linear_memory: &'a MemoryRepresentation) -> FunctionValue {
        if self.m_evm_mem_load_func.borrow().is_none() {
            let context = self.m_context.llvm_context();
            let types_instance = self.m_context.evm_types();

            let arg1 = self.m_context.memrep().get_ptr_type();
            let arg2 = types_instance.get_word_type();
            let ret_type = types_instance.get_word_type();
            let mload_fn_type = FunctionTypeBuilder::new(context)
                .returns(ret_type)
                .arg(arg1)
                .arg(arg2)
                .build()
                .unwrap();

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
            let mem_ptr_value = linear_memory.get_mem_ptr(evm_mem_ptr.into_pointer_value(), trunc_index);
            let ret_val = temp_builder.build_load(mem_ptr_value, "");

            // Value returned from evm memory is in big-endia order, swap if necessary on little-endian
            let native_order_val = byte_order_swap(self.m_context, &temp_builder, ret_val.into_int_value());
            temp_builder.build_return(Some(&native_order_val));

            // Return instance of function with IR built out

            *self.m_evm_mem_load_func.borrow_mut() = Some(mload_func);
            mload_func
        } else {
            let func = self.m_evm_mem_load_func.borrow().unwrap();
            func
        }
    }
}
pub struct EvmMemory<'a, P: Patch + 'a> {
    m_context: &'a JITContext,
    m_gas_mgr: &'a BasicBlockGasManager<'a, P>,
    m_linear_memory: MemoryRepresentation<'a>,
    m_runtime_manager: &'a RuntimeManager<'a>,
    m_exception_manager: &'a ExceptionManager,
    m_mem_declaration_manager: MemoryFuncDeclarationManager<'a, P>
}

impl<'a, P: Patch> EvmMemory<'a, P> {
    pub fn new(
        context: &'a JITContext,
        gas_manager: &'a BasicBlockGasManager<'a, P>,
        rt_manager: &'a RuntimeManager<'a>,
        external_func_mgr: &'a ExternalFunctionManager,
        exception_mgr: &'a ExceptionManager
    ) -> EvmMemory<'a, P> {
        let mem = MemoryRepresentation::new(rt_manager.get_mem_ptr(), context, external_func_mgr);

        EvmMemory {
            m_context: context,
            m_gas_mgr: gas_manager,
            m_linear_memory: mem,
            m_runtime_manager: rt_manager,
            m_exception_manager: exception_mgr,
            m_mem_declaration_manager: MemoryFuncDeclarationManager::new(&context)
        }
    }

    fn get_data_ptr(&self) -> PointerValue {
        let builder = self.m_context.builder();
        let evm_byte_ptr_type = self.m_context.evm_types().get_byte_ptr_type();
        let cast_type = evm_byte_ptr_type.ptr_type(AddressSpace::Generic);
        let mem_ptr = builder.build_bitcast(self.m_runtime_manager.get_mem_ptr(), cast_type, "");

        let data = builder.build_load(mem_ptr.into_pointer_value(), "data");
        assert!(data.is_pointer_value());
        assert_eq!(data.get_type().into_pointer_type(), evm_byte_ptr_type);
        data.into_pointer_value()
    }

    pub fn get_byte_ptr(&self, mem_index: IntValue) -> PointerValue {
        let builder = self.m_context.builder();
        unsafe {
         builder.build_gep(self.get_data_ptr(), &[mem_index], "bytePtr")
        }
    }

    fn allocate_mem(&self, offset_in_mem: IntValue, value_in_mem_size: IntValue) {
        // Check if memory is constant and == zero, don't extend memory if true
        if value_in_mem_size.is_const() {
            let value_size = value_in_mem_size.get_zero_extended_constant();
            assert!(value_size.is_some());
            if value_size.unwrap() == 0 {
                return;
            }
        }

        let arg1 = self.m_linear_memory.get_mem();
        let arg2 = offset_in_mem;
        let arg3 = value_in_mem_size;
        let arg4 = self.m_exception_manager.get_exception_dest();
        let arg5 = *self.m_runtime_manager.get_gas_ptr();

        let alloc_mem_func = self.m_mem_declaration_manager.get_alloc_mem_func(self.m_gas_mgr, self.m_exception_manager, &self.m_linear_memory);
        self.m_context.builder().build_call(alloc_mem_func, &[arg1.into(), arg2.into(), arg3.into(), arg4.into(), arg5.into()], "");
    }

    pub fn memory_load(&self, addr: IntValue) -> IntValue {
        let size_in_bytes:u64 = self.m_context.evm_types().get_word_type().get_bit_width() as u64 / 8u64;
        let size_in_bytes_256 = self.m_context.llvm_context().custom_width_int_type(256).const_int(size_in_bytes, false);

        // Extend memory if necessary
        self.allocate_mem(addr, size_in_bytes_256);

        let load_word_func = self.m_mem_declaration_manager.get_mload_func(&self.m_linear_memory);
        let arg1 = self.m_linear_memory.get_mem();
        let arg2 = addr;

        // Call function to load value from memory
        let callsite_val = self.m_context.builder().build_call(load_word_func, &[arg1.into(), arg2.into()], "");

        // build_call returns a Either<BasicValueEnum, InstructionValue>
        let ret = callsite_val.try_as_basic_value().left().unwrap();
        ret.into_int_value()
    }

    pub fn memory_store(&self, addr: IntValue, value: IntValue) {
        let size_in_bytes:u64 = self.m_context.evm_types().get_word_type().get_bit_width() as u64 / 8u64;
        let size_in_bytes_256 = self.m_context.llvm_context().custom_width_int_type(256).const_int(size_in_bytes, false);

        // Extend memory if necessary
        self.allocate_mem(addr, size_in_bytes_256);

        let store_word_func = self.m_mem_declaration_manager.get_mstore_func(&self.m_linear_memory);
        let arg1 = self.m_linear_memory.get_mem();
        let arg2 = addr;
        let arg3 = value;

        // Call function to store word into memory
        self.m_context.builder().build_call(store_word_func, &[arg1.into(), arg2.into(), arg3.into()], "");
    }

    pub fn memory_store_byte(&self, addr: IntValue, value: IntValue) {
        let evm_byte_type = self.m_context.evm_types().get_byte_type();
        let size_in_bytes:u64 = evm_byte_type.get_bit_width() as u64 / 8u64;
        let size_in_bytes_256 = self.m_context.llvm_context().custom_width_int_type(256).const_int(size_in_bytes, false);

        // Extend memory if necessary
        self.allocate_mem(addr, size_in_bytes_256);

        let byte_value = self.m_context.builder().build_int_truncate(value, evm_byte_type, "");

        let store_byte_func = self.m_mem_declaration_manager.get_mstore8_func(&self.m_linear_memory);
        let arg1 = self.m_linear_memory.get_mem();
        let arg2 = addr;
        let arg3 = byte_value;

        // Call function to store byte into memory
        self.m_context.builder().build_call(store_byte_func, &[arg1.into(), arg2.into(), arg3.into()], "");
    }

    pub fn get_size(&self) -> IntValue {
        let size = self.m_linear_memory.get_internal_mem_size();
        let size_val = size.into_int_value();
        self.m_context.builder().build_int_z_extend(size_val, self.m_context.evm_types().get_word_type(), "")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use evmjit::compiler::evm_compiler::MainFuncCreator;
    use evmjit::compiler::{ExternalFunctionManager, DeclarationManager};
    use patch::EmbeddedPatch;

    #[test]
    fn test_memory_creation() {
        let jitctx = JITContext::new();
        let module = jitctx.module();
        let decl_factory = ExternalFunctionManager::new(&jitctx);

        //let attr_factory = LLVMAttributeFactory::get_instance(&context);

        // Generate outline of main function needed by 'RuntimeTypeManager
       let main =  MainFuncCreator::new("main", &jitctx);

        let rt_manager = RuntimeManager::new(&jitctx, &decl_factory);

        let gas_manager: BasicBlockGasManager<EmbeddedPatch> = BasicBlockGasManager::new(&jitctx, &rt_manager);
        let bb_after_entry = main.get_entry_bb().get_next_basic_block();
        let exc_manager = ExceptionManager::new(&jitctx, &bb_after_entry.unwrap(), &main.get_abort_bb());
        let _memory: EvmMemory<EmbeddedPatch> = EvmMemory::new(&jitctx, &gas_manager, &rt_manager, &decl_factory, &exc_manager);
        module.print_to_stderr()
    }
}
