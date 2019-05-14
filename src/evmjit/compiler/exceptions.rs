#![allow(dead_code)]

use inkwell::basic_block::BasicBlock;
use inkwell::values::PointerValue;
use inkwell::IntPredicate;

use super::intrinsics::LLVMIntrinsic;
use super::intrinsics::LLVMIntrinsicManager;
use super::JITContext;

pub struct ExceptionManager {
    exception_dest: PointerValue,
}

impl ExceptionManager {
    pub fn new(context: &JITContext, normal_path_bb: &BasicBlock, exception_bb: &BasicBlock) -> ExceptionManager {
        let builder = context.builder();
        let llvm_ctx = context.llvm_context();
        let types_instance = context.evm_types();
        let buf_size = llvm_ctx.i64_type().const_int(3, false);
        let byte_ptr_t = types_instance.get_byte_ptr_type();
        let setjmp_words = builder.build_array_alloca(byte_ptr_t, buf_size, "jmpbuf.words");

        let frame_addr_decl = LLVMIntrinsic::FrameAddress.get_intrinsic_declaration(context, None);

        let i32_zero = llvm_ctx.i32_type().const_int(0, false);

        // Save frame pointer
        let fp = builder.build_call(frame_addr_decl, &[i32_zero.into()], "fp");
        let fp_result_as_basic_val = fp.try_as_basic_value().left().unwrap();
        builder.build_store(setjmp_words, fp_result_as_basic_val);

        let stack_save_decl = LLVMIntrinsic::StackSave.get_intrinsic_declaration(context, None);

        // Save stack pointer
        let sp = builder.build_call(stack_save_decl, &[], "sp");
        let sp_result_as_basic_val = sp.try_as_basic_value().left().unwrap();

        unsafe {
            let i64_two = llvm_ctx.i64_type().const_int(2, false);
            let jmp_buf_sp = builder.build_in_bounds_gep(setjmp_words, &[i64_two.into()], "jmpBuf.sp");
            builder.build_store(jmp_buf_sp, sp_result_as_basic_val);

            let setjmp_decl = LLVMIntrinsic::SetJmp.get_intrinsic_declaration(context, None);

            let jmp_buf = builder.build_pointer_cast(setjmp_words, byte_ptr_t, "jmpBuf");
            let setjmp_result = builder.build_call(setjmp_decl, &[jmp_buf.into()], "");
            let setjmp_result_as_int_val = setjmp_result.try_as_basic_value().left().unwrap().into_int_value();

            let normal_path = builder.build_int_compare(IntPredicate::EQ, setjmp_result_as_int_val, i32_zero, "");
            builder.build_conditional_branch(normal_path, &normal_path_bb, &exception_bb);

            ExceptionManager {
                exception_dest: jmp_buf,
            }
        }
    }

    pub fn get_exception_dest(&self) -> PointerValue {
        self.exception_dest
    }
}

#[cfg(test)]
mod tests {
    use inkwell::values::BasicValue;
    use inkwell::values::InstructionOpcode;

    use super::*;
    use evmjit::compiler::evm_compiler::MainFuncCreator;
    use evmjit::compiler::{ExternalFunctionManager, DeclarationManager};
    use evmjit::compiler::runtime::RuntimeManager;
    use evmjit::GetOperandBasicBlock;
    use evmjit::GetOperandValue;

