use super::JITContext;
use bigint::Gas;
use eval::cost::G_COPY;
use eval::cost::G_LOGDATA;
use eval::cost::G_SHA3WORD;
use evmjit::compiler::intrinsics::LLVMIntrinsic;
use evmjit::compiler::intrinsics::LLVMIntrinsicManager;
use evmjit::ModuleLookup;
use inkwell::basic_block::BasicBlock;
use inkwell::types::BasicTypeEnum;
use inkwell::values::IntValue;
use inkwell::IntPredicate;
use patch::Patch;
use std::marker::PhantomData;

pub struct VariableGasCostCalculator<'a, P: Patch> {
    m_context: &'a JITContext,
    _marker: PhantomData<P>,
}

impl<'a, P: Patch> VariableGasCostCalculator<'a, P> {
    pub fn new(context: &'a JITContext) -> VariableGasCostCalculator<'a, P> {
        VariableGasCostCalculator {
            m_context: context,
            _marker: PhantomData,
        }
    }

    pub fn sha3_data_cost(&self, sha3_data_length: IntValue) -> IntValue {
        // Compute cost according to formula:
        // let wordd = Gas::from(len) / Gas::from(32u64);
        // let wordr = Gas::from(len) % Gas::from(32u64);
        // Gas::from(G_SHA3) + Gas::from(G_SHA3WORD) * if wordr == Gas::zero() { wordd } else { wordd + Gas::from(1u64) }

        // We computed G_SHA3 in our fixed cost calcualtion already so we exclude it here
        let types_instance = self.m_context.evm_types();
        let builder = self.m_context.builder();
        let context = self.m_context.llvm_context();
        let gas_type = types_instance.get_gas_type();

        let data_length64 = builder.build_int_truncate(sha3_data_length, gas_type, "data_length");
        let const_five = context.i64_type().const_int(5, false);
        let const_thirty_one = context.i64_type().const_int(31, false);
        let const_zero = context.i64_type().const_zero();
        let const_sha3_word = context.i64_type().const_int(G_SHA3WORD as u64, false);
        // Use logical right shift to divide by 32
        let wordd = builder.build_right_shift(data_length64, const_five, false, "");

        // Use length & 31 to compute mod 32

        let wordrr = builder.build_and(data_length64, const_thirty_one, "");
        let cmp_res = builder.build_int_compare(IntPredicate::NE, wordrr, const_zero, "");

        let add_rem = builder.build_int_z_extend(cmp_res, context.i64_type(), "");
        let multiplier_factor = builder.build_int_nuw_add(wordd, add_rem, "");
        let sha3_data_cost = builder.build_int_nuw_mul(multiplier_factor, const_sha3_word, "");
        builder.build_return(Some(&sha3_data_cost));
        sha3_data_cost
    }

    pub fn log_data_cost(&self, log_data_length: IntValue) -> IntValue {
        let types_instance = self.m_context.evm_types();
        let builder = self.m_context.builder();
        let context = self.m_context.llvm_context();
        let gas_type = types_instance.get_gas_type();

        let data_length64 = builder.build_int_truncate(log_data_length, gas_type, "data_length");
        let log_data_const64 = context.i64_type().const_int(G_LOGDATA as u64, false);
        let log_variable_cost = builder.build_int_nuw_mul(data_length64, log_data_const64, "");
        builder.build_return(Some(&log_variable_cost));
        log_variable_cost
    }

    // Use to compute variable cost for CALLDATACOPY. CODECOPY, RETURNDATACOPY and EXTCODECOPY
    //
    pub fn copy_data_cost(&self, copy_length: IntValue) -> IntValue {
        // Compute cost according to formula:
        // let wordd = Gas::from(len) / Gas::from(32u64);
        // let wordr = Gas::from(len) % Gas::from(32u64);
        // Gas::from(G_COPY) * if wordr == Gas::zero() { wordd } else { wordd + Gas::from(1u64) }

        // We computed the approprate fixed constant in our fixed cost calculation already so we exclude it here
        let types_instance = self.m_context.evm_types();
        let builder = self.m_context.builder();
        let context = self.m_context.llvm_context();
        let gas_type = types_instance.get_gas_type();

        let data_length64 = builder.build_int_truncate(copy_length, gas_type, "data_length");
        let const_five = context.i64_type().const_int(5, false);
        let const_thirty_one = context.i64_type().const_int(31, false);
        let const_zero = context.i64_type().const_zero();
        let const_copy_cost = context.i64_type().const_int(G_COPY as u64, false);
        // Use logical right shift to divide by 32
        let wordd = builder.build_right_shift(data_length64, const_five, false, "");

        // Use length & 31 to compute mod 32

        let wordrr = builder.build_and(data_length64, const_thirty_one, "");
        let cmp_res = builder.build_int_compare(IntPredicate::NE, wordrr, const_zero, "");

        let add_rem = builder.build_int_z_extend(cmp_res, context.i64_type(), "");
        let multiplier_factor = builder.build_int_nuw_add(wordd, add_rem, "");
        let copy_data_cost = builder.build_int_nuw_mul(multiplier_factor, const_copy_cost, "");
        builder.build_return(Some(&copy_data_cost));
        copy_data_cost
    }

    pub fn exp_cost(&self, current_block: &BasicBlock, exponent: IntValue) -> IntValue {
        let types_instance = self.m_context.evm_types();
        let module = self.m_context.module();
        let builder = self.m_context.builder();
        let context = self.m_context.llvm_context();
        let word_type = types_instance.get_word_type();
        let enum_word_type: BasicTypeEnum = BasicTypeEnum::IntType(word_type);
        let gas_type = types_instance.get_gas_type();
        let zero_val256 = context.custom_width_int_type(256).const_zero();

        // We generate this code in the main function for the contract so find it

        let main_func_opt = module.get_main_function(builder);
        assert!(main_func_opt != None);

        let _main_func = main_func_opt.unwrap();
        let exp_exit_bb = context.insert_basic_block_after(current_block, "");
        let exp_cost_calc_bb = context.insert_basic_block_after(&exp_exit_bb, "");

        //let exp_exit_bb = main_func.append_basic_block("");
        //let exp_cost_calc_bb = main_func.append_basic_block("");
        //let exp_entry_block = main_func.append_basic_block("exp_gas_calc");

        //self.m_builder.position_at_end(&exp_entry_block);

        // Check exponent for zero and return zero if true, otherwise calcuate cost
        let zero_compare = builder.build_int_compare(IntPredicate::EQ, exponent, zero_val256, "");
        //assert!(zero_compare.as_instruction_value() != None);

        //let zero_compare_bb_opt = zero_compare.as_instruction_value().unwrap().get_parent();
        //assert!(zero_compare_bb_opt != None);
        // let zero_compare_bb = zero_compare_bb_opt.unwrap();

        builder.build_conditional_branch(zero_compare, &exp_exit_bb, &exp_cost_calc_bb);
        builder.position_at_end(&exp_cost_calc_bb);

        // Formula for exponent calculation is:
        //  Gas::from(G_EXP) + P::gas_expbyte() * (Gas::from(1u64) + Gas::from(log2floor(exponent)) / Gas::from(8u64))
        // IMPORTANT NOTE: We have already accounted for G_EXP in FixedGasCostCalculator so we do not recalcuate it
        // here

        // Use the forumula (sizeof (evm data type) - ctlz (exponent) -1) as a shortcut for log2
        // where sizeof (evm data type) = 256
        // ctlz = count leading zeros

        // Get declaration of ctlz
        let ctlz_decl = LLVMIntrinsic::Ctlz.get_intrinsic_declaration(&self.m_context, Some(enum_word_type));
        let lz256 = builder.build_call(
            ctlz_decl,
            &[exponent.into(), context.bool_type().const_zero().into()],
            "lz256",
        );
        let val = lz256.try_as_basic_value().left().unwrap().into_int_value();
        let lz = builder.build_int_truncate(val, gas_type, "lz");

        let temp1 = context.i64_type().const_int(256, false);
        let sig_bits = builder.build_int_sub(temp1, lz, "sigBits");

        let one = context.i64_type().const_int(1, false);
        let log2val = builder.build_int_sub(sig_bits, one, "log2");

        // Divide by 8 using logical right shift by 3

        let const_three = context.i64_type().const_int(3, false);
        let log_div_8 = builder.build_right_shift(log2val, const_three, false, "");

        let const_one = context.i64_type().const_int(1, false);

        let add_temp1 = builder.build_int_add(log_div_8, const_one, "");
        let expbyte = P::gas_expbyte().as_u64();

        let expbyte_ir = context.i64_type().const_int(expbyte, false);

        let exp_variable_cost = builder.build_int_nuw_mul(add_temp1, expbyte_ir, "");

        builder.build_unconditional_branch(&exp_exit_bb);

        builder.position_at_end(&exp_exit_bb);
        let phi_join = builder.build_phi(context.i64_type(), "exp_phi");

        let zero_val64 = context.i64_type().const_zero();

        phi_join.add_incoming(&[(&exp_variable_cost, &exp_cost_calc_bb), (&zero_val64, &current_block)]);

        builder.build_return(Some(&phi_join.as_basic_value().into_int_value()));

        phi_join.as_basic_value().into_int_value()
    }
}

fn native_log_base2(gas_val: Gas) -> usize {
    gas_val.log2floor()
}

#[cfg(test)]
mod tests {
    use super::*;
    use evmjit::compiler::evm_compiler::MainFuncCreator;
    use evmjit::compiler::external_declarations::ExternalFunctionManager;
    use evmjit::compiler::intrinsics::LLVMIntrinsic;
    use evmjit::compiler::intrinsics::LLVMIntrinsicManager;
    use evmjit::compiler::runtime::RuntimeManager;
    use inkwell::execution_engine::{ExecutionEngine, FunctionLookupError, JitFunction};
    use inkwell::types::BasicTypeEnum;
    use inkwell::OptimizationLevel;
    use patch::EmbeddedPatch;

