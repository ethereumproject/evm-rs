#![allow(dead_code)]
use std::ffi::CString;
use inkwell::context::Context;
use inkwell::types::StructType;
use inkwell::types::PointerType;
use inkwell::values::PointerValue;
use inkwell::AddressSpace;
use evmjit::BasicTypeEnumCompare;
use std::cell::RefCell;
use inkwell::module::Linkage::*;
use inkwell::values::FunctionValue;
use evmjit::compiler::external_declarations::ExternalFunctionManager;
use inkwell::types::IntType;
use inkwell::types::BasicTypeEnum;
use evmjit::compiler::intrinsics::LLVMIntrinsic;
use evmjit::compiler::intrinsics::LLVMIntrinsicManager;
use inkwell::values::BasicValueEnum;
use inkwell::values::IntValue;

use super::super::JITContext;

#[derive(Debug)]

// Internal representation of EVM linear memory

pub struct MemoryRepresentationType {
    memory_type: StructType,
    memory_ptr_type: PointerType,
    memory_array_type: PointerType
}

impl MemoryRepresentationType {
    pub fn new(context: &Context) -> Self {
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
    m_context: &'a JITContext,
    m_evm_mem_ptr_func: RefCell<Option<FunctionValue>>,
    m_evm_mem_extend_func: RefCell<Option<FunctionValue>>,
    m_external_func_mgr: &'a ExternalFunctionManager<'a>
}

impl<'a> MemoryRepresentationFunctionManager<'a> {
    pub fn new(context: &'a JITContext, external_func_mgr: &'a ExternalFunctionManager) -> MemoryRepresentationFunctionManager<'a> {
        MemoryRepresentationFunctionManager {
            m_context: context,
            m_evm_mem_ptr_func: RefCell::new(None),
            m_evm_mem_extend_func: RefCell::new(None),
            m_external_func_mgr: external_func_mgr
        }
    }

    pub fn get_extend_func(&self) -> FunctionValue {
        if self.m_evm_mem_extend_func.borrow().is_none() {
            let types_instance = self.m_context.evm_types();
            let context = self.m_context.llvm_context();

            let ret_type = context.void_type();
            let arg1 = self.m_context.memrep().get_ptr_type();
            let arg2 = types_instance.get_size_type();

            let extend_evm_mem_fn_type = ret_type.fn_type(&[arg1.into(), arg2.into()], false);
            let extend_evm_mem_func = self.m_context.module().add_function ("mem.extend", extend_evm_mem_fn_type, Some(Private));
            let attr_factory = self.m_context.attributes();

            extend_evm_mem_func.add_attribute(0, *attr_factory.attr_nounwind());
            extend_evm_mem_func.add_attribute(1, *attr_factory.attr_nocapture());

            assert!(extend_evm_mem_func.get_nth_param(0).is_some());
            let evm_mem_ptr = extend_evm_mem_func.get_nth_param(0).unwrap();
            evm_mem_ptr.into_pointer_value().set_name("evmMemPtr");

            assert!(extend_evm_mem_func.get_nth_param(1).is_some());
            let requested_size = extend_evm_mem_func.get_nth_param(1).unwrap();
            requested_size.into_int_value().set_name("newSize");

            // Now build function body IR

            let temp_builder = context.create_builder();
            let entry_bb = context.append_basic_block(&extend_evm_mem_func, "extend_entry");

            temp_builder.position_at_end(&entry_bb);

            let data_ptr = temp_builder.build_bitcast(evm_mem_ptr.into_pointer_value(),
                                                      types_instance.get_byte_ptr_type().ptr_type(AddressSpace::Generic), "dataPtr");

            unsafe {
                let gep_arg1 = evm_mem_ptr.into_pointer_value();
                let mem_size_ptr = temp_builder.build_struct_gep(gep_arg1, 1 as u32, "sizePtr");

                // auto capPtr = m_builder.CreateStructGEP(getType(), arrayPtr, 2, "capPtr");

                let mem_cap_ptr = temp_builder.build_struct_gep(gep_arg1, 2 as u32, "capPtr");

                //    //	auto data = m_builder.CreateLoad(dataPtr, "data");

                let data = temp_builder.build_load(data_ptr.into_pointer_value(), "data");

                //    //	auto size = m_builder.CreateLoad(sizePtr, "size");

                let current_size = temp_builder.build_load(mem_size_ptr, "size");

                //    //	auto extSize = m_builder.CreateNUWSub(newSize, size, "extSize");

                let extended_size = temp_builder.build_int_nuw_sub(requested_size.into_int_value(), current_size.into_int_value(), "extendedSize");

                let realloc_func = self.m_external_func_mgr.get_realloc_decl();
                let alloc_mem = temp_builder.build_call(realloc_func, &[data.into(), extended_size.into()], "newMem");
                let alloc_mem_as_ptr = alloc_mem.try_as_basic_value().left().unwrap().into_pointer_value();
                let alloc_mem_as_value = alloc_mem.try_as_basic_value().left().unwrap();

                let mem_ptr = temp_builder.build_gep(alloc_mem_as_ptr, &[extended_size], "extPtr");


                let memset_arg = BasicTypeEnum::IntType (IntType::i64_type());
                let memset_func = LLVMIntrinsic::MemSet.get_intrinsic_declaration(self.m_context,
                                                                               Some(memset_arg));
                let zero_int_8 = context.i8_type().const_zero();
                let bool_false = context.bool_type().const_zero();


                temp_builder.build_call(memset_func, &[mem_ptr.into(), zero_int_8.into(), extended_size.into(), bool_false.into()], "");

                //  auto newData = m_reallocFunc.call(m_builder, {data, newSize}, "newData"); // TODO: Check realloc result for null
                //    //	auto extPtr = m_builder.CreateGEP(newData, size, "extPtr");
                //    //	m_builder.CreateMemSet(extPtr, m_builder.getInt8(0), extSize, 16);
                //    //	m_builder.CreateStore(newData, dataPtr);
                temp_builder.build_store(data_ptr.into_pointer_value(), alloc_mem_as_value);
                //    //	m_builder.CreateStore(newSize, sizePtr);

                temp_builder.build_store(mem_size_ptr, requested_size);

                //    //	m_builder.CreateStore(newSize, capPtr);

                temp_builder.build_store(mem_cap_ptr, requested_size);
                //    //	m_builder.CreateRetVoid();

                temp_builder.build_return(None);
            }

            // Return created function body

            *self.m_evm_mem_extend_func.borrow_mut() = Some(extend_evm_mem_func);
            extend_evm_mem_func
        }
        else {
            let func = self.m_evm_mem_extend_func.borrow().unwrap();
            func
        }
    }

