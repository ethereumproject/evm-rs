#![allow(dead_code)]

use std::ffi::CString;
use singletonum::{Singleton, SingletonInit};
use inkwell::context::Context;
use inkwell::builder::Builder;
use inkwell::module::Module;
use inkwell::types::StructType;
use inkwell::types::PointerType;
use inkwell::values::PointerValue;
use inkwell::AddressSpace;
use evmjit::BasicTypeEnumCompare;
use evmjit::LLVMAttributeFactory;
use evmjit::compiler::evmtypes::EvmTypes;
use std::cell::RefCell;
use inkwell::module::Linkage::*;
use inkwell::values::FunctionValue;

#[derive(Debug, Singleton)]

// Internal representation of EVM linear memory

pub struct MemoryRepresentationType {
    memory_type: StructType,
    memory_ptr_type: PointerType,
    memory_array_type: PointerType
}

unsafe impl Sync for MemoryRepresentationType {}
unsafe impl Send for MemoryRepresentationType {}

impl SingletonInit for MemoryRepresentationType {
    type Init = Context;
    fn init(context: &Context) -> Self {
        let evm_word_t = context.custom_width_int_type(256);
        let evm_word_ptr_t = evm_word_t.ptr_type(AddressSpace::Generic);

        let size_t = context.i64_type();

        let fields = [evm_word_ptr_t.into(),
                      size_t.into(),
                      size_t.into()];

        let mem_struct = context.opaque_struct_type("LinearMemory");
        mem_struct.set_body(&fields, false);
        
        MemoryRepresentationType {
            memory_type: mem_struct,
            memory_ptr_type: mem_struct.ptr_type(AddressSpace::Generic),
            memory_array_type: evm_word_ptr_t
        }
    }
}

impl MemoryRepresentationType {    
    pub fn get_type(&self) -> StructType {
        self.memory_type
    }

    pub fn get_ptr_type(&self) -> PointerType {
        self.memory_ptr_type
    }

    pub fn get_mem_array_type(&self) -> PointerType {
        self.memory_array_type
    }

    pub fn is_mem_representation_type(a_struct: &StructType) -> bool {
        if !a_struct.is_sized() {
            return false;
        }

        if a_struct.count_fields() != 3 {
            return false;
        }

        if a_struct.is_packed() {
            return false;
        }

        if a_struct.is_opaque() {
            return false;
        }

        if a_struct.get_name() != Some(&*CString::new("LinearMemory").unwrap()) {
            return false;
        }

        let field1 = a_struct.get_field_type_at_index(0).unwrap();
        if !field1.is_ptr_to_int256() {
            return false;
        }

        let field2 = a_struct.get_field_type_at_index(1).unwrap();
        if !field2.is_int64() {
            return false;
        }

        let field3 = a_struct.get_field_type_at_index(2).unwrap();
        if !field3.is_int64() {
            return false;
        }

        true

    }
}

// This struct create the local functions needed by MemoryRepresentation on demand

pub struct MemoryRepresentationFunctionManager<'a> {
    m_context: &'a Context,
    m_module: &'a Module,
    m_evm_mem_ptr_func: RefCell<Option<FunctionValue>>,
    m_evm_mem_extend_func: RefCell<Option<FunctionValue>>,
}