    type Log2Func = unsafe extern "C" fn(u64) -> u64;

    fn jit_compile_log2(
        jitctx: &JITContext,
        execution_engine: &ExecutionEngine,
    ) -> Result<JitFunction<Log2Func>, FunctionLookupError> {
        let module = jitctx.module();
        let builder = jitctx.builder();
        let context = jitctx.llvm_context();
        let types_instance = jitctx.evm_types();
        let word_type = types_instance.get_word_type();
        let enum_word_type: BasicTypeEnum = BasicTypeEnum::IntType(word_type);
        let gas_type = types_instance.get_gas_type();

        let i64_type = context.i64_type();
        let fn_type = i64_type.fn_type(&[gas_type.into()], false);

        // Get declaration of ctlz
        let ctlz_decl = LLVMIntrinsic::Ctlz.get_intrinsic_declaration(jitctx, Some(enum_word_type));

        let function = module.add_function("mylog2", fn_type, None);
        let basic_block = context.append_basic_block(&function, "entry");

        builder.position_at_end(&basic_block);

        let x = function.get_nth_param(0).unwrap().into_int_value();
        println!("Parameter value = {:?}", x);

        let x256 = builder.build_int_z_extend(x, word_type, "x256");

        let lz256 = builder.build_call(
            ctlz_decl,
            &[x256.into(), context.bool_type().const_zero().into()],
            "lz256",
        );
        let val = lz256.try_as_basic_value().left().unwrap().into_int_value();
        let lz = builder.build_int_truncate(val, gas_type, "lz");

        let temp1 = context.i64_type().const_int(256, false);
        let sig_bits = builder.build_int_sub(temp1, lz, "sigBits");

        let one = context.i64_type().const_int(1, false);
        let log2val = builder.build_int_sub(sig_bits, one, "log2");

        builder.build_return(Some(&log2val));

        unsafe { execution_engine.get_function::<Log2Func>("mylog2") }
    }

