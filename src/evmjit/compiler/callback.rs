use std::cell::RefCell;

use evmjit::compiler::evmtypes::EvmTypes;
use evmjit::compiler::external_declarations::DeclarationManager;
use evmjit::compiler::runtime::env::EnvDataType;
use evmjit::compiler::util::funcbuilder::*;

use super::JITContext;

use inkwell::context::Context;
use inkwell::module::Linkage::*;
use inkwell::types::FunctionType;
use inkwell::values::FunctionValue;

/// Manager of all declarations for functions which provide EVM functionality.
pub struct CallbackDeclarationManager<'a> {
    m_context: &'a JITContext,
    m_storageload: RefCell<Option<FunctionValue>>,
    m_storagestore: RefCell<Option<FunctionValue>>,
    m_balance: RefCell<Option<FunctionValue>>,
    m_create: RefCell<Option<FunctionValue>>,
    m_blockhash: RefCell<Option<FunctionValue>>,
    m_extcodesize: RefCell<Option<FunctionValue>>,
    m_extcodecopy: RefCell<Option<FunctionValue>>,
    m_log: RefCell<Option<FunctionValue>>,
    m_selfdestruct: RefCell<Option<FunctionValue>>,
    m_call: RefCell<Option<FunctionValue>>,
}

/// CallbackTypes provides function signatures for each callback function provided to the JIT.
#[derive(Debug)]
pub struct CallbackTypes {
    m_storageload: FunctionType,
    m_storagestore: FunctionType,

    m_balance: FunctionType,
    m_create: FunctionType,
    m_blockhash: FunctionType,

    m_extcodesize: FunctionType,
    m_extcodecopy: FunctionType,

    m_log: FunctionType,

    m_selfdestruct: FunctionType,

    m_call: FunctionType,
}

impl<'a> DeclarationManager<'a> for CallbackDeclarationManager<'a> {
    fn new(context: &'a JITContext) -> Self {
        CallbackDeclarationManager {
            m_context: context,
            m_storageload: RefCell::new(None),
            m_storagestore: RefCell::new(None),
            m_balance: RefCell::new(None),
            m_create: RefCell::new(None),
            m_blockhash: RefCell::new(None),
            m_extcodesize: RefCell::new(None),
            m_extcodecopy: RefCell::new(None),
            m_log: RefCell::new(None),
            m_selfdestruct: RefCell::new(None),
            m_call: RefCell::new(None),
        }
    }

    fn get_decl(&self, name: &str) -> FunctionValue {
        // TODO: reduce code duplication here
        // TODO: verify runtime borrowing here
        match name {
            // If we have already declared the function, return the stored reference. Otherwise,
            // initialize, store the reference, and return it.
            "evm.storageload" => {
                if let Some(decl) = *self.m_storageload.borrow() {
                    decl.clone()
                } else {
                    // Explicitly clone here so that we can move decl inside the optional, and return its
                    // copy to the user.
                    let decl = self.init_storageload();
                    let ret = decl.clone();

                    *self.m_storageload.borrow_mut() = Some(decl);
                    ret
                }
            }
            "evm.storagestore" => {
                if let Some(decl) = *self.m_storagestore.borrow() {
                    decl.clone()
                } else {
                    // Explicitly clone here so that we can move decl inside the optional, and return its
                    // copy to the user.
                    let decl = self.init_storagestore();
                    let ret = decl.clone();

                    *self.m_storagestore.borrow_mut() = Some(decl);
                    ret
                }
            }
            "evm.balance" => {
                if let Some(decl) = *self.m_balance.borrow() {
                    decl.clone()
                } else {
                    // Explicitly clone here so that we can move decl inside the optional, and return its
                    // copy to the user.
                    let decl = self.init_balance();
                    let ret = decl.clone();

                    *self.m_balance.borrow_mut() = Some(decl);
                    ret
                }
            }
            "evm.create" => {
                if let Some(decl) = *self.m_create.borrow() {
                    decl.clone()
                } else {
                    // Explicitly clone here so that we can move decl inside the optional, and return its
                    // copy to the user.
                    let decl = self.init_create();
                    let ret = decl.clone();

                    *self.m_create.borrow_mut() = Some(decl);
                    ret
                }
            }
            "evm.blockhash" => {
                if let Some(decl) = *self.m_blockhash.borrow() {
                    decl.clone()
                } else {
                    // Explicitly clone here so that we can move decl inside the optional, and return its
                    // copy to the user.
                    let decl = self.init_blockhash();
                    let ret = decl.clone();

                    *self.m_blockhash.borrow_mut() = Some(decl);
                    ret
                }
            }
            "evm.extcodesize" => {
                if let Some(decl) = *self.m_extcodesize.borrow() {
                    decl.clone()
                } else {
                    // Explicitly clone here so that we can move decl inside the optional, and return its
                    // copy to the user.
                    let decl = self.init_extcodesize();
                    let ret = decl.clone();

                    *self.m_extcodesize.borrow_mut() = Some(decl);
                    ret
                }
            }
            "evm.extcodecopy" => {
                if let Some(decl) = *self.m_extcodecopy.borrow() {
                    decl.clone()
                } else {
                    // Explicitly clone here so that we can move decl inside the optional, and return its
                    // copy to the user.
                    let decl = self.init_extcodecopy();
                    let ret = decl.clone();

                    *self.m_extcodecopy.borrow_mut() = Some(decl);
                    ret
                }
            }
            "evm.log" => {
                if let Some(decl) = *self.m_log.borrow() {
                    decl.clone()
                } else {
                    // Explicitly clone here so that we can move decl inside the optional, and return its
                    // copy to the user.
                    let decl = self.init_log();
                    let ret = decl.clone();

                    *self.m_log.borrow_mut() = Some(decl);
                    ret
                }
            }
            "evm.selfdestruct" => {
                if let Some(decl) = *self.m_selfdestruct.borrow() {
                    decl.clone()
                } else {
                    // Explicitly clone here so that we can move decl inside the optional, and return its
                    // copy to the user.
                    let decl = self.init_selfdestruct();
                    let ret = decl.clone();

                    *self.m_selfdestruct.borrow_mut() = Some(decl);
                    ret
                }
            }
            "evm.call" => {
                if let Some(decl) = *self.m_call.borrow() {
                    decl.clone()
                } else {
                    // Explicitly clone here so that we can move decl inside the optional, and return its
                    // copy to the user.
                    let decl = self.init_call();
                    let ret = decl.clone();

                    *self.m_call.borrow_mut() = Some(decl);
                    ret
                }
            }
            _ => panic!(format!(
                "Callback declaration manager was requested an invalid import: {}",
                name
            )),
        }
    }
}

