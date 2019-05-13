#![allow(dead_code)]

use super::super::JITContext;
use inkwell::values::FunctionValue;
use inkwell::module::Linkage::*;
use inkwell::IntPredicate;
use inkwell::types::IntType;
use inkwell::types::BasicTypeEnum;
use evmjit::compiler::intrinsics::LLVMIntrinsic;
use evmjit::compiler::intrinsics::LLVMIntrinsicManager;

pub struct DivModDeclarationManager<'a> {
    m_context: &'a JITContext,
}

impl<'a> DivModDeclarationManager<'a> {
    pub fn new(context: &'a JITContext) -> DivModDeclarationManager<'a> {
        DivModDeclarationManager {
            m_context: context
        }

    }

    pub fn create_udivmod(&self, func_name: &str, func_type: IntType) -> FunctionValue {
        let context = self.m_context.llvm_context();
        let module = self.m_context.module();
        let attr_factory = self.m_context.attributes();

       //let types_instance = self.m_context.evm_types();

        // Return array with quotient and remainder
        let ret_type = func_type.vec_type(2);

        let arg1 = func_type;
        let arg2 = func_type;
        let divmod_func_type = ret_type.fn_type(&[arg1.into(), arg2.into()], false);
        let divmod_func = module.add_function (func_name, divmod_func_type, Some(Private));

        // Function does not throw
        divmod_func.add_attribute(0, *attr_factory.attr_nounwind());

        // Function does not access memory
        divmod_func.add_attribute(0, *attr_factory.attr_readnone());

        assert!(divmod_func.get_nth_param(0).is_some());
        let x = divmod_func.get_nth_param(0).unwrap();
        x.into_int_value().set_name("x");
        let x_val = x.into_int_value();

        assert!(divmod_func.get_nth_param(1).is_some());
        let y = divmod_func.get_nth_param(1).unwrap();
        y.into_int_value().set_name("y");
        let y_val = y.into_int_value();


        let bits = func_type.get_bit_width();
        let llvm_int_type = context.custom_width_int_type(bits);
        let zero = llvm_int_type.const_zero();
        let one = llvm_int_type.const_int(1, false);
        let zero_64_t = context.i64_type().const_zero();
        let one_64_t = context.i64_type().const_int(1, false);

        // Create a temporary builder for divmode256 function

        let temp_builder = context.create_builder();
        let entry_bb = context.append_basic_block(&divmod_func, "divmod_entry");
        let main_bb = context.append_basic_block(&divmod_func, "divmod_main");
        let loop_bb = context.append_basic_block(&divmod_func, "divmod_loop");
        let continue_bb = context.append_basic_block(&divmod_func, "divmod_continue");
        let return_bb = context.append_basic_block(&divmod_func, "divmod_return");

        temp_builder.position_at_end(&entry_bb);
        // In computing quotient = dividend / divisor,
        // If the divisor is > dividend, then return immediately with zero, since we are doing integer division

        let cond_le = temp_builder.build_int_compare(IntPredicate::ULE, y_val, x_val, "");
        let r0 = x_val;
        temp_builder.build_conditional_branch(cond_le, &main_bb, &return_bb);

        // Divisor doubling using count leading zeros and left shift

        temp_builder.position_at_end(&main_bb);
        let ctlz_arg = BasicTypeEnum::IntType(llvm_int_type);
        let ctlz_func = LLVMIntrinsic::Ctlz.get_intrinsic_declaration(self.m_context, Some(ctlz_arg));

        // The second argument (to ctlz) must be a constant and is a flag to indicate whether the intrinsic should ensure that a zero
        // as the first argument produces a defined result.
        let ensure_nonzero = context.bool_type().const_int(1, false);
        let y_ctlz = temp_builder.build_call(ctlz_func, &[y_val.into(), ensure_nonzero.into()], "ctlz.y");
        let r_ctlz = temp_builder.build_call(ctlz_func, &[r0.into(), ensure_nonzero.into()], "ctlz.r");

        let y_ctlz_val = y_ctlz.try_as_basic_value().left().unwrap().into_int_value();
        let r_ctlz_val = r_ctlz.try_as_basic_value().left().unwrap().into_int_value();

        let i0 = temp_builder.build_int_nuw_sub(y_ctlz_val, r_ctlz_val, "i0");
        let y0 = temp_builder.build_left_shift(y_val, i0, "");
        temp_builder.build_unconditional_branch(&loop_bb);

        temp_builder.position_at_end(&loop_bb);
        let y_phi = temp_builder.build_phi(func_type, "y.phi");
        let y_phi_val = y_phi.as_basic_value().into_int_value();

        let r_phi = temp_builder.build_phi(func_type, "r.phi");
        let r_phi_val = r_phi.as_basic_value().into_int_value();

        let i_phi = temp_builder.build_phi(func_type, "i.phi");
        let i_phi_val = i_phi.as_basic_value().into_int_value();

        let q_phi = temp_builder.build_phi(func_type, "q.phi");
        let q_phi_val = q_phi.as_basic_value().into_int_value();

        let r_update = temp_builder.build_int_nuw_sub(r_phi_val, y_phi_val, "");
        let q_update = temp_builder.build_or(q_phi_val, one, "");
        let r_ge_y = temp_builder.build_int_compare(IntPredicate::UGE, r_phi_val, y_phi_val, "");
        let r1 = temp_builder.build_select(r_ge_y, r_update, r_phi_val, "r1");
        let q1 = temp_builder.build_select(r_ge_y, q_update, q_phi_val, "q1");
        let i_zero = temp_builder.build_int_compare(IntPredicate::EQ, i_phi_val, zero, "");
        temp_builder.build_conditional_branch(i_zero, &return_bb, &continue_bb);

        // Continue basic block

        temp_builder.position_at_end(&continue_bb);
        let i2 = temp_builder.build_int_nuw_sub(i_phi_val, one, "");
        let q2 = temp_builder.build_left_shift(q1.into_int_value(), one, "");
        let y2 = temp_builder.build_right_shift(y_phi_val, one, false, "");
        temp_builder.build_unconditional_branch(&loop_bb);

        // Add incoming edge for PHI nodes


        y_phi.add_incoming(&[(&y0, &main_bb), (&y2, &continue_bb)]);
        r_phi.add_incoming(&[(&r0, &main_bb), (&r1, &continue_bb)]);
        i_phi.add_incoming(&[(&i0, &main_bb), (&i2, &continue_bb)]);
        q_phi.add_incoming(&[(&zero, &main_bb), (&q2, &continue_bb)]);

        temp_builder.position_at_end(&return_bb);

        let q_ret = temp_builder.build_phi(func_type, "q.ret");
        q_ret.add_incoming(&[(&zero, &entry_bb), (&q1, &loop_bb)]);
        let q_ret_val = q_ret.as_basic_value().into_int_value();

        let r_ret = temp_builder.build_phi(func_type, "r.ret");
        r_ret.add_incoming(&[(&r0, &entry_bb), (&r1, &loop_bb)]);

        let undef_ret_val = ret_type.get_undef();
        let mut ret = temp_builder.build_insert_element(undef_ret_val, q_ret_val, zero_64_t, "ret0");
        ret = temp_builder.build_insert_element(ret.into_vector_value(), r_ret.as_basic_value().into_int_value(), one_64_t, "ret");
        temp_builder.build_return(Some(&ret));

        divmod_func
    }

    pub fn create_udivmod256_func(&self) -> FunctionValue {
        let func_name = "jit.udivmod.256";
        let divmod256_func_found = self.m_context.module().get_function(func_name);

        if divmod256_func_found.is_some() {
            divmod256_func_found.unwrap()
        }
        else {
            self.create_udivmod(func_name, self.m_context.evm_types().get_word_type())
        }
    }

    pub fn create_udivmod512_func(&self) -> FunctionValue {
        let func_name = "jit.udivmod.512";
        let divmod512_func_found = self.m_context.module().get_function(func_name);

        if divmod512_func_found.is_some() {
            divmod512_func_found.unwrap()
        }
        else {
            let type_512_t = self.m_context.llvm_context().custom_width_int_type(512);
            self.create_udivmod(func_name, type_512_t)
        }
    }

    pub fn create_udiv256_func(&self) -> FunctionValue {
        let func_name = "jit.udiv.256";
        let udiv256_func_found = self.m_context.module().get_function(func_name);

        if udiv256_func_found.is_some() {
            udiv256_func_found.unwrap()
        }
        else {
            let context = self.m_context.llvm_context();
            let module = self.m_context.module();
            let attr_factory = self.m_context.attributes();
            let udivrem_func = self.create_udivmod256_func();
            let word_type = self.m_context.evm_types().get_word_type();
            let ret_type = word_type;

            let udiv_func_type = ret_type.fn_type(&[word_type.into(), word_type.into()], false);
            let udiv256_func = module.add_function (func_name, udiv_func_type, Some(Private));

            // Function does not throw
            udiv256_func.add_attribute(0, *attr_factory.attr_nounwind());

            // Function does not access memory
            udiv256_func.add_attribute(0, *attr_factory.attr_readnone());

            // Give the function parameters names
            assert!(udiv256_func.get_nth_param(0).is_some());
            let dividend = udiv256_func.get_nth_param(0).unwrap();
            dividend.into_int_value().set_name("dividend");
            let dividend_val = dividend.into_int_value();

            assert!(udiv256_func.get_nth_param(1).is_some());
            let divisor = udiv256_func.get_nth_param(1).unwrap();
            divisor.into_int_value().set_name("divisor");
            let divisor_val = divisor.into_int_value();

            let temp_builder = context.create_builder();
            let entry_bb = context.append_basic_block(&udiv256_func, "udiv256_entry");
            temp_builder.position_at_end(&entry_bb);

            // All we need to do is call the udivmod function and extract the division result from element 0

            let udivrem_result = temp_builder.build_call(udivrem_func, &[dividend_val.into(), divisor_val.into()], "");
            let udivrem_val = udivrem_result.try_as_basic_value().left().unwrap().into_vector_value();

            let index = context.i64_type().const_zero();
            let udiv_result = temp_builder.build_extract_element(udivrem_val, index, "");

            temp_builder.build_return(Some(&udiv_result));

            udiv256_func
        }
    }
}

#[cfg(test)]
mod divmod_test {
    use super::*;
    use rand::distributions::{Distribution, Uniform};
    use rand::Rng;
    use inkwell::execution_engine::{ExecutionEngine, FunctionLookupError, JitFunction};
    use inkwell::OptimizationLevel;
    type DivTestFunc = unsafe extern "C" fn(u64, u64) -> u64;

