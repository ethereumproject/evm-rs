#![allow(dead_code)]

extern crate num_bigint;
use util::opcode::Opcode;
use inkwell::values::IntValue;
use self::num_bigint::BigUint;
use super::super::JITContext;


pub fn read_push_data(context: & JITContext, push_opcode: Opcode, slice: &[u8]) -> IntValue {
    let push_instruction : u8 = push_opcode.into();

    let push1 : u8 = Opcode::PUSH(1).into();
    let push32 : u8 = Opcode::PUSH(32).into();

    assert!(push_instruction >= push1 && push_instruction <= push32);
    let value = BigUint::from_bytes_be(slice);


    let bigint_str = value.to_str_radix(10);
    let api_str = &*bigint_str;
    context.llvm_context().custom_width_int_type(256).const_int_from_string(api_str, 10)
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn test_push1() {
        let jitctx = JITContext::new();
        let mut byte_code : Vec<u8> = Vec::new();

        byte_code.push(55);

        let push1_llvm_value = read_push_data(&jitctx, Opcode::PUSH(1), &byte_code);
        let push1_value = push1_llvm_value.get_zero_extended_constant().unwrap();
        assert_eq!(push1_value, 55);
    }

    #[test]
    fn test_push2() {
        let jitctx = JITContext::new();
        let mut byte_code : Vec<u8> = Vec::new();

        byte_code.push(0xff);
        byte_code.push(0x37);

        let push2_llvm_value = read_push_data(&jitctx, Opcode::PUSH(2), &byte_code);
        let push2_value = push2_llvm_value.get_zero_extended_constant().unwrap();
        assert_eq!(push2_value, 0xff37);
    }

    #[test]
    fn test_push4() {
        let jitctx = JITContext::new();
        let mut byte_code : Vec<u8> = Vec::new();

        byte_code.push(0xaa);
        byte_code.push(0xbb);
        byte_code.push(0xcc);
        byte_code.push(0xdd);

        let push4_llvm_value = read_push_data(&jitctx, Opcode::PUSH(4), &byte_code);
        let push4_value = push4_llvm_value.get_zero_extended_constant().unwrap();
        assert_eq!(push4_value, 0xaabbccdd);
    }

    #[test]
    fn test_push8() {
        let jitctx = JITContext::new();
        let mut byte_code : Vec<u8> = Vec::new();

        byte_code.push(0x59);
        byte_code.push(0x41);
        byte_code.push(0x39);
        byte_code.push(0x37);

        byte_code.push(0x2f);
        byte_code.push(0x1f);
        byte_code.push(0x13);
        byte_code.push(0x11);

        let push8_llvm_value = read_push_data(&jitctx, Opcode::PUSH(8), &byte_code);
        let push8_value = push8_llvm_value.get_zero_extended_constant().unwrap();
        assert_eq!(push8_value, 0x594139372f1f1311u64);
    }

}