// TODO: Reduce code duplication
// TODO: Add attributes to init methods after interface is decided.
impl<'a> CallbackDeclarationManager<'a> {
    fn init_storageload(&self) -> FunctionValue {
        let attrs = self.m_context.attributes();
        let module = self.m_context.module();
        let sig = self.m_context.callback_types().get_type_sload();
        let decl = module.add_function("evm.storageload", sig, Some(External));
        // TODO: Needs readonly attr support
        decl.add_attribute(2, *attrs.attr_noalias());
        decl.add_attribute(2, *attrs.attr_nocapture());
        decl.add_attribute(3, *attrs.attr_noalias());
        decl.add_attribute(3, *attrs.attr_nocapture());
        decl.add_attribute(4, *attrs.attr_noalias());
        decl.add_attribute(4, *attrs.attr_nocapture());

        decl
    }

    fn init_storagestore(&self) -> FunctionValue {
        let module = self.m_context.module();
        let sig = self.m_context.callback_types().get_type_sstore();
        let decl = module.add_function("evm.storagestore", sig, Some(External));
        decl
    }

    fn init_balance(&self) -> FunctionValue {
        let module = self.m_context.module();
        let sig = self.m_context.callback_types().get_type_balance();
        let decl = module.add_function("evm.balance", sig, Some(External));
        decl
    }

    fn init_create(&self) -> FunctionValue {
        let module = self.m_context.module();
        let sig = self.m_context.callback_types().get_type_create();
        let decl = module.add_function("evm.create", sig, Some(External));
        decl
    }

    fn init_blockhash(&self) -> FunctionValue {
        let module = self.m_context.module();
        let sig = self.m_context.callback_types().get_type_blockhash();
        let decl = module.add_function("evm.blockhash", sig, Some(External));
        decl
    }

    fn init_extcodesize(&self) -> FunctionValue {
        let module = self.m_context.module();
        let sig = self.m_context.callback_types().get_type_extcodesize();
        let decl = module.add_function("evm.extcodesize", sig, Some(External));
        decl
    }

    fn init_extcodecopy(&self) -> FunctionValue {
        let module = self.m_context.module();
        let sig = self.m_context.callback_types().get_type_extcodecopy();
        let decl = module.add_function("evm.extcodecopy", sig, Some(External));
        decl
    }

    fn init_log(&self) -> FunctionValue {
        let module = self.m_context.module();
        let sig = self.m_context.callback_types().get_type_log();
        let decl = module.add_function("evm.log", sig, Some(External));
        decl
    }

    fn init_selfdestruct(&self) -> FunctionValue {
        let module = self.m_context.module();
        let sig = self.m_context.callback_types().get_type_selfdestruct();
        let decl = module.add_function("evm.selfdestruct", sig, Some(External));
        decl
    }

    fn init_call(&self) -> FunctionValue {
        let module = self.m_context.module();
        let sig = self.m_context.callback_types().get_type_call();
        let decl = module.add_function("evm.call", sig, Some(External));
        decl
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
    get_type_impl!(get_type_create, m_create);
    get_type_impl!(get_type_blockhash, m_blockhash);
    get_type_impl!(get_type_extcodesize, m_extcodesize);
    get_type_impl!(get_type_extcodecopy, m_extcodecopy);
    get_type_impl!(get_type_log, m_log);
    get_type_impl!(get_type_selfdestruct, m_selfdestruct);
    get_type_impl!(get_type_call, m_call);
}

impl CallbackTypes {
    pub fn new(context: &Context, evm: &EvmTypes, env: &EnvDataType) -> Self {
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
                .arg(evm.get_address_ptr_type())
                .arg(evm.get_word_ptr_type())
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
    use super::super::JITContext;
    use super::*;

    macro_rules! smoke_get_method {
        ($testname:ident, $method:ident) => {
            #[test]
            fn $testname() {
                let ctx = JITContext::new();
                let types = ctx.callback_types();

                let _result = types.$method();
            }
        };
    }

    smoke_get_method!(sload_signature, get_type_sload);
    smoke_get_method!(sstore_signature, get_type_sstore);
    smoke_get_method!(balance_signature, get_type_balance);
    smoke_get_method!(blockhash_signature, get_type_blockhash);
    smoke_get_method!(extcodesize_signature, get_type_extcodesize);
    smoke_get_method!(extcodecopy_signature, get_type_extcodecopy);
    smoke_get_method!(log_signature, get_type_log);
    smoke_get_method!(selfdestruct_signature, get_type_selfdestruct);
    smoke_get_method!(call_signature, get_type_call);
}