    #[test]
    fn test_exception_manager() {
        let jitctx = JITContext::new();
        let decl_factory = ExternalFunctionManager::new(&jitctx);

        let context = jitctx.llvm_context();
        let module = jitctx.module();
        let builder = jitctx.builder();

        // Generate outline of main function needed by 'RuntimeTypeManager
        let main_func = MainFuncCreator::new("main", &jitctx);
        let _runtime = RuntimeManager::new(&jitctx, &decl_factory);

        let normal_path_block = main_func.get_entry_bb().get_next_basic_block();
        assert!(normal_path_block != None);
        let normal_block = normal_path_block.unwrap();
        let exception_block = main_func.get_abort_bb();

        // Create a basic block to put exception handler code in so we can test it independently
        let main_fn_optional = module.get_function("main");
        assert!(main_fn_optional != None);
        let main_fn = main_fn_optional.unwrap();
        let exception_handler_bb = context.append_basic_block(&main_fn, "exception_handler_bb");

        builder.position_at_end(&exception_handler_bb);

        let _exception_mgr = ExceptionManager::new(&jitctx, &normal_block, exception_block);

        //module.print_to_stderr();

        assert!(exception_handler_bb.get_first_instruction() != None);

        // alloca i8*, i64 3
        let first_insn = exception_handler_bb.get_first_instruction().unwrap();
        assert_eq!(first_insn.get_opcode(), InstructionOpcode::Alloca);
        assert_eq!(first_insn.get_num_operands(), 1);
        let alloca_operand0 = first_insn.get_operand_value(0).unwrap();
        assert!(alloca_operand0.is_int_value());
        assert_eq!(alloca_operand0.into_int_value(), context.i64_type().const_int(3, false));

        assert!(first_insn.get_next_instruction() != None);
        let second_insn = first_insn.get_next_instruction().unwrap();
        assert_eq!(second_insn.get_opcode(), InstructionOpcode::Call);
        assert_eq!(second_insn.get_num_operands(), 2);

        let call_operand0 = second_insn.get_operand_value(0).unwrap();
        assert!(call_operand0.is_int_value());
        assert_eq!(call_operand0.into_int_value(), context.i32_type().const_int(0, false));

        assert!(second_insn.get_next_instruction() != None);
        let third_insn = second_insn.get_next_instruction().unwrap();
        assert_eq!(third_insn.get_opcode(), InstructionOpcode::Store);
        assert_eq!(third_insn.get_num_operands(), 2);

        let store_operand0 = third_insn.get_operand_value(0).unwrap();
        assert!(store_operand0.is_pointer_value());
        let store_operand0_ptr_elt_t = store_operand0.into_pointer_value().get_type().get_element_type();
        assert!(store_operand0_ptr_elt_t.is_int_type());
        assert_eq!(store_operand0_ptr_elt_t.into_int_type(), context.i8_type());

        let store_operand1 = third_insn.get_operand_value(1).unwrap().as_basic_value_enum();
        assert!(store_operand1.is_pointer_value());
        let store_operand1_ptr_elt_t = store_operand1.into_pointer_value().get_type().get_element_type();
        assert!(store_operand1_ptr_elt_t.is_pointer_type());
        let store_operand1_ptr_to_ptr_elt_t = store_operand1_ptr_elt_t.into_pointer_type().get_element_type();
        assert!(store_operand1_ptr_to_ptr_elt_t.is_int_type());
        assert_eq!(store_operand1_ptr_to_ptr_elt_t.into_int_type(), context.i8_type());

        assert!(third_insn.get_next_instruction() != None);
        let fourth_insn = third_insn.get_next_instruction().unwrap();
        assert_eq!(fourth_insn.get_opcode(), InstructionOpcode::Call);
        assert_eq!(fourth_insn.get_num_operands(), 1);

        assert!(fourth_insn.get_next_instruction() != None);

        // getelementptr inbounds i8*, i8** %jmpbuf.words, i64 2

        let fifth_insn = fourth_insn.get_next_instruction().unwrap();
        assert_eq!(fifth_insn.get_opcode(), InstructionOpcode::GetElementPtr);
        assert_eq!(fifth_insn.get_num_operands(), 2);

        let gep_operand0 = fifth_insn.get_operand_value(0).unwrap();
        assert!(gep_operand0.is_pointer_value());
        let gep_operand0_ptr_elt_t = gep_operand0.into_pointer_value().get_type().get_element_type();
        assert!(gep_operand0_ptr_elt_t.is_pointer_type());
        let gep_operand0_ptr_to_ptr_elt_t = gep_operand0_ptr_elt_t.into_pointer_type().get_element_type();
        assert!(gep_operand0_ptr_to_ptr_elt_t.is_int_type());
        assert_eq!(gep_operand0_ptr_to_ptr_elt_t.into_int_type(), context.i8_type());

        let gep_operand1 = fifth_insn.get_operand_value(1).unwrap();
        assert!(gep_operand1.is_int_value());
        assert_eq!(gep_operand1.into_int_value(), context.i64_type().const_int(2, false));

        assert!(fifth_insn.get_next_instruction() != None);

        // store i8* %sp, i8** %jmpBuf.sp
        let sixth_insn = fifth_insn.get_next_instruction().unwrap();
        assert_eq!(sixth_insn.get_opcode(), InstructionOpcode::Store);
        assert_eq!(sixth_insn.get_num_operands(), 2);

        let sixth_insn_store_operand0 = sixth_insn.get_operand_value(0).unwrap();
        assert!(sixth_insn_store_operand0.is_pointer_value());
        let sixth_insn_store_operand0_ptr_elt_t = sixth_insn_store_operand0
            .into_pointer_value()
            .get_type()
            .get_element_type();
        assert!(sixth_insn_store_operand0_ptr_elt_t.is_int_type());
        assert_eq!(sixth_insn_store_operand0_ptr_elt_t.into_int_type(), context.i8_type());

        let sixth_insn_store_operand1 = sixth_insn.get_operand_value(1).unwrap();
        assert!(sixth_insn_store_operand1.is_pointer_value());
        let sixth_insn_store_operand1_ptr_elt_t = sixth_insn_store_operand1
            .into_pointer_value()
            .get_type()
            .get_element_type();
        assert!(sixth_insn_store_operand1_ptr_elt_t.is_pointer_type());
        let sixth_insn_store_operand1_ptr_to_ptr_elt_t = sixth_insn_store_operand1_ptr_elt_t
            .into_pointer_type()
            .get_element_type();
        assert!(sixth_insn_store_operand1_ptr_to_ptr_elt_t.is_int_type());
        assert_eq!(
            sixth_insn_store_operand1_ptr_to_ptr_elt_t.into_int_type(),
            context.i8_type()
        );

        assert!(sixth_insn.get_next_instruction() != None);

        // bitcast i8** %jmpbuf.words to i8*

        let seventh_insn = sixth_insn.get_next_instruction().unwrap();
        assert_eq!(seventh_insn.get_opcode(), InstructionOpcode::BitCast);
        assert_eq!(seventh_insn.get_num_operands(), 1);

        let bitcast_operand0 = seventh_insn.get_operand_value(0).unwrap();
        assert!(bitcast_operand0.is_pointer_value());
        let bitcast_operand0_ptr_elt_t = bitcast_operand0.into_pointer_value().get_type().get_element_type();
        assert!(bitcast_operand0_ptr_elt_t.is_pointer_type());

        let bitcast_operand0_ptr_to_ptr_elt_t = bitcast_operand0_ptr_elt_t.into_pointer_type().get_element_type();
        assert_eq!(bitcast_operand0_ptr_to_ptr_elt_t.into_int_type(), context.i8_type());

        assert!(seventh_insn.get_next_instruction() != None);
        let eighth_insn = seventh_insn.get_next_instruction().unwrap();
        assert_eq!(eighth_insn.get_opcode(), InstructionOpcode::Call);
        assert_eq!(eighth_insn.get_num_operands(), 2);

        // call i32 @llvm.eh.sjlj.setjmp(i8* %jmpBuf)

        let eighth_insn_call_operand0 = eighth_insn.get_operand_value(0).unwrap();
        assert!(eighth_insn_call_operand0.is_pointer_value());
        let eighth_insn_call_operand0_ptr_elt_t = eighth_insn_call_operand0
            .into_pointer_value()
            .get_type()
            .get_element_type();
        assert!(eighth_insn_call_operand0_ptr_elt_t.is_int_type());
        assert_eq!(eighth_insn_call_operand0_ptr_elt_t.into_int_type(), context.i8_type());

        assert!(eighth_insn.get_next_instruction() != None);

        // icmp eq i32 %2, 0
        let ninth_insn = eighth_insn.get_next_instruction().unwrap();
        assert_eq!(ninth_insn.get_opcode(), InstructionOpcode::ICmp);
        assert_eq!(ninth_insn.get_num_operands(), 2);

        let icmp_operand0 = ninth_insn.get_operand_value(0).unwrap();
        assert!(icmp_operand0.is_int_value());

        let icmp_operand1 = ninth_insn.get_operand_value(1).unwrap();
        assert!(icmp_operand1.is_int_value());
        assert_eq!(icmp_operand1.into_int_value(), context.i32_type().const_int(0, false));

        // br i1 %3, label %Stop, label %Abort
        assert!(ninth_insn.get_next_instruction() != None);
        let tenth_insn = ninth_insn.get_next_instruction().unwrap();
        assert_eq!(tenth_insn.get_num_operands(), 3);

        let bb1 = tenth_insn.get_operand_as_bb(1).unwrap();
        assert_eq!(bb1.get_name().to_str(), Ok("Abort"));

        let bb2 = tenth_insn.get_operand_as_bb(2).unwrap();
        assert_eq!(bb2.get_name().to_str(), Ok("Stop"));
    }
}
