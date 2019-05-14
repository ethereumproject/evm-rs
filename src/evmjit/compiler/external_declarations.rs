#![allow(dead_code)]

use std::cell::RefCell;

use inkwell::module::Linkage::*;
use inkwell::values::FunctionValue;

use super::util::funcbuilder::*;
use super::JITContext;

/// Trait representing the interface to managers of LLVM function declarations.
pub trait DeclarationManager<'a> {
    /// Initialization method. Takes the context to which declarations should be bound, and
    /// constructs all the declarations.
    fn new(context: &'a JITContext) -> Self;

    /// Returns a declaration corresponding to the given string. If no declaration exists for the
    /// passed string, then panic.
    fn get_decl(&self, name: &str) -> FunctionValue;
}

pub struct ExternalFunctionManager<'a> {
    m_context: &'a JITContext,
    malloc_decl: RefCell<Option<FunctionValue>>,
    free_decl: RefCell<Option<FunctionValue>>,
    realloc_decl: RefCell<Option<FunctionValue>>,
}

impl<'a> DeclarationManager<'a> for ExternalFunctionManager<'a> {
    fn new(context: &'a JITContext) -> Self {
        ExternalFunctionManager {
            m_context: context,
            malloc_decl: RefCell::new(None),
            free_decl: RefCell::new(None),
            realloc_decl: RefCell::new(None),
        }
    }

    fn get_decl(&self, name: &str) -> FunctionValue {
        match name {
            "malloc" => {
                if self.malloc_decl.borrow().is_some() {
                    self.malloc_decl.borrow().unwrap()
                } else {
                    let malloc_func = self.init_malloc();
                    *self.malloc_decl.borrow_mut() = Some(malloc_func);
                    malloc_func
                }
            }
            "free" => {
                if self.free_decl.borrow().is_some() {
                    self.free_decl.borrow().unwrap()
                } else {
                    let free_func = self.init_free();
                    *self.free_decl.borrow_mut() = Some(free_func);
                    free_func
                }
            }
            "realloc" => {
                if self.realloc_decl.borrow().is_some() {
                    self.realloc_decl.borrow().unwrap()
                } else {
                    let realloc_func = self.init_realloc();
                    *self.realloc_decl.borrow_mut() = Some(realloc_func);
                    realloc_func
                }
            }
            _ => panic!(format!("Declaration manager was requested an invalid import: {}", name)),
        }
    }
}

impl<'a> ExternalFunctionManager<'a> {
    fn init_malloc(&self) -> FunctionValue {
        let module = self.m_context.module();
        let types = self.m_context.evm_types();
        let attr_factory = self.m_context.attributes();

        let malloc_fn_type = FunctionTypeBuilder::new(self.m_context.llvm_context())
            .returns(types.get_word_ptr_type())
            .arg(types.get_size_type())
            .build()
            .unwrap();

        let malloc_func = module.add_function("malloc", malloc_fn_type, Some(External));

        malloc_func.add_attribute(0, *attr_factory.attr_nounwind());
        malloc_func.add_attribute(0, *attr_factory.attr_noalias());

        malloc_func
    }

    fn init_free(&self) -> FunctionValue {
        let module = self.m_context.module();
        let types = self.m_context.evm_types();
        let attr_factory = self.m_context.attributes();

        let free_ret_type = self.m_context.llvm_context().void_type();
        let arg1 = types.get_word_ptr_type();

        let free_func_type = FunctionTypeBuilder::new(self.m_context.llvm_context())
            .returns(free_ret_type)
            .arg(arg1)
            .build()
            .unwrap();

        let free_func = module.add_function("free", free_func_type, Some(External));

        free_func.add_attribute(0, *attr_factory.attr_nounwind());
        free_func.add_attribute(1, *attr_factory.attr_nocapture());

        free_func
    }