impl<'a> MemoryRepresentationFunctionManager<'a> {
    pub fn new(context: &'a Context, module: &'a Module) -> MemoryRepresentationFunctionManager<'a> {
        MemoryRepresentationFunctionManager {
            m_context: context,
            m_module: module,
            m_evm_mem_ptr_func: RefCell::new(None),
            m_evm_mem_extend_func: RefCell::new(None),
        }
    }

    pub fn get_evm_mem_ptr_func(&self) -> FunctionValue {
        if self.m_evm_mem_ptr_func.borrow().is_none() {

            // First create function declaration

            let types_instance = EvmTypes::get_instance(self.m_context);
            let arg1 = MemoryRepresentationType::get_instance(self.m_context).get_ptr_type();
            let arg2 = types_instance.get_size_type();
            let evm_mem_ptr_fn_type = types_instance.get_word_ptr_type().fn_type(&[arg1.into(), arg2.into()], false);

            let evm_mem_func = self.m_module.add_function ("mem.getPtr", evm_mem_ptr_fn_type, Some(Private));
            let attr_factory = LLVMAttributeFactory::get_instance(&self.m_context);

            evm_mem_func.add_attribute(0, *attr_factory.attr_nounwind());
            evm_mem_func.add_attribute(1, *attr_factory.attr_nocapture());

            assert!(evm_mem_func.get_nth_param(0).is_some());
            let evm_mem_ptr = evm_mem_func.get_nth_param(0).unwrap();
            evm_mem_ptr.into_pointer_value().set_name("evmMemPtr");

            assert!(evm_mem_func.get_nth_param(1).is_some());
            let index_in_mem = evm_mem_func.get_nth_param(1).unwrap();
            index_in_mem.into_int_value().set_name("index");

            // Now build function body IR

            let temp_builder = self.m_context.create_builder();
            let entry_bb = self.m_context.append_basic_block(&evm_mem_func, "entry");

            temp_builder.position_at_end(&entry_bb);

            let data_ptr = temp_builder.build_bitcast(evm_mem_ptr.into_pointer_value(),
                                                                types_instance.get_byte_ptr_type().ptr_type(AddressSpace::Generic), "");
            let data = temp_builder.build_load(data_ptr.into_pointer_value(), "data");
            let index_value = index_in_mem.into_int_value();

            unsafe {
                let byte_ptr = temp_builder.build_gep(data.into_pointer_value(), &[index_value], "bytePtr");
                let word_ptr = temp_builder.build_bitcast(byte_ptr, types_instance.get_word_ptr_type(), "wordPtr");
                temp_builder.build_return(Some(&word_ptr));
            }

            // Return instance of function with IR built out

            *self.m_evm_mem_ptr_func.borrow_mut() = Some(evm_mem_func);
            evm_mem_func
        }
        else {
            let func = self.m_evm_mem_ptr_func.borrow().unwrap();
            func
        }

    }
}
pub struct MemoryRepresentation<'a> {
    m_context: &'a Context,
    m_builder: &'a Builder,
    m_module: &'a Module,
    m_memory: PointerValue,
    m_func_mgr: MemoryRepresentationFunctionManager<'a>
}

impl<'a> MemoryRepresentation<'a> {

    pub fn new(allocated_memory: PointerValue, context: &'a Context,
               builder: &'a Builder, module: &'a Module) -> MemoryRepresentation<'a> {
        let mem_type = MemoryRepresentationType::get_instance(context).get_type();
        builder.build_store(allocated_memory, mem_type.const_zero());

        MemoryRepresentation {
            m_context: context,
            m_builder: builder,
            m_module: module,
            m_memory: allocated_memory,
            m_func_mgr: MemoryRepresentationFunctionManager::new(context, module)
        }

    }

    pub fn new_with_name(name: &str, context: &'a Context,
                         builder: &'a Builder, module: &'a Module) -> MemoryRepresentation<'a> {
        let mem_type = MemoryRepresentationType::get_instance(context).get_type();
        let alloca_result = builder.build_alloca(mem_type, name);
        builder.build_store(alloca_result, mem_type.const_zero());

        MemoryRepresentation {
            m_context: context,
            m_builder: builder,
            m_module: module,
            m_memory: alloca_result,
            m_func_mgr: MemoryRepresentationFunctionManager::new(context, module)
        }
    }

    pub fn get_memory_representation_type(&self) -> StructType {
        MemoryRepresentationType::get_instance(self.m_context).get_type()
    }
}

#[cfg(test)]
mod mem_rep_tests {
    use super::*;
    use inkwell::attributes::Attribute;
    use inkwell::values::InstructionOpcode;
    use evmjit::GetOperandValue;

