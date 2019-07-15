#![allow(dead_code)]

pub mod divmod;
pub mod exp;
use inkwell::values::IntValue;
use super::JITContext;

use self::exp::ExpDeclarationManager;
use self::divmod::DivModDeclarationManager;

pub trait LibraryFunctions {
    fn exp(&self, arg1 : IntValue, arg2 : IntValue) -> IntValue;
    fn udiv256(&self, arg1 : IntValue, arg2 : IntValue) -> IntValue;
    fn sdiv256(&self, arg1 : IntValue, arg2 : IntValue) -> IntValue;
    fn umod256(&self, arg1 : IntValue, arg2 : IntValue) -> IntValue;
    fn smod256(&self, arg1 : IntValue, arg2 : IntValue) -> IntValue;
    fn umod512(&self, arg1 : IntValue, arg2 : IntValue) -> IntValue;
}

pub struct JitLibraryFunctions<'a> {
    m_context: &'a JITContext,
    m_exp_decl: ExpDeclarationManager<'a>,
    m_divmod_decl: DivModDeclarationManager<'a>
}

impl<'a> JitLibraryFunctions<'a> {
    fn new(context: &'a JITContext) -> Self {
        JitLibraryFunctions {
            m_context: context,
            m_exp_decl: ExpDeclarationManager::new(context),
            m_divmod_decl: DivModDeclarationManager::new(context)
        }
    }
}

impl<'a> LibraryFunctions for JitLibraryFunctions<'a> {
    fn exp(&self, arg1 : IntValue, arg2 : IntValue) -> IntValue {
        self.m_exp_decl.exp(arg1, arg2)
    }

    fn udiv256(&self, arg1 : IntValue, arg2 : IntValue) -> IntValue {
        let udiv_256_func = self.m_divmod_decl.create_udiv256_func();
        let builder = self.m_context.builder();
        let udiv256result = builder.build_call(udiv_256_func, &[arg1.into(), arg2.into()], "");
        let ret_val_256 = udiv256result.try_as_basic_value().left().unwrap().into_int_value();

        ret_val_256
    }

    fn sdiv256(&self, arg1 : IntValue, arg2 : IntValue) -> IntValue {
        let sdiv_256_func = self.m_divmod_decl.create_sdiv256_func();
        let builder = self.m_context.builder();
        let sdiv256result = builder.build_call(sdiv_256_func, &[arg1.into(), arg2.into()], "");
        let ret_val_256 = sdiv256result.try_as_basic_value().left().unwrap().into_int_value();

        ret_val_256
    }

    fn umod256(&self, arg1 : IntValue, arg2 : IntValue) -> IntValue {
        let umod_256_func = self.m_divmod_decl.create_umod256_func();
        let builder = self.m_context.builder();
        let umod256result = builder.build_call(umod_256_func, &[arg1.into(), arg2.into()], "");
        let ret_val_256 = umod256result.try_as_basic_value().left().unwrap().into_int_value();

        ret_val_256
    }

    fn smod256(&self, arg1 : IntValue, arg2 : IntValue) -> IntValue {
        let smod_256_func = self.m_divmod_decl.create_smod256_func();
        let builder = self.m_context.builder();
        let smod256result = builder.build_call(smod_256_func, &[arg1.into(), arg2.into()], "");
        let ret_val_256 = smod256result.try_as_basic_value().left().unwrap().into_int_value();

        ret_val_256
    }

    fn umod512(&self, arg1 : IntValue, arg2 : IntValue) -> IntValue {
        let umod_512_func = self.m_divmod_decl.create_umod512_func();
        let builder = self.m_context.builder();
        let umod512result = builder.build_call(umod_512_func, &[arg1.into(), arg2.into()], "");
        let ret_val_512 = umod512result.try_as_basic_value().left().unwrap().into_int_value();

        ret_val_512        
    }
}