    fn init_realloc(&self) -> FunctionValue {
        let module = self.m_context.module();
        let types = self.m_context.evm_types();
        let attr_factory = self.m_context.attributes();

        let realloc_return_type = types.get_byte_ptr_type();
        let arg1 = types.get_byte_ptr_type();
        let arg2 = types.get_size_type();
        let realloc_func_type = FunctionTypeBuilder::new(self.m_context.llvm_context())
            .returns(realloc_return_type)
            .arg(arg1)
            .arg(arg2)
            .build()
            .unwrap();

        let realloc_func = module.add_function("realloc", realloc_func_type, Some(External));

        realloc_func.add_attribute(0, *attr_factory.attr_noalias());
        realloc_func.add_attribute(0, *attr_factory.attr_nounwind());
        realloc_func.add_attribute(1, *attr_factory.attr_nocapture());

        realloc_func
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;

    use inkwell::attributes::Attribute;

    use super::*;

    #[test]
    fn test_get_malloc_decl() {
        let jitctx = JITContext::new();
        let module = jitctx.module();

        let evmtypes = jitctx.evm_types();
        let attr_factory = jitctx.attributes();

        let decl_manager = ExternalFunctionManager::new(&jitctx);
        let malloc_func_optional = module.get_function("malloc");
        assert!(malloc_func_optional.is_none());

        let malloc_func = decl_manager.get_decl("malloc");
        assert_eq!(malloc_func.count_params(), 1);
        // Free function has one attribute (nounwind)
        assert_eq!(malloc_func.count_attributes(0), 2);
        assert!(malloc_func.get_linkage() == External);
        assert_eq!(*malloc_func.get_name(), *CString::new("malloc").unwrap());

        let size_arg = malloc_func.get_nth_param(0).unwrap();
        assert!(size_arg.is_int_value());
        assert_eq!(size_arg.into_int_value().get_type(), evmtypes.get_size_type());

        let malloc_ret = malloc_func.get_return_type();
        assert!(malloc_ret.is_pointer_type());

        let elem_t = malloc_ret.into_pointer_type().get_element_type();
        assert!(elem_t.is_int_type());
        assert_eq!(elem_t.into_int_type(), evmtypes.get_word_type());

        let nounwind_attr = malloc_func.get_enum_attribute(0, Attribute::get_named_enum_kind_id("nounwind"));
        assert!(nounwind_attr != None);

        let noalias_attr = malloc_func.get_enum_attribute(0, Attribute::get_named_enum_kind_id("noalias"));
        assert!(noalias_attr != None);

        assert_eq!(
            attr_factory.attr_nounwind().get_enum_kind_id(),
            nounwind_attr.unwrap().get_enum_kind_id()
        );
        assert_eq!(
            attr_factory.attr_noalias().get_enum_kind_id(),
            noalias_attr.unwrap().get_enum_kind_id()
        );
    }

    #[test]
    fn test_get_free_decl() {
        let jitctx = JITContext::new();
        let module = jitctx.module();

        let evmtypes = jitctx.evm_types();
        let attr_factory = jitctx.attributes();

        let decl_manager = ExternalFunctionManager::new(&jitctx);
        let free_func_optional = module.get_function("free");
        assert!(free_func_optional.is_none());

        let free_func = decl_manager.get_decl("free");
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
        assert_eq!(area_to_be_freed_ptr_elt_t.into_int_type(), evmtypes.get_word_type());

        let nounwind_attr = free_func.get_enum_attribute(0, Attribute::get_named_enum_kind_id("nounwind"));
        assert!(nounwind_attr != None);
        assert_eq!(
            attr_factory.attr_nounwind().get_enum_kind_id(),
            nounwind_attr.unwrap().get_enum_kind_id()
        );

        let nocapture_attr = free_func.get_enum_attribute(1, Attribute::get_named_enum_kind_id("nocapture"));
        assert!(nocapture_attr != None);
        assert_eq!(
            attr_factory.attr_nocapture().get_enum_kind_id(),
            nocapture_attr.unwrap().get_enum_kind_id()
        );
    }

    #[test]
    fn test_get_realloc_decl() {
        let jitctx = JITContext::new();
        let module = jitctx.module();

        let evmtypes = jitctx.evm_types();
        let attr_factory = jitctx.attributes();

        let decl_manager = ExternalFunctionManager::new(&jitctx);
        let realloc_func_optional = module.get_function("realloc");
        assert!(realloc_func_optional.is_none());

        let realloc_func = decl_manager.get_decl("realloc");
        assert_eq!(*realloc_func.get_name(), *CString::new("realloc").unwrap());
        assert_eq!(realloc_func.count_params(), 2);
        assert_eq!(realloc_func.count_attributes(0), 2);
        assert_eq!(realloc_func.count_attributes(1), 1);
        assert!(realloc_func.get_linkage() == External);

        // Validate argument 1 type

        let old_memory_to_realloc_arg = realloc_func.get_nth_param(0).unwrap();
        assert!(old_memory_to_realloc_arg.is_pointer_value());
        let elem_t = old_memory_to_realloc_arg
            .into_pointer_value()
            .get_type()
            .get_element_type();
        assert_eq!(elem_t.into_int_type(), evmtypes.get_byte_type());

        // Validate argument 2 type

        let new_memory_size_arg = realloc_func.get_nth_param(1).unwrap();
        assert!(new_memory_size_arg.is_int_value());
        assert_eq!(
            new_memory_size_arg.into_int_value().get_type(),
            evmtypes.get_size_type()
        );

        // Validate return type
        let realloc_ret = realloc_func.get_return_type();
        assert!(realloc_ret.is_pointer_type());
        let ret_elem_t = realloc_ret.into_pointer_type().get_element_type();
        assert!(ret_elem_t.is_int_type());
        assert_eq!(ret_elem_t.into_int_type(), evmtypes.get_byte_type());

        // Validate function attributes
        let nounwind_attr = realloc_func.get_enum_attribute(0, Attribute::get_named_enum_kind_id("nounwind"));
        assert!(nounwind_attr != None);

        assert_eq!(
            attr_factory.attr_nounwind().get_enum_kind_id(),
            nounwind_attr.unwrap().get_enum_kind_id()
        );

        let noalias_attr = realloc_func.get_enum_attribute(0, Attribute::get_named_enum_kind_id("noalias"));
        assert!(noalias_attr != None);

        assert_eq!(
            attr_factory.attr_noalias().get_enum_kind_id(),
            noalias_attr.unwrap().get_enum_kind_id()
        );

        // Validate parameter attribute
        let nocapture_attr = realloc_func.get_enum_attribute(1, Attribute::get_named_enum_kind_id("nocapture"));
        assert!(nocapture_attr != None);

        assert_eq!(
            attr_factory.attr_nocapture().get_enum_kind_id(),
            nocapture_attr.unwrap().get_enum_kind_id()
        );
    }
}