    #[test]
    fn test_exp() {
        let jitctx = JITContext::new();
        let context = jitctx.llvm_context();
        //let types_instance = EvmTypes::get_instance(&context);
        //let word_type = types_instance.get_word_type();
        //let enum_word_type = BasicTypeEnum::IntType(word_type);
        let builder = jitctx.builder();
        let decl_factory = ExternalFunctionManager::new(&jitctx);

        // Need to create main function before TransactionConextManager otherwise we will crash
        let main_func = MainFuncCreator::new("main", &jitctx);

        let _manager = RuntimeManager::new(&jitctx, &decl_factory);
        let entry_bb = main_func.get_entry_bb();

        builder.position_at_end(&entry_bb);

        let gas_calculator: VariableGasCostCalculator<EmbeddedPatch> = VariableGasCostCalculator::new(&jitctx);

        let exponent = context.custom_width_int_type(256).const_int(55, false);
        gas_calculator.exp_cost(entry_bb, exponent);
        //module.print_to_stderr();
    }

    #[test]
    fn test_log_data_cost() {
        let jitctx = JITContext::new();
        let context = jitctx.llvm_context();
        //let types_instance = EvmTypes::get_instance(&context);
        //let word_type = types_instance.get_word_type();
        //let enum_word_type = BasicTypeEnum::IntType(word_type);
        let builder = jitctx.builder();
        let decl_factory = ExternalFunctionManager::new(&jitctx);

        // Need to create main function before TransactionConextManager otherwise we will crash
        let main_func = MainFuncCreator::new("main", &jitctx);

        let _manager = RuntimeManager::new(&jitctx, &decl_factory);
        let entry_bb = main_func.get_entry_bb();

        builder.position_at_end(&entry_bb);

        let gas_calculator: VariableGasCostCalculator<EmbeddedPatch> = VariableGasCostCalculator::new(&jitctx);

        let log_data_length = context.custom_width_int_type(256).const_int(30, false);
        gas_calculator.log_data_cost(log_data_length);
        //module.print_to_stderr();
    }