    fn jit_compile_udiv256_test(
        jitctx: &JITContext,
        execution_engine: &ExecutionEngine,
    ) -> Result<JitFunction<DivTestFunc>, FunctionLookupError> {
        let module = jitctx.module();
        let builder = jitctx.builder();
        let context = jitctx.llvm_context();
        let types_instance = jitctx.evm_types();
        let word_type = types_instance.get_word_type();
        let i64_type = context.i64_type();
        let fn_type = i64_type.fn_type(&[i64_type.into(), i64_type.into()], false);

        let function = module.add_function("udiv256_test", fn_type, None);
        let basic_block = context.append_basic_block(&function, "entry");

        builder.position_at_end(&basic_block);

        let x = function.get_nth_param(0).unwrap().into_int_value();
        let x_256_val = builder.build_int_z_extend(x, word_type, "x.256");

        let y = function.get_nth_param(1).unwrap().into_int_value();
        let y_256_val = builder.build_int_z_extend(y, word_type, "x.256");

        let div_decl = DivModDeclarationManager::new(jitctx);

        let udiv_256_func = div_decl.create_udiv256_func();
        let udiv256result = builder.build_call(udiv_256_func, &[x_256_val.into(), y_256_val.into()], "");
        let ret_val_256 = udiv256result.try_as_basic_value().left().unwrap().into_int_value();
        let ret_val_64 = builder.build_int_truncate(ret_val_256, i64_type, "");
        builder.build_return(Some(&ret_val_64));

        unsafe { execution_engine.get_function::<DivTestFunc>("udiv256_test") }
    }

