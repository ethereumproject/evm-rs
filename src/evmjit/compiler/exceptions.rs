#![allow(dead_code)]

use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::builder::Builder;
use inkwell::values::PointerValue;
use inkwell::basic_block::BasicBlock;
use inkwell::IntPredicate;
use evmjit::compiler::evmtypes::EvmTypes;
use evmjit::compiler::intrinsics::LLVMIntrinsic;
use evmjit::compiler::intrinsics::LLVMIntrinsicManager;
use singletonum::Singleton;

struct ExceptionManager {
    exception_dest: PointerValue,
}

impl ExceptionManager {
    pub fn new(context: &Context, builder: &Builder, module: &Module,
               normal_path_bb: BasicBlock, exception_bb: BasicBlock) -> ExceptionManager {

        let types_instance = EvmTypes::get_instance(context);
        let buf_size = context.i64_type().const_int(3, false);
        let byte_ptr_t = types_instance.get_byte_ptr_type();
        let setjmp_words = builder.build_array_alloca(byte_ptr_t, buf_size, "jmpbuf.words");

        let frame_addr_decl = LLVMIntrinsic::FrameAddress.get_intrinsic_declaration(&context,
                                                                                    &module,
                                                                                    None);

        let i32_zero = context.i32_type().const_int(0, false);

        // Save frame pointer
        let fp = builder.build_call (frame_addr_decl, &[i32_zero.into()], "fp");
        let fp_result_as_basic_val = fp.try_as_basic_value().left().unwrap();
        builder.build_store(setjmp_words, fp_result_as_basic_val);

        let stack_save_decl = LLVMIntrinsic::StackSave.get_intrinsic_declaration(&context,
                                                                                 &module,
                                                                                 None);

        // Save stack pointer
        let sp = builder.build_call (stack_save_decl, &[], "sp");
        let sp_result_as_basic_val = sp.try_as_basic_value().left().unwrap();


        unsafe {
            let i64_two = context.i64_type().const_int(2, false);
            let jmp_buf_sp = builder.build_in_bounds_gep(setjmp_words, &[i64_two.into()], "jmpBuf.sp");
            builder.build_store(jmp_buf_sp, sp_result_as_basic_val);

            let setjmp_decl = LLVMIntrinsic::SetJmp.get_intrinsic_declaration(&context,
                                                                              &module,
                                                                              None);
        
            let jmp_buf = builder.build_pointer_cast(setjmp_words, byte_ptr_t, "jmpBuf");
            let setjmp_result = builder.build_call(setjmp_decl, &[jmp_buf.into()], "");
            let setjmp_result_as_int_val = setjmp_result.try_as_basic_value().left().unwrap().into_int_value();

            let normal_path = builder.build_int_compare(IntPredicate::EQ,
                                                        setjmp_result_as_int_val,
                                                        i32_zero, "");
            builder.build_conditional_branch(normal_path, &normal_path_bb, &exception_bb);

            ExceptionManager {
                exception_dest: jmp_buf
            }
        }
    }

    pub fn get_exception_dest(&self) -> PointerValue {
        self.exception_dest
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;
    use super::*;
    use inkwell::values::InstructionOpcode;
    use evmjit::compiler::evm_compiler::MainFuncCreator;

    #[test]
    fn test_exception_manager() {
        let context = Context::create();
        let module = context.create_module("my_module");
        let builder = context.create_builder();

        // Generate outline of main function needed by 'RuntimeTypeManager
        MainFuncCreator::new ("main", &context, &builder, &module);

    }
}
