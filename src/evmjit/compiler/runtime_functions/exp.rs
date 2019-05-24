#![allow(dead_code)]

use super::super::JITContext;
use inkwell::values::FunctionValue;
use inkwell::module::Linkage::*;
use inkwell::IntPredicate;
use inkwell::values::IntValue;
use evmjit::compiler::util::funcbuilder::*;

pub struct ExpDeclarationManager<'a> {
    m_context: &'a JITContext,
}

impl<'a> ExpDeclarationManager<'a> {
    pub fn new(context: &'a JITContext) -> ExpDeclarationManager<'a> {
        ExpDeclarationManager {
            m_context: context
        }
    }

    // Create function to calculate b^n; b^n is the product of multiplying n bases

    fn create_exp2_func (&self) -> FunctionValue {
        let func_name = "jit.exp.256";
        let exp_func_found = self.m_context.module().get_function(func_name);

        if exp_func_found.is_some() {
            exp_func_found.unwrap()
        } else {
            let context = self.m_context.llvm_context();
            let module = self.m_context.module();
            let attr_factory = self.m_context.attributes();
            let word_type = self.m_context.evm_types().get_word_type();

            let exp_func_type = FunctionTypeBuilder::new(context)
                .returns(word_type)
                .arg(word_type)
                .arg(word_type)
                .build()
                .unwrap();

            let exp_func = module.add_function(func_name, exp_func_type, Some(Private));

            // Function does not throw
            exp_func.add_attribute(0, *attr_factory.attr_nounwind());

            // Function does not access memory
            exp_func.add_attribute(0, *attr_factory.attr_readnone());

            // Give the function parameters names
            assert!(exp_func.get_nth_param(0).is_some());
            let base = exp_func.get_nth_param(0).unwrap();
            base.into_int_value().set_name("base");
            let base_val = base.into_int_value();

            assert!(exp_func.get_nth_param(1).is_some());
            let exponent = exp_func.get_nth_param(1).unwrap();
            exponent.into_int_value().set_name("exponent");
            let exponent_val = exponent.into_int_value();

            let one = word_type.const_int(1, false);
            let zero = word_type.const_zero();

            let temp_builder = context.create_builder();

            let entry_bb = context.append_basic_block(&exp_func, "exp_entry");
            let loop_body_bb = context.append_basic_block(&exp_func, "loop_body");
            let return_bb = context.append_basic_block(&exp_func, "exp_return");

            temp_builder.position_at_end(&entry_bb);
            let exp_zero = temp_builder.build_int_compare(IntPredicate::EQ, exponent_val, zero, "e.zero");
            temp_builder.build_conditional_branch(exp_zero, &return_bb, &loop_body_bb);

            temp_builder.position_at_end(&loop_body_bb);

            // Initialize SSA Phi nodes

            let res_phi = temp_builder.build_phi(word_type, "r.phi");
            let res_phi_val = res_phi.as_basic_value().into_int_value();

            let exponent_phi = temp_builder.build_phi(word_type, "e.phi");
            let exponent_phi_val = exponent_phi.as_basic_value().into_int_value();

            let base_phi = temp_builder.build_phi(word_type, "b.phi");
            let base_phi_val = base_phi.as_basic_value().into_int_value();


            let and_res = temp_builder.build_and(exponent_phi_val, one, "");

            let e_odd = temp_builder.build_int_compare(IntPredicate::EQ, and_res, zero, "e.isodd");
            let base_sel = temp_builder.build_select(e_odd, one, base_phi_val, "");
            let res_update = temp_builder.build_int_mul(base_sel.into_int_value(), res_phi_val, "");
            let base_update = temp_builder.build_int_mul(base_phi_val, base_phi_val, "");
            let exp_update = temp_builder.build_right_shift(exponent_phi_val, one, false, "");
            let exp_zero_cond = temp_builder.build_int_compare(IntPredicate::EQ, exp_update, zero, "");
            temp_builder.build_conditional_branch(exp_zero_cond, &return_bb, &loop_body_bb);

            res_phi.add_incoming(&[(&res_update, &loop_body_bb), (&one, &entry_bb)]);
            exponent_phi.add_incoming(&[(&exp_update, &loop_body_bb), (&exponent_val, &entry_bb)]);
            base_phi.add_incoming(&[(&base_update, &loop_body_bb), (&base_val, &entry_bb),]);

            temp_builder.position_at_end(&return_bb);
            let return_phi = temp_builder.build_phi(word_type, "r.phi");
            let return_phi_val = return_phi.as_basic_value().into_int_value();

            return_phi.add_incoming(&[(&one, &entry_bb), (&res_update, &loop_body_bb)]);
            temp_builder.build_return(Some(&return_phi_val));

            exp_func
        }
    }

    pub fn exp(&self, base: IntValue, exponent: IntValue) -> IntValue {
        let builder = self.m_context.builder();

        if base.is_const() && exponent.is_const() {
            // Since both base and exponent are constant, calculate exp at compile time

            let word_type = self.m_context.evm_types().get_word_type();
            let one = word_type.const_int(1, false);
            let mut b = base;
            let mut e = exponent;
            let mut r = one;

            let two = word_type.const_int(2, false);
            let mut e_mod;

            // The key here is that the const_ methods will turn this loop
            // into a constant IntVal that we can return

            while e.get_zero_extended_constant() != Some(0u64) {
                e_mod = e.const_unsigned_remainder(two);
                if e_mod.get_zero_extended_constant() == Some(1u64) {
                    r = r.const_mul(b);
                }
                b = b.const_mul(b);
                e = e.const_rshr(one);
            }

            r
        }
        else {
            let exp_res = builder.build_call(self.create_exp2_func(), &[base.into(), exponent.into()], "");

            let exp_val = exp_res.try_as_basic_value().left().unwrap().into_int_value();
            exp_val
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::distributions::{Distribution, Uniform};
    use rand::Rng;
    use inkwell::execution_engine::{ExecutionEngine, FunctionLookupError, JitFunction};
    use inkwell::OptimizationLevel;
    use std::ffi::CString;
    use inkwell::values::InstructionOpcode;
    type ExpTestFunc = unsafe extern "C" fn(u64, u64) -> u64;
    type ExpTestFunc2 = unsafe extern "C" fn() -> u64;


    fn jit_compile_exp256_test(
        jitctx: &JITContext,
        execution_engine: &ExecutionEngine,
    ) -> Result<JitFunction<ExpTestFunc>, FunctionLookupError> {
        let module = jitctx.module();
        let builder = jitctx.builder();
        let context = jitctx.llvm_context();
        let types_instance = jitctx.evm_types();
        let word_type = types_instance.get_word_type();
        let i64_type = context.i64_type();
        let fn_type = i64_type.fn_type(&[i64_type.into(), i64_type.into()], false);

        let function = module.add_function("exp_test", fn_type, None);
        let basic_block = context.append_basic_block(&function, "entry");

        builder.position_at_end(&basic_block);

        let base = function.get_nth_param(0).unwrap().into_int_value();
        let base_256_val = builder.build_int_z_extend(base, word_type, "base.256");

        let exponent = function.get_nth_param(1).unwrap().into_int_value();
        let exponent_256_val = builder.build_int_z_extend(exponent, word_type, "exponent.256");

        let exp_decl = ExpDeclarationManager::new(jitctx);

        let exp_val_256 = exp_decl.exp(base_256_val, exponent_256_val);
        let ret_val_64 = builder.build_int_truncate(exp_val_256, i64_type, "");
        builder.build_return(Some(&ret_val_64));

        unsafe { execution_engine.get_function::<ExpTestFunc>("exp_test") }
    }

    fn jit_compile_exp256_constant_test(
        jitctx: &JITContext,
        execution_engine: &ExecutionEngine,
    ) -> Result<JitFunction<ExpTestFunc2>, FunctionLookupError> {
        let module = jitctx.module();
        let builder = jitctx.builder();
        let context = jitctx.llvm_context();
        let types_instance = jitctx.evm_types();
        let word_type = types_instance.get_word_type();
        let i64_type = context.i64_type();
        let fn_type = i64_type.fn_type(&[], false);

        let function = module.add_function("exp_const_test", fn_type, None);
        let basic_block = context.append_basic_block(&function, "entry");

        builder.position_at_end(&basic_block);

        let base = word_type.const_int(11, false);

        let exponent = word_type.const_int(5, false);

        let exp_decl = ExpDeclarationManager::new(jitctx);

        let exp_val_256 = exp_decl.exp(base, exponent);
        let ret_val_64 = builder.build_int_truncate(exp_val_256, i64_type, "");
        builder.build_return(Some(&ret_val_64));

        unsafe { execution_engine.get_function::<ExpTestFunc2>("exp_const_test") }
    }

    #[test]
    fn test_exp256() {
        let jitctx = JITContext::new();
        let module = jitctx.module();

        let execution_engine = module.create_jit_execution_engine(OptimizationLevel::Default).unwrap();

        let exp_jit_func = jit_compile_exp256_test(&jitctx, &execution_engine).unwrap();

        //module.print_to_stderr();

        let mut rng = rand::thread_rng();
        let base_range = Uniform::from(1u64..500u64);
        let exponent_range = Uniform::from(1u32..5u32);

        let mut base : u64;
        let mut exponent: u32;
        let mut exp_res : u64;

        for _ in 1..500  {
            base = base_range.sample(&mut rng);
            exponent = exponent_range.sample(&mut rng);

            exp_res = base.pow(exponent);
            unsafe {
                assert_eq!(exp_jit_func.call(base, exponent as u64), exp_res);
            }
        }

    }

    #[test]
    fn test_exp256_constant() {
        let jitctx = JITContext::new();
        let module = jitctx.module();

        let execution_engine = module.create_jit_execution_engine(OptimizationLevel::Default).unwrap();

        let exp_jit_func = jit_compile_exp256_constant_test(&jitctx, &execution_engine).unwrap();

        module.print_to_stderr();

        let b = 11u64;
        let e = 5u32;

        unsafe {
            assert_eq!(exp_jit_func.call(), b.pow(e));
        }

        // Verify that the generated test function consists of nothing but a return of
        // the calculated value

        let exp_fn_optional = module.get_function("exp_const_test");
        assert!(exp_fn_optional != None);

        let exp_fn = exp_fn_optional.unwrap();
        assert!(exp_fn.count_params() == 0);

        assert!(exp_fn.get_first_basic_block() != None);
        let entry_block = exp_fn.get_first_basic_block().unwrap();

        assert!(entry_block.get_first_instruction() != None);
        let first_bb_insn = entry_block.get_first_instruction().unwrap();
        assert_eq!(first_bb_insn.get_opcode(), InstructionOpcode::Return);
        assert!(first_bb_insn.get_next_instruction() == None);
    }
}