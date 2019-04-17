#![allow(dead_code)]

use inkwell::basic_block::BasicBlock;
use inkwell::module::Linkage::*;
use inkwell::values::FunctionValue;

use super::JITContext;

pub struct MainFuncCreator {
    m_main_func: FunctionValue,
    m_jumptable_bb: BasicBlock,
    m_entry_bb: BasicBlock,
    m_stop_bb: BasicBlock,
    m_abort_bb: BasicBlock,
}

impl MainFuncCreator {
    pub fn new(name: &str, context: &JITContext) -> MainFuncCreator {
        let llvm_ctx = context.llvm_context();
        let builder = context.builder();
        let module = context.module();
        let types_instance = context.evm_types();

        let main_ret_type = types_instance.get_contract_return_type();
        let arg1 = context.rt().get_ptr_type();

        let main_func_type = main_ret_type.fn_type(&[arg1.into()], false);
        let main_func = module.add_function(name, main_func_type, Some(External));
        main_func.get_first_param().unwrap().into_pointer_value().set_name("rt");

        let entry_bb = llvm_ctx.append_basic_block(&main_func, "Entry");
        let stop_bb = llvm_ctx.append_basic_block(&main_func, "Stop");
        let jumptable_bb = llvm_ctx.append_basic_block(&main_func, "JumpTable");
        let abort_bb = llvm_ctx.append_basic_block(&main_func, "Abort");

        builder.position_at_end(&jumptable_bb);
        let target = builder.build_phi(types_instance.get_word_type(), "target");
        builder.build_switch(*target.as_basic_value().as_int_value(), &abort_bb, &[]);
        builder.position_at_end(&entry_bb);

        MainFuncCreator {
            m_main_func: main_func,
            m_jumptable_bb: jumptable_bb,
            m_entry_bb: entry_bb,
            m_stop_bb: stop_bb,
            m_abort_bb: abort_bb,
        }
    }

    pub fn get_main_func(&self) -> FunctionValue {
        self.m_main_func
    }

    pub fn get_jumptable_bb(&self) -> &BasicBlock {
        &self.m_jumptable_bb
    }

    pub fn get_entry_bb(&self) -> &BasicBlock {
        &self.m_entry_bb
    }

    pub fn get_abort_bb(&self) -> &BasicBlock {
        &self.m_abort_bb
    }
}
