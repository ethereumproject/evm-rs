#![allow(dead_code)]

use evmjit::compiler::evmtypes::EvmTypes;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::values::FunctionValue;
use inkwell::module::Linkage;
use inkwell::types::FunctionType;
use inkwell::attributes::Attribute;
use evmjit::LLVMAttributeFactory;
use std::cell::RefCell;
use inkwell::module::Linkage::*;
use singletonum::Singleton;

#[cfg(feature = "std")] use std::collections::HashMap as Map;
#[cfg(not(feature = "std"))] use alloc::collections::BTreeMap as Map;

/// Function declaration description. Can be passed to ExternalFunctionManager.
pub trait FuncDecl {
    fn identifier(&self) -> String;
    fn signature(&self) -> FunctionType;
    fn attrs(&self) -> Vec<(u32, Attribute)>;
    fn linkage(&self) -> Option<Linkage>;
}

// Stub structs for function declarations.
pub struct MallocDecl<'a>(&'a Context);
pub struct FreeDecl<'a>(&'a Context);
pub struct ReallocDecl<'a>(&'a Context);

pub struct ExternalFunctionManager<'a> {
    m_context: &'a Context,
    m_module: &'a Module,
    m_decls: RefCell<Map<String, FunctionValue>>,
}

impl<'a> ExternalFunctionManager<'a> {
    pub fn new(context: &'a Context, module: &'a Module) -> ExternalFunctionManager<'a> {
        let ret = ExternalFunctionManager {
            m_context: context,
            m_module: module,
            m_decls: RefCell::new(Map::new()),
        };

        ret
    }

    /// Gets a declaration by its name in the module.
    pub fn get_decl_by_name<T>(&self, id: T) -> Option<FunctionValue>
        where T: Into<String>
    {
         if let Some(decl) = self.m_decls.borrow().get(&id.into()) {
             Some(decl.clone())
         } else {
             None
         }
    }

    /// Gets a declaration given a struct implementing FuncDecl.
    pub fn get_decl<T>(&self, decl: T) -> FunctionValue 
        where T: FuncDecl 
    {
        let mut map = self.m_decls.borrow_mut();
        
        // NOTE: this essentially enforces that a function with a given identifier can only be
        // declared once.
        if let Some(func) = map.get(&decl.identifier()) {
            return func.clone();
        }

        let ret = self.m_module.add_function(decl.identifier().as_str(), decl.signature(), decl.linkage());

        for (idx, attr) in decl.attrs().iter() {
            ret.add_attribute(*idx, *attr);
        }

        map.insert(decl.identifier(), ret);

        ret
        
    }
}

// Perhaps a decl factory would be useful, a struct that simply returns a trait object Box<dyn FuncDecl>
// when passed a known name string.
impl<'a> MallocDecl<'a> {
    pub fn new(context: &'a Context) -> Self {
        MallocDecl(context)
    }
}

impl<'a> FreeDecl<'a> {
    pub fn new(context: &'a Context) -> Self {
        FreeDecl(context)
    }
}

impl<'a> ReallocDecl<'a> {
    pub fn new(context: &'a Context) -> Self {
        ReallocDecl(context)
    }
}

impl<'a> FuncDecl for MallocDecl<'a> {
    fn identifier(&self) -> String {
        "malloc".to_string()
    }

    fn signature(&self) -> FunctionType {
        // TODO: Update for function type builder api when merged
        let types_instance = EvmTypes::get_instance(self.0);
        types_instance.get_word_ptr_type().fn_type(&[types_instance.get_size_type().into()], false)
    }

    fn attrs(&self) -> Vec<(u32, Attribute)> {
        let attr_factory = LLVMAttributeFactory::get_instance(&self.0);
        vec![
            (0, *attr_factory.attr_nounwind()),
            (0, *attr_factory.attr_noalias()),
        ]
    }

    fn linkage(&self) -> Option<Linkage> {
        Some(External)
    }
}

