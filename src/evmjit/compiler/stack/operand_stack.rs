use std::cell::Cell;
use inkwell::values::{IntValue, PointerValue};
use inkwell::module::Linkage::*;
use evmjit::compiler::util::funcbuilder::*;
use inkwell::AddressSpace;
use inkwell::values::FunctionValue;
use inkwell::values::InstructionValue;
use inkwell::IntPredicate;
use inkwell::values::CallSiteValue;
use super::super::JITContext;
use evmjit::compiler::stack::EVM_MAX_STACK_SIZE;
use evmjit::compiler::intrinsics::LLVMIntrinsic;
use evmjit::compiler::intrinsics::LLVMIntrinsicManager;
use evmjit::compiler::exceptions::ExceptionManager;
use evmjit::compiler::runtime::RuntimeManager;

pub struct OperandStack<'a> {
    m_context: &'a JITContext,
    m_operand_stack: Vec<IntValue>,
    m_min_stack_size: Cell<usize>,
    m_max_stack_size: Cell<usize>,
    m_stack_base: PointerValue,
    m_stack_size_ptr: PointerValue,
    m_exception_dest: PointerValue
}

impl<'a> OperandStack<'a> {
    pub fn new(context: &'a JITContext, rt_manager: &RuntimeManager, exception_mgr: &ExceptionManager) -> OperandStack<'a> {
        let stack_base = rt_manager.get_stack_base();
        let stack_size_ptr = rt_manager.get_stack_size_ptr();
        let jmp_buf = exception_mgr.get_exception_dest();


        OperandStack {
            m_context: context,
            m_operand_stack: Vec::new(),
            m_min_stack_size: Cell::new(0),
            m_max_stack_size: Cell::new(0),
            m_stack_base: stack_base,
            m_stack_size_ptr: stack_size_ptr,
            m_exception_dest: jmp_buf
        }
    }

    pub fn num_elements(&self) -> usize {
        self.m_operand_stack.len()
    }

    pub fn pop(&mut self) -> IntValue {
        assert!(self.m_operand_stack.len() > 0);
        let val = self.m_operand_stack.pop();
        assert!(val.is_some());
        let min_val = std::cmp::min(self.m_min_stack_size.get(), self.m_operand_stack.len());
        self.m_min_stack_size.set(min_val);
        val.unwrap()
    }

    pub fn push(&mut self, value: IntValue) {
        let word_t = self.m_context.evm_types().get_word_type();

        // Make sure we are pushing a 256-bit value
        assert!(value.get_type() == word_t);
        self.m_operand_stack.push(value);
        let max_val = std::cmp::max(self.m_max_stack_size.get(), self.m_operand_stack.len());
        self.m_max_stack_size.set(max_val);
    }

    pub fn dup(&mut self, index: usize) {
        let val = self.get(index);
        self.push(val);
    }

    pub fn swap(&mut self, index : usize) {
        assert!((index as isize ) > 0);

        let val = self.get(index);
        let top_of_stack_val = self.get(0);
        self.set(index, top_of_stack_val);
        self.set(0, val);
    }

    //void LocalStack::finalize()
    //{
    //	m_sp->setArgOperand(2, m_builder.getInt64(minSize()));
    //	m_sp->setArgOperand(3, m_builder.getInt64(maxSize()));
    //	m_sp->setArgOperand(4, m_builder.getInt64(size()));

    pub fn finalize_stack(&self) {
        let builder = self.m_context.builder();
        let type_factory = self.m_context.evm_types();
        let size_t = type_factory.get_size_type();

        let current_bb_opt = builder.get_insert_block();
        assert!(current_bb_opt.is_some());

        let current_bb = current_bb_opt.unwrap();
        let first_inst = current_bb.get_first_instruction();
        assert!(first_inst.is_some());

        let temp_builder = self.m_context.llvm_context().create_builder();
        temp_builder.position_before(&first_inst.unwrap());

        let stack_check_func = create_stack_check_func(self.m_context);
        let min_stack_size = size_t.const_int(self.m_min_stack_size.get() as u64, false);
        let max_stack_size = size_t.const_int(self.m_max_stack_size.get() as u64, false);
        let num_elements = size_t.const_int(self.num_elements() as u64, false);
        temp_builder.build_call(stack_check_func, &[self.m_stack_base.into(),
                                                                  self.m_stack_size_ptr.into(),
                                                                  min_stack_size.into(),
                                                                  max_stack_size.into(),
                                                                  num_elements.into(),
                                                                  self.m_exception_dest.into()], "stack.check");
    }

    fn reverse_index(&self, index: usize) -> usize {
        assert!(index < self.m_operand_stack.len());

        self.m_operand_stack.len() - index - 1
    }

    fn get(&self, index: usize) -> IntValue {
        assert!(index < self.m_operand_stack.len());

        let mut it = self.m_operand_stack.iter().rev().into_iter();
        let val = it.nth(index);
        assert!(val.is_some());

        *val.unwrap()
    }

    fn set(&mut self, index: usize, value: IntValue) {
        assert!(index < self.m_operand_stack.len());

        let actual_index = self.reverse_index(index);
        self.m_operand_stack[actual_index] = value;
    }
}

fn create_stack_check_func(context: &JITContext) -> FunctionValue {
    static FUNC_NAME: &str  = "jit.stack.check";
    let stack_check_func_found = context.module().get_function(FUNC_NAME);

    if stack_check_func_found.is_some() {
        stack_check_func_found.unwrap()
    }
    else {
        let llvm_context = context.llvm_context();
        let module = context.module();
        let attr_factory = context.attributes();
        let type_factory = context.evm_types();

        let word_ptr = type_factory.get_word_ptr_type();
        let size_t = type_factory.get_size_type();
        let size_ptr = size_t.ptr_type(AddressSpace::Generic);
        let byte_ptr = type_factory.get_byte_ptr_type();

        let stack_check_func_type = FunctionTypeBuilder::new(llvm_context)
            .returns(word_ptr)
            .arg(word_ptr)
            .arg(size_ptr)
            .arg(size_t)
            .arg(size_t)
            .arg(size_t)
            .arg(byte_ptr)
            .build()
            .unwrap();

        let stack_check_func = module.add_function(FUNC_NAME, stack_check_func_type, Some(Private));

        // Function does not throw
        stack_check_func.add_attribute(0, *attr_factory.attr_nounwind());
        stack_check_func.add_attribute(1, *attr_factory.attr_readnone());
        stack_check_func.add_attribute(2, *attr_factory.attr_noalias());
        stack_check_func.add_attribute(2, *attr_factory.attr_nocapture());

        let mut arg_num = 0;

        assert!(stack_check_func.get_nth_param(arg_num).is_some());
        let base = stack_check_func.get_nth_param(arg_num).unwrap();
        let base_val = base.into_pointer_value();
        base_val.set_name("base");


        arg_num += 1;
        assert!(stack_check_func.get_nth_param(arg_num).is_some());
        let size_ptr = stack_check_func.get_nth_param(arg_num).unwrap();
        let size_ptr_val = size_ptr.into_pointer_value();
        size_ptr_val.set_name("size.ptr");

        arg_num += 1;
        assert!(stack_check_func.get_nth_param(arg_num).is_some());
        let min = stack_check_func.get_nth_param(arg_num).unwrap();
        let min_val = min.into_int_value();
        min_val.set_name("min");

        arg_num += 1;
        assert!(stack_check_func.get_nth_param(arg_num).is_some());
        let max = stack_check_func.get_nth_param(arg_num).unwrap();
        let max_val = max.into_int_value();
        max_val.set_name("max");


        arg_num += 1;
        assert!(stack_check_func.get_nth_param(arg_num).is_some());
        let bb_size = stack_check_func.get_nth_param(arg_num).unwrap();
        let bb_stack_size_val = bb_size.into_int_value();
        bb_stack_size_val.set_name("bb_stack_size");


        arg_num += 1;
        assert!(stack_check_func.get_nth_param(arg_num).is_some());
        let jmp_buf = stack_check_func.get_nth_param(arg_num).unwrap();
        let jmp_buf_ptr_val = jmp_buf.into_pointer_value();
        jmp_buf_ptr_val.set_name("jmpBuf");

        let temp_builder = llvm_context.create_builder();
        let stack_check_bb = llvm_context.append_basic_block(&stack_check_func, "stack_check");
        let stack_update_bb = llvm_context.append_basic_block(&stack_check_func, "stack_update_");
        let stack_overflow_bb = llvm_context.append_basic_block(&stack_check_func, "stack_overflow");


        let zero_64_t = llvm_context.i64_type().const_zero();
        let max_stack_64_t = llvm_context.i64_type().const_int(EVM_MAX_STACK_SIZE as u64, false);

        temp_builder.position_at_end(&stack_check_bb);
        let size = temp_builder.build_load(size_ptr_val, "size");
        let size_val = size.into_int_value();
        let min_size = temp_builder.build_int_add(size_val, min_val, "size.min");
        let max_size = temp_builder.build_int_add(size_val, max_val, "size.max");
        let min_ok = temp_builder.build_int_compare(IntPredicate::SGE, min_size, zero_64_t, "");
        let max_ok = temp_builder.build_int_compare(IntPredicate::ULE, max_size, max_stack_64_t, "");
        let stack_ok = temp_builder.build_and(min_ok, max_ok, "bStackOk");
        temp_builder.build_conditional_branch(stack_ok, &stack_update_bb, &stack_overflow_bb);

        temp_builder.position_at_end(&stack_update_bb);
        let new_stack_size = temp_builder.build_int_nsw_add(size_val, bb_stack_size_val, "new.stack_size");
        temp_builder.build_store(size_ptr_val, new_stack_size);
        unsafe {
            let sp = temp_builder.build_gep(base_val, &[size_val], "sp");
            temp_builder.build_return(Some(&sp));
        }

        temp_builder.position_at_end(&stack_overflow_bb);
        let func_decl = LLVMIntrinsic::LongJmp.get_intrinsic_declaration(&context, None);
        temp_builder.build_call(func_decl, &[jmp_buf_ptr_val.into()], "");
        temp_builder.build_unreachable();

        stack_check_func
    }

}

#[cfg(test)]
mod runtime_tests {
    use rand::distributions::{Distribution, Uniform};
    //use rand::Rng;
    use super::*;
    use evmjit::compiler::evm_compiler::MainFuncCreator;
    use evmjit::compiler::{ExternalFunctionManager, DeclarationManager};


