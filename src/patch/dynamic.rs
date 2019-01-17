#[cfg(feature = "std")] use std::cell::Cell;
#[cfg(not(feature = "std"))] use core::cell::Cell;
#[cfg(feature = "std")] use std::marker::PhantomData;
#[cfg(not(feature = "std"))] use core::marker::PhantomData;
use bigint::{Address, Gas, U256};
use patch::{AccountPatch, Patch, Precompiled};

#[derive(Clone)]
pub struct BlockRangePatch<A> {
    /// Lower block number range bound
    pub block_num_lower: Option<U256>,
    /// Upper block number range bound
    pub block_num_upper: Option<U256>,
    /// Maximum contract size.
    pub code_deposit_limit: Option<usize>,
    /// Limit of the call stack.
    pub callstack_limit: usize,
    /// Gas paid for extcode.
    pub gas_extcode: Gas,
    /// Gas paid for BALANCE opcode.
    pub gas_balance: Gas,
    /// Gas paid for SLOAD opcode.
    pub gas_sload: Gas,
    /// Gas paid for SUICIDE opcode.
    pub gas_suicide: Gas,
    /// Gas paid for SUICIDE opcode when it hits a new account.
    pub gas_suicide_new_account: Gas,
    /// Gas paid for CALL opcode.
    pub gas_call: Gas,
    /// Gas paid for EXP opcode for every byte.
    pub gas_expbyte: Gas,
    /// Gas paid for a contract creation transaction.
    pub gas_transaction_create: Gas,
    /// Whether to force code deposit even if it does not have enough
    /// gas.
    pub force_code_deposit: bool,
    /// Whether the EVM has DELEGATECALL opcode.
    pub has_delegate_call: bool,
    /// Whether the EVM has STATICCALL opcode.
    pub has_static_call: bool,
    /// Whether the EVM has REVERT opcode.
    pub has_revert: bool,
    /// Whether the EVM has RETURNDATASIZE and RETURNDATACOPY opcode.
    pub has_return_data: bool,
    /// Whether the EVM has SHL, SHR and SAR
    pub has_bitwise_shift: bool,
    /// Whether the EVM has EXTCODEHASH
    pub has_extcodehash: bool,
    /// Whether EVM should implement the EIP1283 gas metering scheme for SSTORE opcode
    pub has_reduced_sstore_gas_metering: bool,
    /// Whether to throw out of gas error when
    /// CALL/CALLCODE/DELEGATECALL requires more than maximum amount
    /// of gas.
    pub err_on_call_with_more_gas: bool,
    /// If true, only consume at maximum l64(after_gas) when
    /// CALL/CALLCODE/DELEGATECALL.
    pub call_create_l64_after_gas: bool,
    /// Maximum size of the memory, in bytes.
    /// NOTE: **NOT** runtime-configurable by block number
    pub memory_limit: usize,
    /// Precompiled contracts at given address, with required code,
    /// and its definition.
    pub precompileds: Vec<(Address, Option<&'static [u8]>, &'static dyn Precompiled)>,
    _marker: PhantomData<A>
}

impl<A: AccountPatch> Patch for BlockRangePatch<A> {
    type Account = A;
    fn code_deposit_limit(&self) -> Option<usize> { self.code_deposit_limit }
    fn callstack_limit(&self) -> usize { self.callstack_limit }
    fn gas_extcode(&self) -> Gas { self.gas_extcode }
    fn gas_balance(&self) -> Gas { self.gas_balance }
    fn gas_sload(&self) -> Gas { self.gas_sload }
    fn gas_suicide(&self) -> Gas { self.gas_suicide }
    fn gas_suicide_new_account(&self) -> Gas { self.gas_suicide_new_account }
    fn gas_call(&self) -> Gas { self.gas_call }
    fn gas_expbyte(&self) -> Gas { self.gas_expbyte }
    fn gas_transaction_create(&self) -> Gas { self.gas_transaction_create }
    fn force_code_deposit(&self) -> bool { self.force_code_deposit }
    fn has_delegate_call(&self) -> bool { self.has_delegate_call }
    fn has_static_call(&self) -> bool { self.has_static_call }
    fn has_revert(&self) -> bool { self.has_revert }
    fn has_return_data(&self) -> bool { self.has_return_data }
    fn has_bitwise_shift(&self) -> bool { self.has_bitwise_shift }
    fn has_extcodehash(&self) -> bool { self.has_extcodehash }
    fn has_reduced_sstore_gas_metering(&self) -> bool { self.has_reduced_sstore_gas_metering }
    fn err_on_call_with_more_gas(&self) -> bool { self.err_on_call_with_more_gas }
    fn call_create_l64_after_gas(&self) -> bool { self.call_create_l64_after_gas }
    fn memory_limit(&self) -> usize { self.memory_limit }
    fn precompileds(&self) -> &[(Address, Option<&'static [u8]>, &'static dyn Precompiled)] {
        &self.precompileds
    }
}

/// Block-number range configurable Patch
// TODO: benchmark performance
pub struct DynamicPatch<A> {
    patches: Vec<BlockRangePatch<A>>,
    block_number: Cell<U256>,
    current_patch_idx: Cell<usize>,
}

impl<A> DynamicPatch<A> {
    fn current_patch(&self) -> &BlockRangePatch<A> {
        // Start with stored idx as an attempt to perform lookups faster
        let idx = self.current_patch_idx.get();

        // Make a wrapped cycle through patches idx -> idx-1
        let patches_cnt = self.patches.len();
        let block_num = self.block_number.get();
        let mut found = false;
        for (idx, patch) in self.patches.iter()
            .enumerate()
            .skip(idx)
            .cycle()
            .take(patches_cnt)
        {
            found = patch.block_num_lower.map(|n| n <= block_num).unwrap_or(true) &&
                patch.block_num_upper.map(|n| block_num < n).unwrap_or(true);

            if found {
                self.current_patch_idx.set(idx);
                break;
            }
        }

        if !found {
            panic!("dynamic patch configuration is incorrect: couldn't find appropriate patch for block number {}", block_num);
        }

        &self.patches[self.current_patch_idx.get()]
    }
}

macro_rules! delegate_patch_methods {
    { $($method:tt -> $ret:ty;)* } => {
        $(
            fn $method(&self) -> $ret {
                self.current_patch().$method()
            }
        )*
    };
}

impl<A: AccountPatch> Patch for DynamicPatch<A> {
    type Account = A;

    fn set_block_number(&self, block_number: U256) {
        self.block_number.set(block_number)
    }

    fn precompileds(&self) -> &[(Address, Option<&[u8]>, &dyn Precompiled)] {
        self.current_patch().precompileds()
    }

    delegate_patch_methods! {
        code_deposit_limit -> Option<usize>;
        callstack_limit -> usize;
        gas_extcode -> Gas;
        gas_balance -> Gas;
        gas_sload -> Gas;
        gas_suicide -> Gas;
        gas_suicide_new_account -> Gas;
        gas_call -> Gas;
        gas_expbyte -> Gas;
        gas_transaction_create -> Gas;
        force_code_deposit -> bool;
        has_delegate_call -> bool;
        has_static_call -> bool;
        has_revert -> bool;
        has_return_data -> bool;
        has_bitwise_shift -> bool;
        has_extcodehash -> bool;
        has_reduced_sstore_gas_metering -> bool;
        err_on_call_with_more_gas -> bool;
        call_create_l64_after_gas -> bool;
        memory_limit -> usize;
    }

}