impl<'a> FuncDecl for FreeDecl<'a> {
    fn identifier(&self) -> String {
        "free".to_string()
    }

    fn signature(&self) -> FunctionType { 
        // TODO: Update for function type builder api when merged
        let types_instance = EvmTypes::get_instance(self.0);
        types_instance.get_void_type().fn_type(&[types_instance.get_word_ptr_type().into()], false)
    }

    fn attrs(&self) -> Vec<(u32, Attribute)> {
        let attr_factory = LLVMAttributeFactory::get_instance(&self.0);
        vec![
            (0, *attr_factory.attr_nounwind()),
            (1, *attr_factory.attr_nocapture()),
        ]
    }

    fn linkage(&self) -> Option<Linkage> {
        Some(External)
    }
}

impl<'a> FuncDecl for ReallocDecl<'a> {
    fn identifier(&self) -> String {
        "realloc".to_string()
    }

    fn signature(&self) -> FunctionType {
        // TODO: Update for function type builder api when merged
        let types_instance = EvmTypes::get_instance(self.0);
        types_instance.get_byte_ptr_type().fn_type(&[types_instance.get_byte_ptr_type().into(), types_instance.get_size_type().into()], false)
    }

    fn attrs(&self) -> Vec<(u32, Attribute)> {
        let attr_factory = LLVMAttributeFactory::get_instance(&self.0);
        vec![
            (0, *attr_factory.attr_noalias()),
            (0, *attr_factory.attr_nounwind()),
            (1, *attr_factory.attr_nocapture()),
        ]
    }