    #[test]
    fn test_push_and_pop() {
        let jitctx = JITContext::new();
        let decl_factory = ExternalFunctionManager::new(&jitctx);

        let module = jitctx.module();
        let types_instance = jitctx.evm_types();
        let word_type = types_instance.get_word_type();

        // Generate outline of main function needed by 'RuntimeTypeManager
        let main_func = MainFuncCreator::new("main", &jitctx);
        let runtime = RuntimeManager::new(&jitctx, &decl_factory);

        let normal_path_block = main_func.get_entry_bb().get_next_basic_block();
        assert!(normal_path_block != None);
        let normal_block = normal_path_block.unwrap();
        let exception_block = main_func.get_abort_bb();

        let exception_mgr = ExceptionManager::new(&jitctx, &normal_block, exception_block);
        let mut op_stack = OperandStack::new(&jitctx, &runtime, &exception_mgr);

        let mut rng = rand::thread_rng();
        let val_range = Uniform::from(1u64..std::u64::MAX);
        let val1 = val_range.sample(&mut rng);
        let val2 = val_range.sample(&mut rng);
        let val3 = val_range.sample(&mut rng);
        let val4 = val_range.sample(&mut rng);

        let val1_const = word_type.const_int(val1, false);
        let val2_const = word_type.const_int(val2, false);
        let val3_const = word_type.const_int(val3, false);
        let val4_const = word_type.const_int(val4, false);

        op_stack.push(val1_const);
        op_stack.push(val2_const);
        op_stack.push(val3_const);
        op_stack.push(val4_const);

        assert_eq!(op_stack.num_elements(), 4);

        let val4_pop_val = op_stack.pop();
        assert_eq!(val4_pop_val, val4_const);

        let val3_pop_val = op_stack.pop();
        assert_eq!(val3_pop_val, val3_const);

        let val2_pop_val = op_stack.pop();
        assert_eq!(val2_pop_val, val2_const);

        let val1_pop_val = op_stack.pop();
        assert_eq!(val1_pop_val, val1_const);

        module.print_to_stderr();
    }

