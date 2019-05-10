#![allow(dead_code)]

use evmjit::compiler::external_declarations::ExternalFunctionManager;
use evmjit::compiler::stack::EVM_MAX_STACK_SIZE;
use inkwell::values::BasicValueEnum;
use inkwell::values::PointerValue;

use super::super::JITContext;

#[derive(Debug, Copy, Clone)]
pub struct StackAllocator {
    stack_base: BasicValueEnum,
    stack_size_ptr: PointerValue,
}

impl StackAllocator {
    pub fn new(context: &JITContext, decl_factory: &ExternalFunctionManager) -> StackAllocator {
        let builder = context.builder();
        let types_instance = context.evm_types();

        let malloc_func = decl_factory.get_malloc_decl();

        let malloc_size = (types_instance.get_word_type().get_bit_width() / 8) * EVM_MAX_STACK_SIZE;
        let malloc_size_ir_value = context.llvm_context().i64_type().const_int(malloc_size as u64, false);
        let base = builder.build_call(malloc_func, &[malloc_size_ir_value.into()], "stack_base");

        let size_ptr = builder.build_alloca(types_instance.get_size_type(), "stack.size");
        builder.build_store(size_ptr, context.llvm_context().i64_type().const_zero());

        StackAllocator {
            stack_base: base.try_as_basic_value().left().unwrap(),
            stack_size_ptr: size_ptr,
        }
    }

    pub fn get_stack_base_as_ir_value(&self) -> BasicValueEnum {
        self.stack_base
    }

    pub fn get_stack_size_as_ir_value(&self) -> PointerValue {
        self.stack_size_ptr
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use inkwell::module::Linkage::External;
    use inkwell::values::InstructionOpcode;
    use std::ffi::CString;

    #[test]
    fn test_stack_allocator_new() {
        let jitctx = JITContext::new();
        let context = jitctx.llvm_context();
        let module = jitctx.module();
        let builder = jitctx.builder();
        let decl_factory = ExternalFunctionManager::new(&jitctx);

        // Create dummy function

        let fn_type = context.void_type().fn_type(&[], false);
        let my_fn = module.add_function("my_fn", fn_type, Some(External));
        let entry_bb = context.append_basic_block(&my_fn, "entry");

        builder.position_at_end(&entry_bb);
        StackAllocator::new(&jitctx, &decl_factory);

        let malloc_func_optional = module.get_function("malloc");
        assert!(malloc_func_optional != None);

        let entry_block_optional = my_fn.get_first_basic_block();
        assert!(entry_block_optional != None);
        let entry_block = entry_block_optional.unwrap();
        assert_eq!(*entry_block.get_name(), *CString::new("entry").unwrap());

        assert!(entry_block.get_first_instruction() != None);
        let first_insn = entry_block.get_first_instruction().unwrap();
        assert_eq!(first_insn.get_opcode(), InstructionOpcode::Call);

        assert!(first_insn.get_next_instruction() != None);
        let second_insn = first_insn.get_next_instruction().unwrap();
        assert_eq!(second_insn.get_opcode(), InstructionOpcode::Alloca);

        assert!(second_insn.get_next_instruction() != None);
        let third_insn = second_insn.get_next_instruction().unwrap();
        assert_eq!(third_insn.get_opcode(), InstructionOpcode::Store);

        assert!(third_insn.get_next_instruction() == None);
    }
}