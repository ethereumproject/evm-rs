/// Builder API for ergonomic function building.

use std::error::Error;
use std::fmt;

use inkwell::context::Context;
use inkwell::types::FunctionType;
use inkwell::types::AnyTypeEnum;
use inkwell::types::BasicTypeEnum;

/// Function type builder. Return type defaults to Void.
pub struct FunctionTypeBuilder<'a> {
    m_ctx: &'a Context,
    m_ret: AnyTypeEnum,
    m_args: Vec<BasicTypeEnum>,
}

/// Function type builder error.
#[derive(PartialEq)]
pub enum FunctionTypeBuilderError {
    InvalidReturnType,
    Custom(String),
}

impl<'a> FunctionTypeBuilder<'a> {
    /// Initialize a new builder with a given LLVM context.
    pub fn new(context: &'a Context) -> Self {
        FunctionTypeBuilder {
            m_ctx: context,
            m_ret: context.void_type().into(),
            m_args: Vec::new(),
        }
    }
    
    /// Consume the builder and return an LLVM function signature..
    pub fn build(self) -> Result<FunctionType, FunctionTypeBuilderError> {
        // Arguments don't need validation because BasicTypeEnum doesn't include any
        // non-first-class types.
        if !self.return_is_valid() {
            Err(FunctionTypeBuilderError::InvalidReturnType)
        } else {
            // TODO: Support var_args, reduce code duplication
            match self.m_ret {
                AnyTypeEnum::ArrayType(t) => Ok(t.fn_type(self.m_args.as_slice(), false)),
                AnyTypeEnum::FloatType(t) => Ok(t.fn_type(self.m_args.as_slice(), false)),
                AnyTypeEnum::IntType(t) => Ok(t.fn_type(self.m_args.as_slice(), false)),
                AnyTypeEnum::PointerType(t) => Ok(t.fn_type(self.m_args.as_slice(), false)),
                AnyTypeEnum::StructType(t) => Ok(t.fn_type(self.m_args.as_slice(), false)),
                AnyTypeEnum::VectorType(t) => Ok(t.fn_type(self.m_args.as_slice(), false)),
                AnyTypeEnum::VoidType(t) => Ok(t.fn_type(self.m_args.as_slice(), false)),
                _ => panic!() // this should never be reached because of the previous validation step.
            }
        }
    }

    /// Get the return type of the function.
    pub fn get_return_type(&self) -> AnyTypeEnum {
        self.m_ret
    }

    /// Get the argument list.
    pub fn get_args(&self) -> &Vec<BasicTypeEnum> {
        &self.m_args
    }

    /// Set the return type of the function.
    pub fn returns<T>(mut self, ret: T) -> Self 
        where T: Into<AnyTypeEnum>
    {
        self.m_ret = ret.into();
        self
    }
    
    /// Add an argument to the end of the builder's argument list.
    pub fn arg<T>(mut self, argument: T) -> Self
        where T: Into<BasicTypeEnum>
    {
        self.m_args.push(argument.into());
        self
    }
    
    /// Ensure the function built has a valid return type. True if valid.
    fn return_is_valid(&self) -> bool {
        !self.m_ret.is_function_type()
    }
}

impl From<String> for FunctionTypeBuilderError {
    fn from(msg: String) -> Self {
        FunctionTypeBuilderError::Custom(msg)
    }
}

impl fmt::Display for FunctionTypeBuilderError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            self.description(),
        )
    }
}

impl fmt::Debug for FunctionTypeBuilderError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            self.description(),
        )
    }
}

impl Error for FunctionTypeBuilderError {
    fn description(&self) -> &str {
        match self {
            FunctionTypeBuilderError::InvalidReturnType => "Invalid return type",
            FunctionTypeBuilderError::Custom(msg) => msg,
        }
    }

    fn cause(&self) -> Option<&Error> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use evmjit::compiler::evmtypes::EvmTypes;
    use singletonum::Singleton;

    #[test]
    fn builder_noret_noargs() {
        let context = Context::create();
        let func = FunctionTypeBuilder::new(&context)
            .build()
            .unwrap();

        assert_eq!(func, context.void_type().fn_type(&[], false));
    }

    #[test]
    fn builder_intret_noargs() {
        let context = Context::create();
        let func = FunctionTypeBuilder::new(&context)
            .returns(context.i64_type())
            .build()
            .unwrap();

        assert_eq!(func, context.i64_type().fn_type(&[], false));
    }

    #[test]
    fn builder_bad_ret() {
        let context = Context::create();
        let err = FunctionTypeBuilder::new(&context)
            .returns(context.void_type().fn_type(&[], false))
            .build();

        assert_eq!(FunctionTypeBuilderError::InvalidReturnType, err.unwrap_err());
    }

    #[test]
    fn builder_evm_func() {
        // test a SHA3 callback signature
        let context = Context::create();
        let types = EvmTypes::get_instance(&context);
        let func = FunctionTypeBuilder::new(&context)
            .returns(context.void_type())
            .arg(types.get_byte_ptr_type())
            .arg(types.get_size_type())
            .build()
            .unwrap();

        assert_eq!(
            func, 
            context.void_type()
                .fn_type(
                    &[types.get_byte_ptr_type().into(), types.get_size_type().into()],
                    false
                )
        );
    }
}