    #[test]
    fn test_memory_representation_type() {
        let context = Context::create();
        let mem_type_singleton = MemoryRepresentationType::get_instance(&context);
        let mem_struct = mem_type_singleton.get_type();

        assert!(MemoryRepresentationType::is_mem_representation_type(&mem_struct));

        let mem_struct_ptr = mem_type_singleton.get_ptr_type();
        assert!(mem_struct_ptr.get_element_type().is_struct_type());
        assert!(MemoryRepresentationType::is_mem_representation_type(mem_struct_ptr.get_element_type().as_struct_type()));

        let evm_word_t = context.custom_width_int_type(256);
        let evm_word_ptr_t = evm_word_t.ptr_type(AddressSpace::Generic);
        let size_t = context.i64_type();

        let fields = [evm_word_ptr_t.into(),
            size_t.into(),
            context.i32_type().into()];

        let mem_struct2 = context.opaque_struct_type("LinearMemory");
        mem_struct2.set_body(&fields, false);

        assert!(!MemoryRepresentationType::is_mem_representation_type(&mem_struct2));

        let ptr_elem_t = mem_type_singleton.get_mem_array_type().get_element_type();
        assert!(ptr_elem_t.is_int_type());
        assert_eq!(ptr_elem_t.into_int_type().get_bit_width(), 256);
    }

    #[test]
    fn test_memory_representation_function_manager() {
        let context = Context::create();
        let module = context.create_module("my_module");

        let mem_rep = MemoryRepresentationFunctionManager::new(&context, &module);
        assert!(module.get_function("mem.getPtr").is_none());
        let _mem_get_ptr_func = mem_rep.get_evm_mem_ptr_func();
        assert!(module.get_function("mem.getPtr").is_some());
    }

