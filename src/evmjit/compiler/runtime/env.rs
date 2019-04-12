#![allow(dead_code)]
use inkwell::context::Context;
use inkwell::types::StructType;
use inkwell::types::PointerType;
use inkwell::AddressSpace;
use std::ffi::CString;

#[derive(Debug)]

pub struct EnvDataType
{
    env_type: StructType,
    env_ptr_type: PointerType,
}

impl EnvDataType {
    pub fn new(context: &Context) -> Self {
        let env_t = context.opaque_struct_type("Env");
        
        EnvDataType {
            env_type : env_t,
            env_ptr_type : env_t.ptr_type(AddressSpace::Generic)
        }
    }

    pub fn get_type(&self) -> StructType {
        self.env_type
    }

    pub fn get_ptr_type(&self) -> PointerType {
        self.env_ptr_type
    }

    pub fn is_env_data_type(a_struct: &StructType) -> bool {
        if a_struct.count_fields() != 0 {
            return false;
        }

        if a_struct.is_sized() {
            return false;
        }
        
        if a_struct.is_packed() {
            return false;
        }
            
        if !a_struct.is_opaque() {
            return false;
        }
        
        if a_struct.get_name() != Some(&*CString::new("Env").unwrap()) {
            return false;
        }

        return true;
    }
}

#[test]
fn test_env_data_type() {
    let context = Context::create();
    let env_data_type_singleton = EnvDataType::new(&context);
    let env_data_t = env_data_type_singleton.get_type();

    assert!(EnvDataType::is_env_data_type(&env_data_t));
    
    let env_data_ptr_t = env_data_type_singleton.get_ptr_type();
    assert!(env_data_ptr_t.get_element_type().is_struct_type());
    assert!(EnvDataType::is_env_data_type (env_data_ptr_t.get_element_type().as_struct_type()));
}