    #[test]
    fn test_dup() {
        let jitctx = JITContext::new();
        let decl_factory = ExternalFunctionManager::new(&jitctx);

        let types_instance = jitctx.evm_types();
        let word_type = types_instance.get_word_type();

        // Generate outline of main function needed by 'RuntimeTypeManager
        let main_func = MainFuncCreator::new("main", &jitctx);
        let runtime = RuntimeManager::new(&jitctx, &decl_factory);

        let normal_path_block = main_func.get_entry_bb().get_next_basic_block();
        assert!(normal_path_block != None);
        let normal_block = normal_path_block.unwrap();
        let exception_block = main_func.get_abort_bb();

        let exception_mgr = ExceptionManager::new(&jitctx, &normal_block, exception_block);
        let mut op_stack = OperandStack::new(&jitctx, &runtime, &exception_mgr);

        let mut rng = rand::thread_rng();
        let val_range = Uniform::from(1u64..std::u64::MAX);
        let val1 = val_range.sample(&mut rng);
        let val2 = val_range.sample(&mut rng);
        let val3 = val_range.sample(&mut rng);
        let val4 = val_range.sample(&mut rng);

        let val1_const = word_type.const_int(val1, false);
        let val2_const = word_type.const_int(val2, false);
        let val3_const = word_type.const_int(val3, false);
        let val4_const = word_type.const_int(val4, false);

        op_stack.push(val1_const);
        op_stack.push(val2_const);
        op_stack.push(val3_const);
        op_stack.push(val4_const);

        op_stack.dup(2);

        assert_eq!(op_stack.num_elements(), 5);
        let index2_dup_pop_val = op_stack.pop();
        assert_eq!(index2_dup_pop_val, val2_const);

        assert_eq!(op_stack.num_elements(), 4);

    }