    pub fn get_evm_mem_ptr_func(&self) -> FunctionValue {
        if self.m_evm_mem_ptr_func.borrow().is_none() {

            // First create function declaration

            let types_instance = self.m_context.evm_types();
            let arg1 = self.m_context.memrep().get_ptr_type();
            let arg2 = types_instance.get_size_type();
            let evm_mem_ptr_fn_type = types_instance.get_word_ptr_type().fn_type(&[arg1.into(), arg2.into()], false);

            let evm_mem_func = self.m_context.module().add_function ("mem.getPtr", evm_mem_ptr_fn_type, Some(Private));
            let attr_factory = self.m_context.attributes();

            evm_mem_func.add_attribute(0, *attr_factory.attr_nounwind());
            evm_mem_func.add_attribute(1, *attr_factory.attr_nocapture());

            assert!(evm_mem_func.get_nth_param(0).is_some());
            let evm_mem_ptr = evm_mem_func.get_nth_param(0).unwrap();
            evm_mem_ptr.into_pointer_value().set_name("evmMemPtr");

            assert!(evm_mem_func.get_nth_param(1).is_some());
            let index_in_mem = evm_mem_func.get_nth_param(1).unwrap();
            index_in_mem.into_int_value().set_name("index");

            // Now build function body IR

            let temp_builder = self.m_context.llvm_context().create_builder();
            let entry_bb = self.m_context.llvm_context().append_basic_block(&evm_mem_func, "mem_ptr_entry");

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
    m_context: &'a JITContext,
    m_memory: PointerValue,
    m_func_mgr: MemoryRepresentationFunctionManager<'a>
}

impl<'a> MemoryRepresentation<'a> {

    pub fn new(allocated_memory: PointerValue, context: &'a JITContext,
                external_func_mgr: &'a ExternalFunctionManager) -> MemoryRepresentation<'a> {
        let mem_type = context.memrep().get_type();
        context.builder().build_store(allocated_memory, mem_type.const_zero());

        MemoryRepresentation {
            m_context: context,
            m_memory: allocated_memory,
            m_func_mgr: MemoryRepresentationFunctionManager::new(context, external_func_mgr)
        }

    }

    pub fn new_with_name(name: &str, context: &'a JITContext,
                         external_func_mgr: &'a ExternalFunctionManager) -> MemoryRepresentation<'a> {
        let mem_type = context.memrep().get_type();
        let alloca_result = context.builder().build_alloca(mem_type, name);
        context.builder().build_store(alloca_result, mem_type.const_zero());

        MemoryRepresentation {
            m_context: context,
            m_memory: alloca_result,
            m_func_mgr: MemoryRepresentationFunctionManager::new(context, external_func_mgr)
        }
    }

    pub fn get_memory_representation_type(&self) -> StructType {
        self.m_context.memrep().get_type()
    }

    pub fn get_internal_mem_size(&self) -> BasicValueEnum {
        self.get_mem_size(self.m_memory)
    }

    pub fn get_mem_size(&self, mem: PointerValue) -> BasicValueEnum {
        unsafe {
            let size_ptr = self.m_context.builder().build_struct_gep(mem, 1, "sizePtr");
            self.m_context.builder().build_load(size_ptr, "mem.size")
        }
    }

    // llvm::Value* getPtr(llvm::Value* _arrayPtr, llvm::Value* _index) { return m_getPtrFunc.call(m_builder, {_arrayPtr, _index}); }

    pub fn get_mem_ptr(&self, mem: PointerValue, index: IntValue) -> PointerValue {
        let call_site = self.m_context.builder().build_call(self.m_func_mgr.get_evm_mem_ptr_func(),
                                                  &[mem.into(), index.into()], "");
        assert!(call_site.try_as_basic_value().left().is_some());
        let ret = call_site.try_as_basic_value().left().unwrap();
        ret.into_pointer_value()
    }

    pub fn extend_memory_size(&self, mem: PointerValue, size: IntValue) {
        assert_eq!(mem.get_type().get_element_type(), self.m_memory.get_type().get_element_type());
        assert_eq!(size.get_type(), self.m_context.evm_types().get_size_type());
        let extend_func = self.m_func_mgr.get_extend_func();
        self.m_context.builder().build_call(extend_func, &[mem.into(), size.into()], "");
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
        let mem_type_singleton = MemoryRepresentationType::new(&context);
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
        let jitctx = JITContext::new();
        let module = jitctx.module();
        let external_func_mgr = ExternalFunctionManager::new(&jitctx);

        let mem_rep = MemoryRepresentationFunctionManager::new(&jitctx, &external_func_mgr);
        assert!(module.get_function("mem.getPtr").is_none());
        let _mem_get_ptr_func = mem_rep.get_evm_mem_ptr_func();
        assert!(module.get_function("mem.getPtr").is_some());

        assert!(module.get_function("mem.extend").is_none());
        let _mem_extend_func = mem_rep.get_extend_func();
        assert!(module.get_function("mem.extend").is_some());
    }

    #[test]
    fn test_memory_representation_get_ptr() {
        let jitctx = JITContext::new();
        let external_func_mgr = ExternalFunctionManager::new(&jitctx);

        let mem_rep = MemoryRepresentationFunctionManager::new(&jitctx, &external_func_mgr);
        let mem_get_ptr_func = mem_rep.get_evm_mem_ptr_func();

        //module.print_to_stderr();

        let attr_factory = jitctx.attributes();

        assert_eq!(mem_get_ptr_func.count_params(), 2);
        assert_eq!(mem_get_ptr_func.count_basic_blocks(), 1);
        assert_eq!(mem_get_ptr_func.get_linkage(), Private);

        // Verify function has nounwind attribute
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

    #[test]
    fn test_memory_representation_extend_memory() {
        let jitctx = JITContext::new();
        let external_func_mgr = ExternalFunctionManager::new(&jitctx);

        let mem_rep = MemoryRepresentationFunctionManager::new(&jitctx, &external_func_mgr);
        let mem_get_extend_func = mem_rep.get_extend_func();

        //module.print_to_stderr();

        let attr_factory = jitctx.attributes();

        assert_eq!(mem_get_extend_func.count_params(), 2);
        assert_eq!(mem_get_extend_func.count_basic_blocks(), 1);
        assert_eq!(mem_get_extend_func.get_linkage(), Private);

        // Verify function has nounwind attribute
        assert_eq!(mem_get_extend_func.count_attributes(0), 1);
        let nounwind_attr = mem_get_extend_func.get_enum_attribute(0, Attribute::get_named_enum_kind_id("nounwind"));
        assert!(nounwind_attr != None);

        // Verify operand 1 has nocapture attribute
        assert_eq!(mem_get_extend_func.count_attributes(1), 1);
        let nocapture_attr = mem_get_extend_func.get_enum_attribute(1, Attribute::get_named_enum_kind_id("nocapture"));
        assert!(nocapture_attr != None);

        assert_eq!(nounwind_attr.unwrap(), *attr_factory.attr_nounwind());
        assert_eq!(nocapture_attr.unwrap(), *attr_factory.attr_nocapture());

        assert!(mem_get_extend_func.get_nth_param(0) != None);
        let mem_ptr_arg = mem_get_extend_func.get_nth_param(0).unwrap();
        assert!(mem_ptr_arg.is_pointer_value());
        let arg1_elem_t = mem_ptr_arg.into_pointer_value().get_type().get_element_type();
        assert!(arg1_elem_t.is_struct_type());
        assert!(MemoryRepresentationType::is_mem_representation_type(&arg1_elem_t.into_struct_type()));

        assert!(mem_get_extend_func.get_nth_param(1) != None);
        let size_arg = mem_get_extend_func.get_nth_param(1).unwrap();
        assert!(size_arg.is_int_value());
        assert_eq!(size_arg.into_int_value().get_type().get_bit_width(), 64);

        let entry_block_optional = mem_get_extend_func.get_first_basic_block();
        assert!(entry_block_optional != None);
        let entry_block = entry_block_optional.unwrap();


        // Validate instructions

        // %dataPtr = bitcast %LinearMemory* %evmMemPtr to i8**

        assert!(entry_block.get_first_instruction() != None);
        let first_insn = entry_block.get_first_instruction().unwrap();
        assert_eq!(first_insn.get_opcode(), InstructionOpcode::BitCast);
        assert_eq!(first_insn.get_num_operands(), 1);

        let bitcast_operand0 = first_insn.get_operand_value(0).unwrap();
        assert_eq!(bitcast_operand0, mem_ptr_arg);
        assert!(first_insn.get_next_instruction() != None);

        //   %sizePtr = getelementptr inbounds %LinearMemory, %LinearMemory* %evmMemPtr, i32 0, i32 1

        let second_insn = first_insn.get_next_instruction().unwrap();
        assert_eq!(second_insn.get_opcode(), InstructionOpcode::GetElementPtr);
        assert_eq!(second_insn.get_num_operands(), 3);

        let second_insn_operand0 = second_insn.get_operand_value(0).unwrap();
        assert!(second_insn_operand0.is_pointer_value());

        assert_eq!(second_insn_operand0, mem_ptr_arg);

        let second_insn_operand1 = second_insn.get_operand_value(1).unwrap();
        assert!(second_insn_operand1.is_int_value());
        let val = second_insn_operand1.into_int_value();
        assert!(val.is_const());
        assert_eq!(val.get_zero_extended_constant().unwrap(), 0u64);

        let second_insn_operand2 = second_insn.get_operand_value(2).unwrap();
        assert!(second_insn_operand2.is_int_value());
        let val2 = second_insn_operand2.into_int_value();
        assert!(val2.is_const());
        assert_eq!(val2.get_zero_extended_constant().unwrap(), 1u64);

        assert!(second_insn.get_next_instruction() != None);

        //%capPtr = getelementptr inbounds %LinearMemory, %LinearMemory* %evmMemPtr, i32 0, i32 2

        let third_insn = second_insn.get_next_instruction().unwrap();
        assert_eq!(third_insn.get_opcode(), InstructionOpcode::GetElementPtr);
        assert_eq!(third_insn.get_num_operands(), 3);

        let third_insn_operand1 = third_insn.get_operand_value(1).unwrap();
        assert!(third_insn_operand1.is_int_value());
        let third_insn_val = third_insn_operand1.into_int_value();
        assert!(third_insn_val.is_const());
        assert_eq!(third_insn_val.get_zero_extended_constant().unwrap(), 0u64);

        let third_insn_operand2 = third_insn.get_operand_value(2).unwrap();
        assert!(third_insn_operand2.is_int_value());
        let third_insn_val2 = third_insn_operand2.into_int_value();
        assert!(third_insn_val2.is_const());
        assert_eq!(third_insn_val2.get_zero_extended_constant().unwrap(), 2u64);

        assert!(third_insn.get_next_instruction() != None);

        // %data = load i8*, i8** %dataPtr

        let fourth_insn = third_insn.get_next_instruction().unwrap();
        assert_eq!(fourth_insn.get_opcode(), InstructionOpcode::Load);
        assert_eq!(fourth_insn.get_num_operands(), 1);

        let fourth_insn_load_operand0 = fourth_insn.get_operand_value(0).unwrap();
        assert!(fourth_insn_load_operand0.is_pointer_value());
        let fourth_insn_load_operand0_ptr_elt_t = fourth_insn_load_operand0.into_pointer_value().get_type().get_element_type();
        assert!(fourth_insn_load_operand0_ptr_elt_t.is_pointer_type());
        let ptr_to_ptr_type = fourth_insn_load_operand0_ptr_elt_t.into_pointer_type().get_element_type();
        assert!(ptr_to_ptr_type.is_int_type());
        assert!(ptr_to_ptr_type.into_int_type().get_bit_width() == 8);

        //%size = load i64, i64* %sizePtr
        assert!(fourth_insn.get_next_instruction() != None);
        let fifth_insn = fourth_insn.get_next_instruction().unwrap();
        assert_eq!(fifth_insn.get_opcode(), InstructionOpcode::Load);
        assert_eq!(fifth_insn.get_num_operands(), 1);

        let fifth_insn_load_operand0 = fifth_insn.get_operand_value(0).unwrap();
        assert!(fifth_insn_load_operand0.is_pointer_value());
        let fifth_insn_load_operand0_ptr_elt_t = fifth_insn_load_operand0.into_pointer_value().get_type().get_element_type();
        assert!(fifth_insn_load_operand0_ptr_elt_t.is_int_type());
        assert!(fifth_insn_load_operand0_ptr_elt_t.into_int_type().get_bit_width() == 64);

        // %extendedSize = sub nuw i64 %newSize, %size
        assert!(fifth_insn.get_next_instruction() != None);
        let sixth_insn = fifth_insn.get_next_instruction().unwrap();
        assert_eq!(sixth_insn.get_opcode(), InstructionOpcode::Sub);
        assert_eq!(sixth_insn.get_num_operands(), 2);
        let sixth_insn_operand0 = sixth_insn.get_operand_value(0).unwrap();
        assert_eq!(sixth_insn_operand0, size_arg);
        let sixth_insn_operand1 = sixth_insn.get_operand_value(1).unwrap();
        let load_size_use = fifth_insn.get_first_use().unwrap().get_used_value().left().unwrap();
        assert_eq!(sixth_insn_operand1, load_size_use);

        // %newMem = call i8* @realloc(i8* %data, i64 %extendedSize)
        assert!(sixth_insn.get_next_instruction() != None);
        let seventh_insn = sixth_insn.get_next_instruction().unwrap();
        assert_eq!(seventh_insn.get_opcode(), InstructionOpcode::Call);
        assert_eq!(seventh_insn.get_num_operands(), 3);

        let seventh_insn_operand0 = seventh_insn.get_operand_value(0).unwrap();
        assert!(seventh_insn_operand0.is_pointer_value());
        let data_use_from_fourth_insn = fourth_insn.get_first_use().unwrap().get_used_value().left().unwrap();
        assert_eq!(seventh_insn_operand0, data_use_from_fourth_insn);

        let seventh_insn_operand1 = seventh_insn.get_operand_value(1).unwrap();
        let size_use_from_sixth_insn = sixth_insn.get_first_use().unwrap().get_used_value().left().unwrap();
        assert_eq!(seventh_insn_operand1, size_use_from_sixth_insn);

        // %extPtr = getelementptr i8, i8* %newMem, i64 %extendedSize
        assert!(seventh_insn.get_next_instruction() != None);
        let eighth_insn = seventh_insn.get_next_instruction().unwrap();
        assert_eq!(eighth_insn.get_opcode(), InstructionOpcode::GetElementPtr);
        assert_eq!(eighth_insn.get_num_operands(), 2);

        let eighth_insn_operand0 = eighth_insn.get_operand_value(0).unwrap();
        let mem_from_seventh_insn = seventh_insn.get_first_use().unwrap().get_used_value().left().unwrap();
        assert_eq!(eighth_insn_operand0, mem_from_seventh_insn);

        let eighth_insn_operand1 = eighth_insn.get_operand_value(1).unwrap();
        assert_eq!(eighth_insn_operand1, size_use_from_sixth_insn);

        // call void @llvm.memset.p0i8.i64(i8* %extPtr, i8 0, i64 %extendedSize, i1 false)

        assert!(eighth_insn.get_next_instruction() != None);
        let ninth_insn = eighth_insn.get_next_instruction().unwrap();
        assert_eq!(ninth_insn.get_opcode(), InstructionOpcode::Call);
        assert_eq!(ninth_insn.get_num_operands(), 5);

        let ninth_insn_operand0 = ninth_insn.get_operand_value(0).unwrap();
        let mem_from_eighth_insn = eighth_insn.get_first_use().unwrap().get_used_value().left().unwrap();
        assert_eq!(ninth_insn_operand0, mem_from_eighth_insn);

        let ninth_insn_operand1 = ninth_insn.get_operand_value(1).unwrap();
        assert!(ninth_insn_operand1.is_int_value());
        assert_eq!(ninth_insn_operand1.get_type().into_int_type().get_bit_width(), 8);
        let ninth_val1 = ninth_insn_operand1.into_int_value();
        assert!(ninth_val1.is_const());
        assert_eq!(ninth_val1.get_zero_extended_constant().unwrap(), 0u64);

        let ninth_insn_operand2 = ninth_insn.get_operand_value(2).unwrap();
        assert_eq!(ninth_insn_operand2, size_use_from_sixth_insn);

        let ninth_insn_operand3 = ninth_insn.get_operand_value(3).unwrap();
        assert!(ninth_insn_operand3.is_int_value());
        assert_eq!(ninth_insn_operand3.get_type().into_int_type().get_bit_width(), 1);
        let ninth_val3= ninth_insn_operand1.into_int_value();
        assert!(ninth_val3.is_const());
        assert_eq!(ninth_val3.get_zero_extended_constant().unwrap(), 0u64);

        // store i8* %newMem, i8** %dataPtr
        assert!(ninth_insn.get_next_instruction() != None);
        let tenth_insn = ninth_insn.get_next_instruction().unwrap();
        assert_eq!(tenth_insn.get_opcode(), InstructionOpcode::Store);
        assert_eq!(tenth_insn.get_num_operands(), 2);
        let tenth_insn_operand0 = tenth_insn.get_operand_value(0).unwrap();
        let mem_from_seventh_insn = seventh_insn.get_first_use().unwrap().get_used_value().left().unwrap();
        assert_eq!(tenth_insn_operand0, mem_from_seventh_insn);
        let tenth_insn_operand1 = tenth_insn.get_operand_value(1).unwrap();
        let data_ptr_from_first_insn = first_insn.get_first_use().unwrap().get_used_value().left().unwrap();
        assert_eq!(tenth_insn_operand1, data_ptr_from_first_insn);

        // store i64 %newSize, i64* %sizePtr

        assert!(tenth_insn.get_next_instruction() != None);
        let eleventh_insn = tenth_insn.get_next_instruction().unwrap();
        assert_eq!(eleventh_insn.get_opcode(), InstructionOpcode::Store);
        assert_eq!(eleventh_insn.get_num_operands(), 2);
        let eleventh_insn_operand0 = eleventh_insn.get_operand_value(0).unwrap();
        assert_eq!(eleventh_insn_operand0, size_arg);

        let eleventh_insn_operand1 = eleventh_insn.get_operand_value(1).unwrap();
        let size_ptr_from_second_insn = second_insn.get_first_use().unwrap().get_used_value().left().unwrap();
        assert_eq!(eleventh_insn_operand1, size_ptr_from_second_insn);

        // store i64 %newSize, i64* %capPtr
        assert!(eleventh_insn.get_next_instruction() != None);
        let twelfth_insn = eleventh_insn.get_next_instruction().unwrap();
        assert_eq!(twelfth_insn.get_opcode(), InstructionOpcode::Store);
        assert_eq!(twelfth_insn.get_num_operands(), 2);

        let twelfth_insn_operand0 = twelfth_insn.get_operand_value(0).unwrap();
        assert_eq!(twelfth_insn_operand0, size_arg);
        let twelfth_insn_operand1 = twelfth_insn.get_operand_value(1).unwrap();
        let mem_cap_ptr_from_second_insn = third_insn.get_first_use().unwrap().get_used_value().left().unwrap();
        assert_eq!(twelfth_insn_operand1, mem_cap_ptr_from_second_insn);

        assert!(twelfth_insn.get_next_instruction() != None);
        let thirteenth_insn = twelfth_insn.get_next_instruction().unwrap();
        assert_eq!(thirteenth_insn.get_opcode(), InstructionOpcode::Return);
        assert_eq!(thirteenth_insn.get_num_operands(), 0);

        assert!(thirteenth_insn.get_next_instruction().is_none());
    }
}

