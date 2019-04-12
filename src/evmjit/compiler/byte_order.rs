#![allow(dead_code)]

use inkwell::module::Module;
use inkwell::values::IntValue;
use inkwell::context::Context;
use inkwell::builder::Builder;
use inkwell::types::BasicTypeEnum;
use super::intrinsics::{LLVMIntrinsic, LLVMIntrinsicManager};
use super::JITContext;

pub fn byte_order_swap(context: &JITContext, builder: &Builder, value: IntValue) -> IntValue {
    // Swap byte order if the host system is little endian

    if cfg!(target_endian = "little") {
   // if byteorder::NativeEndian == byteorder::LE {
        // TODO add support for byte swapping constants at compile time
        // Current problem is that the LLVM API does not expose the APInt class
        // which allows access to the underlying integer value type

        let enum_addr_type = BasicTypeEnum::IntType(value.get_type());
        let bswap_arg = Some(enum_addr_type);
        let bswap_func = LLVMIntrinsic::Bswap.get_intrinsic_declaration(context, bswap_arg);
        let callsite_val = builder.build_call (bswap_func, &[value.into()], "");

        // build_call returns a Either<BasicValueEnum, InstructionValue>
        let ret = callsite_val.try_as_basic_value().left().unwrap();
        ret.into_int_value()
    }
    else {
        value
    }
}