    #[test]
    fn test_udiv256() {
        let jitctx = JITContext::new();
        let module = jitctx.module();

        let execution_engine = module.create_jit_execution_engine(OptimizationLevel::Default).unwrap();

        let udiv_jit_func = jit_compile_udiv256_test(&jitctx, &execution_engine).unwrap();

        module.print_to_stderr();

        let mut rng = rand::thread_rng();
        let divisor_range = Uniform::from(1..std::u64::MAX);

        let mut divisor : u64 = 1;
        let mut dividend: u64 = 1;
        let mut quotient : u64 = 1;

        let mut number = 0;

        while number != 20 {
            println!("{}!", number);
            divisor = divisor_range.sample(&mut rng);
            dividend = rng.gen_range(divisor, std::u64::MAX);
            println!("Test number = {:?}", number);
            println!("Random Divisor = {:?}", divisor);
            println!("Random Dividend = {:?}", dividend);
            quotient  = dividend/divisor;
            unsafe {
                assert_eq!(udiv_jit_func.call(dividend, divisor), quotient);
            }
            number = number + 1;
        }

        for x in 1..20 {
            divisor = divisor_range.sample(&mut rng);
            dividend = rng.gen_range(divisor, std::u64::MAX);
            println!("Test number = {:?}", x);
            println!("Random Divisor = {:?}", divisor);
            println!("Random Dividend = {:?}", dividend);
            quotient  = dividend/divisor;
            unsafe {
                assert_eq!(udiv_jit_func.call(dividend, divisor), quotient);
            }
        }

        let x = 200u64;
        let y = 3u64;

        println!("Divisor = {:?}", x);
        println!(" Dividend = {:?}", y);

        unsafe {
            assert_eq!(udiv_jit_func.call(x, y), x/y);
        }
    }
}