    #[test]
    fn test_sha3() {
        let jitctx = JITContext::new();
        let context = jitctx.llvm_context();

        //let types_instance = EvmTypes::get_instance(&context);
        //let word_type = types_instance.get_word_type();
        //let enum_word_type = BasicTypeEnum::IntType(word_type);
        let builder = jitctx.builder();
        let decl_factory = ExternalFunctionManager::new(&jitctx);

        // Need to create main function before TransactionConextManager otherwise we will crash
        let main_func = MainFuncCreator::new("main", &jitctx);

        let _manager = RuntimeManager::new(&jitctx, &decl_factory);
        let entry_bb = main_func.get_entry_bb();

        builder.position_at_end(&entry_bb);

        let gas_calculator: VariableGasCostCalculator<EmbeddedPatch> = VariableGasCostCalculator::new(&jitctx);

        let sha3_data_len = context.custom_width_int_type(256).const_int(19, false);
        gas_calculator.sha3_data_cost(sha3_data_len);
        //module.print_to_stderr();
    }

    #[test]
    fn test_copy_data_cost() {
        let jitctx = JITContext::new();
        let context = jitctx.llvm_context();

        //let types_instance = EvmTypes::get_instance(&context);
        //let word_type = types_instance.get_word_type();
        //let enum_word_type = BasicTypeEnum::IntType(word_type);
        let builder = jitctx.builder();
        let decl_factory = ExternalFunctionManager::new(&jitctx);

        // Need to create main function before TransactionConextManager otherwise we will crash
        let main_func = MainFuncCreator::new("main", &jitctx);

        let _manager = RuntimeManager::new(&jitctx, &decl_factory);
        let entry_bb = main_func.get_entry_bb();

        builder.position_at_end(&entry_bb);

        let gas_calculator: VariableGasCostCalculator<EmbeddedPatch> = VariableGasCostCalculator::new(&jitctx);

        let copy_data_len = context.custom_width_int_type(256).const_int(157, false);
        gas_calculator.copy_data_cost(copy_data_len);
        //module.print_to_stderr();
    }

    #[test]

    // This test simultes that we can compute log2 using the formula
    // 256 - ctlz (val) - 1
    // where ctlz is the count leading zero function

    fn test_log2_using_jit() {
        let jitctx = JITContext::new();
        let module = jitctx.module();
        //let types_instance = EvmTypes::get_instance(&context);
        //let word_type = types_instance.get_word_type();
        //let enum_word_type = BasicTypeEnum::IntType(word_type);

        let execution_engine = module.create_jit_execution_engine(OptimizationLevel::None).unwrap();

        let mylog = jit_compile_log2(&jitctx, &execution_engine).unwrap();

        module.print_to_stderr();

        let x = 55u64;

        unsafe {
            assert_eq!(mylog.call(x), 5);
        }
    }
}