    #[test]
    fn test_swap() {
        let jitctx = JITContext::new();
        let decl_factory = ExternalFunctionManager::new(&jitctx);

        let types_instance = jitctx.evm_types();
        let word_type = types_instance.get_word_type();

        // Generate outline of main function needed by 'RuntimeTypeManager
        let main_func = MainFuncCreator::new("main", &jitctx);
        let runtime = RuntimeManager::new(&jitctx, &decl_factory);

        let normal_path_block = main_func.get_entry_bb().get_next_basic_block();
        assert!(normal_path_block != None);
        let normal_block = normal_path_block.unwrap();
        let exception_block = main_func.get_abort_bb();

        let exception_mgr = ExceptionManager::new(&jitctx, &normal_block, exception_block);
        let mut op_stack = OperandStack::new(&jitctx, &runtime, &exception_mgr);

        let mut rng = rand::thread_rng();
        let val_range = Uniform::from(1u64..std::u64::MAX);
        let val1 = val_range.sample(&mut rng);
        let val2 = val_range.sample(&mut rng);
        let val3 = val_range.sample(&mut rng);
        let val4 = val_range.sample(&mut rng);

        let val1_const = word_type.const_int(val1, false);
        let val2_const = word_type.const_int(val2, false);
        let val3_const = word_type.const_int(val3, false);
        let val4_const = word_type.const_int(val4, false);



        op_stack.push(val1_const);
        op_stack.push(val2_const);
        op_stack.push(val3_const);
        op_stack.push(val4_const);

        assert_eq!(op_stack.num_elements(), 4);

        let val4_pop_val = op_stack.pop();
        assert_eq!(val4_pop_val, val4_const);

        assert_eq!(op_stack.num_elements(), 3);

        op_stack.push(val4_const);


        // Simulate a swap3, Swap the top of the stack with third item on stack
        op_stack.swap(2);
        let pop_top_val = op_stack.pop();
        assert_eq!(pop_top_val, val2_const);

        op_stack.pop();
        let pop_temp_val = op_stack.pop();
        assert_eq!(pop_temp_val, val4_const);
    }

    #[test]
    fn test_finalize() {
        let jitctx = JITContext::new();
        let builder = jitctx.builder();
        let decl_factory = ExternalFunctionManager::new(&jitctx);
        let context = jitctx.llvm_context();
        let module = jitctx.module();
        let types_instance = jitctx.evm_types();
        let word_type = types_instance.get_word_type();

        // Generate outline of main function needed by 'RuntimeTypeManager
        let main_func_creator = MainFuncCreator::new("main", &jitctx);
        let runtime = RuntimeManager::new(&jitctx, &decl_factory);

        let normal_path_block = main_func_creator.get_entry_bb().get_next_basic_block();
        assert!(normal_path_block != None);
        let normal_block = normal_path_block.unwrap();
        let exception_block = main_func_creator.get_abort_bb();

        let exception_mgr = ExceptionManager::new(&jitctx, &normal_block, exception_block);
        let mut op_stack = OperandStack::new(&jitctx, &runtime, &exception_mgr);

        let mut rng = rand::thread_rng();
        let val_range = Uniform::from(1u64..std::u64::MAX);
        let val1 = val_range.sample(&mut rng);
        let val2 = val_range.sample(&mut rng);
        let val3 = val_range.sample(&mut rng);
        let val4 = val_range.sample(&mut rng);
        let val5 = val_range.sample(&mut rng);

        let val1_const = word_type.const_int(val1, false);
        let val2_const = word_type.const_int(val2, false);
        let val3_const = word_type.const_int(val3, false);
        let val4_const = word_type.const_int(val4, false);
        let val5_const = word_type.const_int(val5, false);

        let main_func = main_func_creator.get_main_func();
        let compile_bb = context.append_basic_block(&main_func, "compile_block");

        builder.position_at_end(&compile_bb);

        let malloc_func = decl_factory.get_decl("malloc");

        let malloc_size = (types_instance.get_word_type().get_bit_width() / 8) * EVM_MAX_STACK_SIZE;
        let malloc_size_ir_value = context.i64_type().const_int(malloc_size as u64, false);
        let _base = builder.build_call(malloc_func, &[malloc_size_ir_value.into()], "stack_base");

        op_stack.push(val1_const);
        op_stack.push(val2_const);
        op_stack.push(val3_const);
        op_stack.push(val4_const);

        let _val1 = op_stack.pop();
        let _val2 = op_stack.pop();

        op_stack.push(val5_const);
        op_stack.finalize_stack();

        module.print_to_stderr();

    }
}

