use bigint::Gas;

fn ctlz_256(gas_val: Gas) -> Gas {
    let mut gas = gas_val;
    let mut n : Gas = Gas::zero();
    //let bits = Gas::from(256u64);
    let gas2 = Gas::one() + Gas::one();

    let mut c : u64 = 0;
    //let loop_limit : u64 = (bits - Gas::one()).into();

    for i in 1..256u64 {
        if gas < Gas::zero() {
            break;
        }

        c = c + i;
        n = n + Gas::one();
        gas = gas * gas2;
    }

    n
}

fn simulate_log_base2(gas_val: Gas) -> usize {

        //let num_leading : Gas = ctlz(gas_val);
        let num_leading : Gas = ctlz_256(gas_val) - Gas::one();
        let const_256 : Gas = From::from(256u64);
        let log2_sim = const_256 - num_leading;
        log2_sim.as_u64() as usize
}

fn native_log_base2(gas_val: Gas) -> usize {
    gas_val.log2floor()
}

#[cfg(test)]
mod tests {
    use super::*;
    use evmjit::compiler::evmtypes::EvmTypes;
    use evmjit::compiler::intrinsics::LLVMIntrinsic;
    use evmjit::compiler::intrinsics::LLVMIntrinsicManager;
    use singletonum::Singleton;
    use inkwell::context::Context;
    //use inkwell::module::Module;
    //use inkwell::builder::Builder;
    use inkwell::types::BasicTypeEnum;
    //use inkwell::types::IntType;

    #[test]
    fn test_log2() {
        let temp1 = simulate_log_base2 (Gas::from (55u64));
        let temp2 = native_log_base2(Gas::from (55u64));
        assert_eq!(temp1, temp2);

    }

    #[test]
    fn test_log2_using_jit() {
        let context = Context::create();
        let module = context.create_module("evm_module");
        let types_instance = EvmTypes::get_instance(&context);
        let word_type = types_instance.get_word_type();
        let enum_word_type = BasicTypeEnum::IntType(word_type);
        let builder = context.create_builder();

        // Get declaration of ctlz
        let ctlz_decl = LLVMIntrinsic::Ctlz.get_intrinsic_declaration(&context,
                                                                      &module,
                                                                      Some(enum_word_type));

        let const_256 = context.custom_width_int_type(256).const_int_from_string("55", 10);

        //auto lz256 = m_builder.CreateCall(ctlz, {_exponent, m_builder.getInt1(false)});
        //auto lz = m_builder.CreateTrunc(lz256, Type::Gas, "lz");
        //auto sigBits = m_builder.CreateSub(m_builder.getInt64(256), lz, "sigBits");

        let lz256 = builder.build_call (ctlz_decl, &[const_256.into(), context.bool_type().const_zero().into()], "lz256");
        let val = lz256.try_as_basic_value().left().unwrap().into_int_value();
        let lz = builder.build_int_truncate(val, word_type, "lz");

        let temp1 = context.i64_type().const_int(256, false);
        let one = context.i64_type().const_int(1, false);
        let sigBits = builder.build_int_sub(temp1, lz, "sigBits");
        let log2Val = builder.build_int_sub (sigBits, one, "log2");

    }
}

