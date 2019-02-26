use evmjit::compiler::evmtypes::EvmTypes;
use evmjit::compiler::runtime::env::EnvDataType;
use evmjit::compiler::util::funcbuilder::*;

use inkwell::context::Context;
use inkwell::builder::Builder;
use inkwell::module::Module;
//use inkwell::values::FunctionValue;
use inkwell::types::FunctionType;
use singletonum::{Singleton, SingletonInit};

//#[cfg(feature = "std")] use std::collections::HashMap as Map;
//#[cfg(not(feature = "std"))] use alloc::collections::btree_map::BTreeMap as Map;

/// Structure handling JIT callback interface. 
/// CallbackInterfaceManager is responsible for handling linkage needed for EVM opcodes that
/// access state or otherwise cannot be implemented solely through codegen.
pub struct CallbackInterfaceManager<'a> {
    m_context: &'a Context,
    m_builder: &'a Builder,
    m_module: &'a Module,
}


/// CallbackTypes provides function signatures for each callback function provided to the JIT.
#[derive(Singleton)]
pub struct CallbackTypes {
    m_storageload: FunctionType,
    m_storagestore: FunctionType,

    m_balance: FunctionType,
    m_calldataload: FunctionType,
    m_create: FunctionType,
    m_blockhash: FunctionType,

    m_sha3: FunctionType,

    m_extcodesize: FunctionType,
    m_extcodecopy: FunctionType,

    m_log: FunctionType,

    m_selfdestruct: FunctionType,

    m_call: FunctionType,
}

unsafe impl Sync for CallbackTypes {}
unsafe impl Send for CallbackTypes {}

impl<'a> CallbackInterfaceManager<'a> {
    pub fn new(context: &'a Context, builder: &'a Builder, module: &'a Module) -> CallbackInterfaceManager<'a> {
        CallbackInterfaceManager {
            m_context: context,
            m_builder: builder,
            m_module: module,
        }
    }
}

macro_rules! get_type_impl {
    ($method_name:ident, $member: ident) => {
        pub fn $method_name(&self) -> FunctionType {
            self.$member 
        }
    }
}

impl CallbackTypes {
    get_type_impl!(get_type_sload, m_storageload);
    get_type_impl!(get_type_sstore, m_storagestore);
    get_type_impl!(get_type_balance, m_balance);
    get_type_impl!(get_type_calldataload, m_calldataload);
    get_type_impl!(get_type_create, m_create);
    get_type_impl!(get_type_blockhash, m_blockhash);
    get_type_impl!(get_type_sha3, m_sha3);
    get_type_impl!(get_type_extcodesize, m_extcodesize);
    get_type_impl!(get_type_extcodecopy, m_extcodecopy);
    get_type_impl!(get_type_log, m_log);
    get_type_impl!(get_type_selfdestruct, m_selfdestruct);
    get_type_impl!(get_type_call, m_call);
}

impl SingletonInit for CallbackTypes {
    type Init = Context;
    fn init(context: &Context) -> Self {
        // Access EVM and environment types
        let evm = EvmTypes::get_instance(context);
        let env = EnvDataType::get_instance(context);
        // TODO: double check these signatures
        CallbackTypes {
            m_storageload: FunctionTypeBuilder::new(context)
                .arg(env.get_ptr_type())
                .arg(evm.get_address_ptr_type())
                .arg(evm.get_word_ptr_type())
                .arg(evm.get_word_ptr_type())
                .build()
                .unwrap(),
            m_storagestore: FunctionTypeBuilder::new(context)
                .arg(env.get_ptr_type())
                .arg(evm.get_address_ptr_type())
                .arg(evm.get_word_ptr_type())
                .arg(evm.get_word_ptr_type())
                .build()
                .unwrap(),
            m_balance: FunctionTypeBuilder::new(context)
                .arg(env.get_ptr_type())
                .arg(evm.get_address_type())
                .arg(evm.get_word_ptr_type())
                .build()
                .unwrap(),
            m_calldataload: FunctionTypeBuilder::new(context)
                .build()
                .unwrap(),
            m_create: FunctionTypeBuilder::new(context)
                .arg(env.get_ptr_type())
                .arg(evm.get_gas_ptr_type())
                .arg(evm.get_word_ptr_type())
                .arg(evm.get_byte_ptr_type())
                .arg(evm.get_size_type())
                .arg(evm.get_word_ptr_type())
                .build()
                .unwrap(),
            m_blockhash: FunctionTypeBuilder::new(context)
                .arg(env.get_ptr_type())
                .arg(evm.get_word_ptr_type())
                .arg(evm.get_word_ptr_type())
                .build()
                .unwrap(),
            m_sha3: FunctionTypeBuilder::new(context)
                .arg(evm.get_byte_ptr_type())
                .arg(evm.get_size_type())
                .build()
                .unwrap(),
            m_extcodesize: FunctionTypeBuilder::new(context) // maybe incorrect
                .arg(evm.get_address_ptr_type())
                .build()
                .unwrap(),
            m_extcodecopy: FunctionTypeBuilder::new(context) // maybe incorrect
                .arg(evm.get_address_ptr_type())
                .arg(evm.get_byte_ptr_type())
                .build()
                .unwrap(),
            m_log: FunctionTypeBuilder::new(context)
                .arg(env.get_ptr_type())
                .arg(evm.get_byte_ptr_type())
                .arg(evm.get_size_type())
                .arg(evm.get_word_ptr_type())
                .arg(evm.get_word_ptr_type())
                .arg(evm.get_word_ptr_type())
                .arg(evm.get_word_ptr_type())
                .build()
                .unwrap(),
            m_selfdestruct: FunctionTypeBuilder::new(context)
                .arg(env.get_ptr_type())
                .arg(evm.get_address_ptr_type())
                .arg(evm.get_address_ptr_type())
                .build()
                .unwrap(),
            m_call: FunctionTypeBuilder::new(context) // maybe incorrect
                .returns(evm.get_bool_type())
                .arg(env.get_ptr_type())
                .arg(evm.get_gas_ptr_type())
                .arg(evm.get_gas_type())
                .arg(evm.get_word_ptr_type())
                .arg(evm.get_word_ptr_type())
                .arg(evm.get_word_ptr_type())
                .arg(evm.get_word_ptr_type())
                .arg(evm.get_word_ptr_type())
                .arg(evm.get_byte_ptr_type())
                .arg(evm.get_size_type())
                .arg(evm.get_byte_ptr_type())
                .arg(evm.get_size_type())
                .build()
                .unwrap(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! smoke_get_method {
        ($testname:ident, $method:ident) => {
            #[test]
            fn $testname() {
                let ctx = Context::create();
                let types = CallbackTypes::get_instance(&ctx);

                let _result = types.$method();
            }
        }
    }

    smoke_get_method!(sload_signature, get_type_sload);
    smoke_get_method!(sstore_signature, get_type_sstore);
    smoke_get_method!(balance_signature, get_type_balance);
    smoke_get_method!(calldataload_signature, get_type_calldataload);
    smoke_get_method!(blockhash_signature, get_type_blockhash);
    smoke_get_method!(sha3_signature, get_type_sha3);
    smoke_get_method!(extcodesize_signature, get_type_extcodesize);
    smoke_get_method!(extcodecopy_signature, get_type_extcodecopy);
    smoke_get_method!(log_signature, get_type_log);
    smoke_get_method!(selfdestruct_signature, get_type_selfdestruct);
    smoke_get_method!(call_signature, get_type_call);
}