    #[test]
    fn test_memory_representation_get_ptr() {
        let context = Context::create();
        let module = context.create_module("my_module");

        let mem_rep = MemoryRepresentationFunctionManager::new(&context, &module);
        let mem_get_ptr_func = mem_rep.get_evm_mem_ptr_func();

        module.print_to_stderr();

        let attr_factory = LLVMAttributeFactory::get_instance(&context);

        assert_eq!(mem_get_ptr_func.count_params(), 2);
        assert_eq!(mem_get_ptr_func.count_basic_blocks(), 1);
        assert_eq!(mem_get_ptr_func.get_linkage(), Private);

        // Verify gas function has nounwind attribute
        assert_eq!(mem_get_ptr_func.count_attributes(0), 1);
        let nounwind_attr = mem_get_ptr_func.get_enum_attribute(0, Attribute::get_named_enum_kind_id("nounwind"));
        assert!(nounwind_attr != None);

        // Verify operand 1 has nocapture attribute
        assert_eq!(mem_get_ptr_func.count_attributes(1), 1);
        let nocapture_attr = mem_get_ptr_func.get_enum_attribute(1, Attribute::get_named_enum_kind_id("nocapture"));
        assert!(nocapture_attr != None);

        assert_eq!(nounwind_attr.unwrap(), *attr_factory.attr_nounwind());
        assert_eq!(nocapture_attr.unwrap(), *attr_factory.attr_nocapture());

        // Validate return type

        let ret_type = mem_get_ptr_func.get_return_type();
        assert!(ret_type.is_pointer_type());

        let ret_elem_t = ret_type.into_pointer_type().get_element_type();
        assert!(ret_elem_t.is_int_type());

        assert_eq!(ret_elem_t.into_int_type().get_bit_width(), 256);

        // Validate arguments

        assert!(mem_get_ptr_func.get_nth_param(0) != None);
        let mem_ptr_arg = mem_get_ptr_func.get_nth_param(0).unwrap();
        assert!(mem_ptr_arg.is_pointer_value());
        let arg1_elem_t = mem_ptr_arg.into_pointer_value().get_type().get_element_type();
        assert!(arg1_elem_t.is_struct_type());
        assert!(MemoryRepresentationType::is_mem_representation_type(&arg1_elem_t.into_struct_type()));

        assert!(mem_get_ptr_func.get_nth_param(1) != None);
        let index_arg = mem_get_ptr_func.get_nth_param(1).unwrap();
        assert!(index_arg.is_int_value());
        assert_eq!(index_arg.into_int_value().get_type().get_bit_width(), 64);

        let entry_block_optional = mem_get_ptr_func.get_first_basic_block();
        assert!(entry_block_optional != None);
        let entry_block = entry_block_optional.unwrap();

        // Validate instructions

        // %0 = bitcast %LinearMemory* %evmMemPtr to i8**

        assert!(entry_block.get_first_instruction() != None);
        let first_insn = entry_block.get_first_instruction().unwrap();
        assert_eq!(first_insn.get_opcode(), InstructionOpcode::BitCast);
        assert_eq!(first_insn.get_num_operands(), 1);

        let bitcast_operand0 = first_insn.get_operand_value(0).unwrap();
        assert_eq!(bitcast_operand0, mem_ptr_arg);
        assert!(first_insn.get_next_instruction() != None);

        //  %data = load i8*, i8** %0

        let second_insn = first_insn.get_next_instruction().unwrap();
        assert_eq!(second_insn.get_opcode(), InstructionOpcode::Load);
        assert_eq!(second_insn.get_num_operands(), 1);

        let load_operand0 = second_insn.get_operand_value(0).unwrap();
        assert!(load_operand0.is_pointer_value());
        let load_operand0_ptr_elt_t = load_operand0.into_pointer_value().get_type().get_element_type();
        assert!(load_operand0_ptr_elt_t.is_pointer_type());
        let ptr_to_ptr_type = load_operand0_ptr_elt_t.into_pointer_type().get_element_type();
        assert!(ptr_to_ptr_type.is_int_type());
        assert!(ptr_to_ptr_type.into_int_type().get_bit_width() == 8);

        //  %bytePtr = getelementptr i8, i8* %data, i64 %index

        assert!(second_insn.get_next_instruction() != None);

        let third_insn = second_insn.get_next_instruction().unwrap();
        assert_eq!(third_insn.get_opcode(), InstructionOpcode::GetElementPtr);
        assert_eq!(third_insn.get_num_operands(), 2);

        let third_insn_operand0 = third_insn.get_operand_value(0).unwrap();
        assert!(third_insn_operand0.is_pointer_value());

        // Verify that data ptr is first operand of gep
        let data_val_use = second_insn.get_first_use().unwrap().get_used_value().left().unwrap();
        assert_eq!(third_insn_operand0, data_val_use);

        // Verify that second argument to gep is index
        let third_insn_operand1 = third_insn.get_operand_value(1).unwrap();
        assert_eq!(third_insn_operand1, index_arg);

        //  %wordPtr = bitcast i8* %bytePtr to i256*

        assert!(third_insn.get_next_instruction() != None);

        let fourth_insn = third_insn.get_next_instruction().unwrap();
        assert_eq!(fourth_insn.get_opcode(), InstructionOpcode::BitCast);
        assert_eq!(fourth_insn.get_num_operands(), 1);

        let fourth_insn_operand0 = fourth_insn.get_operand_value(0).unwrap();

        // Verify that data ptr is first operand of gep
        let byte_ptr_use = third_insn.get_first_use().unwrap().get_used_value().left().unwrap();
        assert_eq!(fourth_insn_operand0, byte_ptr_use);

        //  ret i256* %wordPtr

        assert!(fourth_insn.get_next_instruction() != None);

        let fifth_insn = fourth_insn.get_next_instruction().unwrap();
        assert_eq!(fifth_insn.get_opcode(), InstructionOpcode::Return);
        assert_eq!(fifth_insn.get_num_operands(), 1);

        let fifth_insn_operand0 = fifth_insn.get_operand_value(0).unwrap();
        assert_eq!(fifth_insn_operand0.get_type(), ret_type);
        assert!(fifth_insn.get_next_instruction().is_none());
    }
}

