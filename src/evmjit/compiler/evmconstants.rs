#![allow(dead_code)]

use inkwell::context::Context;
use inkwell::values::IntValue;

#[derive(Debug)]
pub struct EvmConstants {
    gas_max: IntValue,
    i64_zero: IntValue,
}

impl EvmConstants {
    pub fn new(context: &Context) -> Self {
        EvmConstants {
            gas_max: context.i64_type().const_int(std::i64::MAX as u64, false),
            i64_zero: context.i64_type().const_int(0u64, false),
        }
    }

    pub fn get_gas_max(&self) -> IntValue {
        self.gas_max
    }

    pub fn get_i64_zero(&self) -> IntValue {
        self.i64_zero
    }
}

#[test]
fn test_evmconstants() {
    let context = Context::create();
    let evm_constants_singleton = EvmConstants::new(&context);

    let max_g = evm_constants_singleton.get_gas_max();
    assert!(max_g.is_const());
    assert_eq!(max_g.get_zero_extended_constant(), Some(std::i64::MAX as u64));

    let i64_zero = evm_constants_singleton.get_i64_zero();
    assert!(i64_zero.is_const());
    assert_eq!(i64_zero.get_zero_extended_constant(), Some(0));
}