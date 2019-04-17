use inkwell::AddressSpace;
use inkwell::types::BasicTypeEnum;
use inkwell::builder::Builder;
use inkwell::module::Module;
use inkwell::values::FunctionValue;
use inkwell::values::InstructionValue;
use inkwell::values::BasicValueEnum;
use inkwell::basic_block::BasicBlock;

pub mod compiler;

pub trait ModuleLookup {
    fn in_main_function(&self, builder: & Builder) -> bool;
    fn get_main_function(&self, builder: & Builder) -> Option<FunctionValue>;
}

impl ModuleLookup for Module {
    fn get_main_function(&self, builder: & Builder) -> Option<FunctionValue> {
        // The parent of the first basic block is a function

        let bb = builder.get_insert_block();
        assert!(bb != None);

        let found_func = bb.unwrap().get_parent();
        assert!(found_func != None);
        let found_func_val = found_func.unwrap();

        // The main function (by convention) is the first one in the module
        let main_func = self.get_first_function();
        assert!(main_func != None);

        if found_func_val == main_func.unwrap() {
            found_func
        }
        else {
            None
        }
    }

    fn in_main_function(&self, builder: & Builder) -> bool {
        if self.get_main_function(builder) != None {
            true
        } else {
            false
        }
    }
}

pub trait GetOperandValue {
    fn get_operand_value(&self, index: u32) -> Option<BasicValueEnum>;
}

impl GetOperandValue for InstructionValue {
    fn get_operand_value(&self, index: u32) -> Option<BasicValueEnum> {
        let operand = self.get_operand(index);
        if operand == None {
            None
        } else {
            Some(operand.unwrap().left().unwrap())
        }
    }
}

pub trait GetOperandBasicBlock {
    fn get_operand_as_bb(&self, index: u32) -> Option<BasicBlock>;
}

impl GetOperandBasicBlock for InstructionValue {
    fn get_operand_as_bb(&self, index: u32) -> Option<BasicBlock> {
        let operand = self.get_operand(index);
        if operand == None {
            None
        } else {
            Some(operand.unwrap().right().unwrap())
        }
    }
}

pub trait FindBasicBlock {
    fn find_bb(&self, name : &str) -> Option<BasicBlock>;
}

pub trait BasicTypeEnumCompare {
    fn is_int_t(self) -> bool;
    fn is_int1(self) -> bool;
    fn is_int8(self) -> bool;
    fn is_int32(self) -> bool;
    fn is_int64(self) -> bool;
    fn is_int128(self) -> bool;
    fn is_int256(self) -> bool;
    fn is_ptr_type(self) -> bool;
    fn is_ptr_to_int8(self) -> bool;
    fn is_ptr_to_int256(self) -> bool;
    fn is_ptr_to_struct(self) -> bool;
    fn is_array_t(self) -> bool;
    fn is_array_of_len_n(self, len:u32) -> bool;
    fn is_int8_array(self, len:u32) -> bool;
}

impl BasicTypeEnumCompare for BasicTypeEnum {
    fn is_int_t(self) -> bool {
        self.is_int_type()
    }

    fn is_int1(self) -> bool {
        self.is_int_type() && (self.into_int_type().get_bit_width() == 1)
    }

    fn is_int8(self) -> bool {
        self.is_int_type() && (self.into_int_type().get_bit_width() == 8)
    }

    fn is_int32(self) -> bool {
        self.is_int_type() && (self.into_int_type().get_bit_width() == 32)
    }

    fn is_int64(self) -> bool {
        self.is_int_type() && (self.into_int_type().get_bit_width() == 64)
    }

    fn is_int128(self) -> bool {
        self.is_int_type() && (self.into_int_type().get_bit_width() == 128)
    }

    fn is_int256(self) -> bool {
        self.is_int_type() && (self.into_int_type().get_bit_width() == 256)
    }

    fn is_ptr_type(self) -> bool {
        self.is_pointer_type() &&
        (self.as_pointer_type().get_address_space() == AddressSpace::Generic)
    }

    fn is_ptr_to_int8(self) -> bool {
        if !self.is_ptr_type() {
            false;
        }

        let elem_t = self.as_pointer_type().get_element_type();
        elem_t.is_int_type() && (elem_t.as_int_type().get_bit_width() == 8)
    }

    fn is_ptr_to_int256(self) -> bool {
        if !self.is_ptr_type() {
            false;
        }

        let elem_t = self.as_pointer_type().get_element_type();
        elem_t.is_int_type() && (elem_t.as_int_type().get_bit_width() == 256)
    }

    fn is_ptr_to_struct(self) -> bool {
        if !self.is_ptr_type() {
            false;
        }

        let elem_t = self.as_pointer_type().get_element_type();
        elem_t.is_struct_type()
    }

    fn is_array_t(self) -> bool {
        self.is_array_type()
    }

    fn is_array_of_len_n(self, len : u32) -> bool {
        self.is_array_type() && (self.into_array_type().len() == len)
    }

    fn is_int8_array(self, len : u32) -> bool {
        self.is_array_of_len_n (len) &&
        self.into_array_type().get_element_type().is_int_type() &&
        (self.into_int_type().get_bit_width() == len)
    }
}