    fn linkage(&self) -> Option<Linkage> {
        Some(External)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use inkwell::attributes::Attribute;
    use std::ffi::CString;

    #[test]
    fn test_get_malloc_decl() {
        let context = Context::create();
        let module = context.create_module("my_module");

        let attr_factory = LLVMAttributeFactory::get_instance(&context);

        let decl_manager = ExternalFunctionManager::new(&context, &module);
        let malloc_func_optional = module.get_function("malloc");
        assert!(malloc_func_optional.is_none());

        let malloc_func = decl_manager.get_decl(MallocDecl::new(&context));
        assert_eq!(malloc_func.count_params(), 1);
        // Free function has one attribute (nounwind)
        assert_eq!(malloc_func.count_attributes(0), 2);
        assert!(malloc_func.get_linkage() == External);
        assert_eq!(*malloc_func.get_name(), *CString::new("malloc").unwrap());

        let size_arg = malloc_func.get_nth_param(0).unwrap();
        assert!(size_arg.is_int_value());
        assert_eq!(size_arg.into_int_value().get_type(), context.i64_type());

        let malloc_ret = malloc_func.get_return_type();
        assert!(malloc_ret.is_pointer_type());

        let elem_t = malloc_ret.into_pointer_type().get_element_type();
        assert!(elem_t.is_int_type());
        assert_eq!(elem_t.into_int_type(), context.custom_width_int_type(256));

        let nounwind_attr = malloc_func.get_enum_attribute(0, Attribute::get_named_enum_kind_id("nounwind"));
        assert!(nounwind_attr != None);

        let noalias_attr = malloc_func.get_enum_attribute(0, Attribute::get_named_enum_kind_id("noalias"));
        assert!(noalias_attr != None);

        assert_eq!(nounwind_attr.unwrap(), *attr_factory.attr_nounwind());
        assert_eq!(noalias_attr.unwrap(), *attr_factory.attr_noalias());
    }

    #[test]
    fn test_get_free_decl() {
        let context = Context::create();
        let module = context.create_module("my_module");

        let attr_factory = LLVMAttributeFactory::get_instance(&context);

        let decl_manager = ExternalFunctionManager::new(&context, &module);
        let free_func_optional = module.get_function("free");
        assert!(free_func_optional.is_none());

        let free_func = decl_manager.get_decl(FreeDecl::new(&context));
        assert_eq!(*free_func.get_name(), *CString::new("free").unwrap());
        assert_eq!(free_func.count_params(), 1);

        // Free function has one attribute (nounwind)
        assert_eq!(free_func.count_attributes(0), 1);

        // Free function parameter has one attribute (nocapture)
        assert_eq!(free_func.count_attributes(1), 1);

        assert!(free_func.get_linkage() == External);

        let area_to_be_freed_arg = free_func.get_nth_param(0).unwrap();
        assert!(area_to_be_freed_arg.is_pointer_value());

        let area_to_be_freed_ptr_elt_t = area_to_be_freed_arg.into_pointer_value().get_type().get_element_type();
        assert!(area_to_be_freed_ptr_elt_t.is_int_type());
        assert_eq!(area_to_be_freed_ptr_elt_t.into_int_type(), context.custom_width_int_type(256));


        let nounwind_attr = free_func.get_enum_attribute(0, Attribute::get_named_enum_kind_id("nounwind"));
        assert!(nounwind_attr != None);
        assert_eq!(nounwind_attr.unwrap(), *attr_factory.attr_nounwind());

        let nocapture_attr = free_func.get_enum_attribute(1, Attribute::get_named_enum_kind_id("nocapture"));
        assert!(nocapture_attr != None);
        assert_eq!(nocapture_attr.unwrap(), *attr_factory.attr_nocapture());
    }

    #[test]
    fn test_get_realloc_decl() {
        let context = Context::create();
        let module = context.create_module("my_module");

        let attr_factory = LLVMAttributeFactory::get_instance(&context);

        let decl_manager = ExternalFunctionManager::new(&context, &module);
        let realloc_func_optional = module.get_function("realloc");
        assert!(realloc_func_optional.is_none());

        let realloc_func = decl_manager.get_decl(ReallocDecl::new(&context));
        assert_eq!(*realloc_func.get_name(), *CString::new("realloc").unwrap());
        assert_eq!(realloc_func.count_params(), 2);
        assert_eq!(realloc_func.count_attributes(0), 2);
        assert_eq!(realloc_func.count_attributes(1), 1);
        assert!(realloc_func.get_linkage() == External);

        // Validate argument 1 type

        let old_memory_to_realloc_arg = realloc_func.get_nth_param(0).unwrap();
        assert!(old_memory_to_realloc_arg.is_pointer_value());
        let elem_t = old_memory_to_realloc_arg.into_pointer_value().get_type().get_element_type();
        assert_eq!(elem_t.into_int_type(), context.i8_type());

        // Validate argument 2 type

        let new_memory_size_arg = realloc_func.get_nth_param(1).unwrap();
        assert!(new_memory_size_arg.is_int_value());
        assert_eq!(new_memory_size_arg.into_int_value().get_type(), context.i64_type());

        // Validate return type
        let realloc_ret = realloc_func.get_return_type();
        assert!(realloc_ret.is_pointer_type());
        let ret_elem_t = realloc_ret.into_pointer_type().get_element_type();
        assert!(ret_elem_t.is_int_type());
        assert_eq!(ret_elem_t.into_int_type(), context.i8_type());

        // Validate function attributes
        let nounwind_attr = realloc_func.get_enum_attribute(0, Attribute::get_named_enum_kind_id("nounwind"));
        assert!(nounwind_attr != None);
        assert_eq!(nounwind_attr.unwrap(), *attr_factory.attr_nounwind());

        let noalias_attr = realloc_func.get_enum_attribute(0, Attribute::get_named_enum_kind_id("noalias"));
        assert!(noalias_attr != None);
        assert_eq!(noalias_attr.unwrap(), *attr_factory.attr_noalias());

        // Validate parameter attribute

        let nocapture_attr = realloc_func.get_enum_attribute(1, Attribute::get_named_enum_kind_id("nocapture"));
        assert!(nocapture_attr != None);
        assert_eq!(nocapture_attr.unwrap(), *attr_factory.attr_nocapture());

    }
}
