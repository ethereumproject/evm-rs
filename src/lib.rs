//! EthereumVM implementation, traits and structs.
//!
//! EthereumVM works on two different levels. It handles:
//! 1. a transaction, or
//! 2. an Ethereum execution context.
//!
//! To interact with the virtual machine, you usually only need to
//! work with [VM](trait.VM.html) methods.
//!
//! ### A EthereumVM's Lifecycle
//!
//! A VM can be started after it is given a `Transaction` (or
//! `Context`) and a `BlockHeader`. The user can then `fire` or `step`
//! to run it.  [`fire`](trait.VM.html#method.fire) runs the EVM code
//! (given in field `code` of the transaction) until it finishes or
//! cannot continue. However [`step`](trait.VM.html#tymethod.step)
//! only runs at most one instruction. If the virtual machine needs
//! some information (accounts in the current block, or block hashes
//! of previous blocks) it fails, returning a
//! [`RequireError`](errors/enum.RequireError.html) enumeration. With
//! the data returned in the `RequireError` enumeration, one can use
//! the methods
//! [`commit_account`](trait.VM.html#tymethod.commit_account) and
//! [`commit_blockhash`](trait.VM.html#tymethod.commit_blockhash) to
//! commit the information to the VM. `fire` or `step` can be
//! subsequently called to restart from that point. The current VM
//! status can always be obtained using the `status` function. Again,
//! see [VM](trait.VM.html) for a list of methods that can be applied.
//!
//! ### Patch: Specifying a Network and Hard-fork
//!
//! Every VM is associated with a `Patch`. This patch tells the VM
//! which Ethereum network and which hard fork it is on. You will need
//! to specify the patch as the type parameter. To interact with
//! multiple patches at the same time, it is recommended that you use
//! trait objects.
//!
//! The example below creates a new EthereumVM and stores the object in
//! `vm` which can be used to `fire`, `step` or get status on. To do
//! this, it must first create a transaction and a block header.  The
//! patch associated with the VM is either `EmbeddedPatch` or
//! `VMTestPatch` depending on an arbitrary block number value set at
//! the beginning of the program.
//!
//! ```
//! use ethereumvm::{EmbeddedPatch, VMTestPatch,
//!                 HeaderParams, ValidTransaction, TransactionAction,
//!                 VM, SeqTransactionVM};
//! use bigint::{Gas, U256, Address};
//! use std::rc::Rc;
//!
//! fn main() {
//!   let block_number = 1000;
//!   let transaction = ValidTransaction {
//!     caller: Some(Address::default()),
//!     gas_price: Gas::zero(),
//!     gas_limit: Gas::max_value(),
//!     action: TransactionAction::Create,
//!     value: U256::zero(),
//!     input: Rc::new(Vec::new()),
//!     nonce: U256::zero()
//!   };
//!   let header = HeaderParams {
//!     beneficiary: Address::default(),
//!     timestamp: 0,
//!     number: U256::zero(),
//!     difficulty: U256::zero(),
//!     gas_limit: Gas::zero()
//!   };
//!   let cfg_before_500 = VMTestPatch::default();
//!   let cfg_after_500 = EmbeddedPatch::default();
//!   let vm = if block_number < 500 {
//!     SeqTransactionVM::new(
//!       &cfg_before_500,
//!       transaction,
//!       header
//!     );
//!   } else {
//!     SeqTransactionVM::new(
//!       &cfg_after_500,
//!       transaction,
//!       header
//!     );
//!   };
//! }
//! ```
//!
//! ### Transaction Execution
//!
//! To start a VM on the Transaction level, use the `TransactionVM`
//! struct. Usually, you want to use the sequential memory module
//! which can be done using the type definition
//! `SeqTransactionVM`.
//!
//! Calling `TransactionVM::new` or `SeqTransactionVM::new` requires
//! the transaction passed in to be valid (according to the rules for
//! an Ethereum transaction). If the transaction is invalid, the VM
//! will probably panic. If you want to handle untrusted transactions,
//! you should use `SeqTransactionVM::new_untrusted`, which will not
//! panic but instead return an error if the transaction is invalid.
//!
//! ### Context Execution
//!
//! To start a VM on the Context level, use the `ContextVM`
//! struct. Usually, you use the sequential memory module with the
//! type definition `SeqContextVM`. Context execution, as with other
//! EVM implementations, will not handle transaction-level gas
//! reductions.

#![deny(
    unused_import_braces,
    unused_imports,
    unused_comparisons,
    unused_must_use,
    unused_variables,
    non_shorthand_field_patterns,
    unreachable_code,
    missing_docs
)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), feature(alloc))]

// extern crates below are left in code on purpose even though it's discouraged in Edition 2018
// in the event of incorrect feature setting, the error will pop up on
// extern crate statement in lib.rs, which is easier to debug
#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(feature = "c-secp256k1")]
extern crate secp256k1;

#[cfg(feature = "rust-secp256k1")]
extern crate secp256k1;

#[cfg(feature = "std")]
extern crate block;

// BUG: without old-style #[macro_use] extern crate, evm-rs cannot be compiled as a dependency.
#[macro_use]
extern crate log;

mod commit;
pub mod errors;
mod eval;
mod memory;
mod params;
mod patch;
mod pc;
mod stack;
mod transaction;
mod util;

pub use crate::commit::{AccountChange, AccountCommitment, AccountState, BlockhashState, Storage};
pub use crate::errors::{CommitError, NotSupportedError, OnChainError, PreExecutionError, RequireError};
pub use crate::eval::{Machine, MachineStatus, Runtime, State};
pub use crate::memory::{Memory, SeqMemory};
pub use crate::params::*;
pub use crate::patch::*;
pub use crate::pc::{Instruction, PCMut, Valids, PC};
pub use crate::stack::Stack;
pub use crate::transaction::{TransactionVM, UntrustedTransaction, ValidTransaction};
pub use crate::util::opcode::Opcode;
pub use block_core::TransactionAction;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

#[cfg(not(feature = "std"))]
use alloc::boxed::Box;
#[cfg(not(feature = "std"))]
use alloc::{collections::btree_map as map, collections::BTreeSet as Set};
use bigint::{Address, Gas, H256, U256};
#[cfg(not(feature = "std"))]
use core::cmp::min;
#[cfg(feature = "std")]
use std::cmp::min;
#[cfg(feature = "std")]
use std::collections::{hash_map as map, HashSet as Set};

#[derive(Debug, Clone, PartialEq)]
/// VM Status
pub enum VMStatus {
    /// A running VM.
    Running,
    /// VM is stopped without errors.
    ExitedOk,
    /// VM is stopped due to an error. The state of the VM is before
    /// the last failing instruction.
    ExitedErr(OnChainError),
    /// VM is stopped because it does not support certain
    /// operations. The client is expected to either drop the
    /// transaction or panic. This rarely happens unless the executor
    /// agrees upon on a really large number of gas limit, so it
    /// usually can be safely ignored.
    ExitedNotSupported(NotSupportedError),
}

/// Represents an EVM. This is usually the main interface for clients
/// to interact with.
pub trait VM {
    /// Commit an account information to this VM. This should only
    /// be used when receiving `RequireError`.
    fn commit_account(&mut self, commitment: AccountCommitment) -> Result<(), CommitError>;
    /// Commit a block hash to this VM. This should only be used when
    /// receiving `RequireError`.
    fn commit_blockhash(&mut self, number: U256, hash: H256) -> Result<(), CommitError>;
    /// Returns the current status of the VM.
    fn status(&self) -> VMStatus;
    /// Read the next instruction to be executed.
    fn peek(&self) -> Option<Instruction>;
    /// Read the next opcode to be executed.
    fn peek_opcode(&self) -> Option<Opcode>;
    /// Run one instruction and return. If it succeeds, VM status can
    /// still be `Running`. If the call stack has more than one items,
    /// this will only executes the last items' one single
    /// instruction.
    fn step(&mut self) -> Result<(), RequireError>;
    /// Run instructions until it reaches a `RequireError` or
    /// exits. If this function succeeds, the VM status can only be
    /// either `ExitedOk` or `ExitedErr`.
    fn fire(&mut self) -> Result<(), RequireError> {
        loop {
            match self.status() {
                VMStatus::Running => self.step()?,
                VMStatus::ExitedOk | VMStatus::ExitedErr(_) | VMStatus::ExitedNotSupported(_) => return Ok(()),
            }
        }
    }
    /// Returns the changed or committed accounts information up to
    /// current execution status.
    fn accounts(&self) -> map::Values<Address, AccountChange>;
    /// Returns all fetched or modified addresses.
    fn used_addresses(&self) -> Set<Address>;
    /// Returns the out value, if any.
    fn out(&self) -> &[u8];
    /// Returns the available gas of this VM.
    fn available_gas(&self) -> Gas;
    /// Returns the refunded gas of this VM.
    fn refunded_gas(&self) -> Gas;
    /// Returns logs to be appended to the current block if the user
    /// decided to accept the running status of this VM.
    fn logs(&self) -> &[Log];
    /// Returns all removed account addresses as for current VM execution.
    fn removed(&self) -> &[Address];
    /// Returns the real used gas by the transaction or the VM
    /// context. Only available when the status of the VM is
    /// exited. Otherwise returns zero.
    fn used_gas(&self) -> Gas;
}

/// A sequential VM. It uses sequential memory representation and hash
/// map storage for accounts.
pub type SeqContextVM<'a, P> = ContextVM<'a, SeqMemory, P>;
/// A sequential transaction VM. This is same as `SeqContextVM` except
/// it runs at transaction level.
pub type SeqTransactionVM<'a, P> = TransactionVM<'a, SeqMemory, P>;

/// A VM that executes using a context and block information.
pub struct ContextVM<'a, M, P: Patch> {
    runtime: Runtime,
    machines: Vec<Machine<'a, M, P>>,
    fresh_account_state: AccountState<'a, P::Account>,
}

impl<'a, M: Memory, P: Patch> ContextVM<'a, M, P> {
    /// Create a new VM using the given context, block header and patch.
    pub fn new(patch: &'a P, context: Context, block: HeaderParams) -> Self {
        let mut machines = Vec::new();
        let account_patch = patch.account_patch();
        machines.push(Machine::new(patch, context, 1));
        ContextVM {
            machines,
            runtime: Runtime::new(block),
            fresh_account_state: AccountState::new(account_patch),
        }
    }

    /// Create a new VM with the given account state and blockhash state.
    pub fn with_states(
        patch: &'a P,
        context: Context,
        block: HeaderParams,
        account_state: AccountState<'a, P::Account>,
        blockhash_state: BlockhashState,
    ) -> Self {
        let mut machines = Vec::new();
        machines.push(Machine::with_states(patch, context, 1, account_state.clone()));
        ContextVM {
            machines,
            runtime: Runtime::with_states(block, blockhash_state),
            fresh_account_state: account_state,
        }
    }

    /// Create a new VM with customized initialization code.
    pub fn with_init<F: FnOnce(&mut ContextVM<M, P>)>(
        patch: &'a P,
        context: Context,
        block: HeaderParams,
        account_state: AccountState<'a, P::Account>,
        blockhash_state: BlockhashState,
        f: F,
    ) -> Self {
        let mut vm = Self::with_states(patch, context, block, account_state, blockhash_state);
        f(&mut vm);
        vm.fresh_account_state =
            AccountState::derive_from(patch.account_patch(), &vm.machines[0].state().account_state);
        vm
    }

    /// Create a new VM with the result of the previous VM. This is
    /// usually used by transaction for chainning them.
    pub fn with_previous(patch: &'a P, context: Context, block: HeaderParams, vm: &'a ContextVM<'a, M, P>) -> Self {
        Self::with_states(
            patch,
            context,
            block,
            vm.machines[0].state().account_state.clone(),
            vm.runtime.blockhash_state.clone(),
        )
    }

    /// Returns the current state of the VM.
    pub fn current_state(&self) -> &State<M, P> {
        self.current_machine().state()
    }

    /// Returns the current runtime machine.
    pub fn current_machine(&self) -> &Machine<M, P> {
        self.machines.last().unwrap()
    }

    /// Add a new context history hook.
    pub fn add_context_history_hook<F: 'static + Fn(&Context)>(&mut self, f: F) {
        self.runtime.context_history_hooks.push(Box::new(f));
        debug!("registered a new history hook");
    }
}

impl<'a, M: Memory, P: Patch> VM for ContextVM<'a, M, P> {
    fn commit_account(&mut self, commitment: AccountCommitment) -> Result<(), CommitError> {
        for machine in &mut self.machines {
            machine.commit_account(commitment.clone())?;
        }
        debug!("committed account info: {:?}", commitment);
        Ok(())
    }

    fn commit_blockhash(&mut self, number: U256, hash: H256) -> Result<(), CommitError> {
        self.runtime.blockhash_state.commit(number, hash)?;
        debug!("committed blockhash number {}: {}", number, hash);
        Ok(())
    }

    #[allow(clippy::single_match)]
    fn status(&self) -> VMStatus {
        match self.machines.last().unwrap().status().clone() {
            MachineStatus::ExitedNotSupported(err) => return VMStatus::ExitedNotSupported(err),
            _ => (),
        }

        match self.machines[0].status() {
            MachineStatus::Running | MachineStatus::InvokeCreate(_) | MachineStatus::InvokeCall(_, _) => {
                VMStatus::Running
            }
            MachineStatus::ExitedOk => VMStatus::ExitedOk,
            MachineStatus::ExitedErr(err) => VMStatus::ExitedErr(err),
            MachineStatus::ExitedNotSupported(err) => VMStatus::ExitedNotSupported(err),
        }
    }

    fn peek(&self) -> Option<Instruction> {
        match self.machines.last().unwrap().status().clone() {
            MachineStatus::Running => self.machines.last().unwrap().peek(),
            _ => None,
        }
    }

    fn peek_opcode(&self) -> Option<Opcode> {
        match self.machines.last().unwrap().status().clone() {
            MachineStatus::Running => self.machines.last().unwrap().peek_opcode(),
            _ => None,
        }
    }

    fn step(&mut self) -> Result<(), RequireError> {
        match self.machines.last().unwrap().status().clone() {
            MachineStatus::Running => {
                self.machines.last_mut().unwrap().step(&self.runtime)?;
                if self.machines.len() == 1 {
                    match self.machines.last().unwrap().status().clone() {
                        MachineStatus::ExitedOk | MachineStatus::ExitedErr(_) => self
                            .machines
                            .last_mut()
                            .unwrap()
                            .finalize_context(&self.fresh_account_state),
                        _ => (),
                    }
                }
                Ok(())
            }
            MachineStatus::ExitedOk | MachineStatus::ExitedErr(_) => {
                if self.machines.is_empty() {
                    panic!()
                } else if self.machines.len() == 1 {
                    Ok(())
                } else {
                    let finished = self.machines.pop().unwrap();
                    self.machines.last_mut().unwrap().apply_sub(finished);
                    Ok(())
                }
            }
            MachineStatus::ExitedNotSupported(_) => Ok(()),
            MachineStatus::InvokeCall(context, _) => {
                for hook in &self.runtime.context_history_hooks {
                    hook(&context)
                }

                let mut sub = self.machines.last().unwrap().derive(context);
                sub.invoke_call()?;
                self.machines.push(sub);
                Ok(())
            }
            MachineStatus::InvokeCreate(context) => {
                for hook in &self.runtime.context_history_hooks {
                    hook(&context)
                }

                let mut sub = self.machines.last().unwrap().derive(context);
                sub.invoke_create()?;
                self.machines.push(sub);
                Ok(())
            }
        }
    }

    fn fire(&mut self) -> Result<(), RequireError> {
        loop {
            debug!("machines status:");
            for (n, machine) in self.machines.iter().enumerate() {
                debug!("Machine {}: {:x?}", n, machine.status());
            }
            match self.status() {
                VMStatus::Running => self.step()?,
                VMStatus::ExitedOk | VMStatus::ExitedErr(_) | VMStatus::ExitedNotSupported(_) => return Ok(()),
            }
        }
    }

    fn accounts(&self) -> map::Values<Address, AccountChange> {
        self.machines[0].state().account_state.accounts()
    }

    fn used_addresses(&self) -> Set<Address> {
        self.machines[0].state().account_state.used_addresses()
    }

    fn out(&self) -> &[u8] {
        self.machines[0].state().out.as_slice()
    }

    fn available_gas(&self) -> Gas {
        self.machines[0].state().available_gas()
    }

    fn refunded_gas(&self) -> Gas {
        self.machines[0].state().refunded_gas
    }

    fn logs(&self) -> &[Log] {
        self.machines[0].state().logs.as_slice()
    }

    fn removed(&self) -> &[Address] {
        self.machines[0].state().removed.as_slice()
    }

    fn used_gas(&self) -> Gas {
        let total_used = self.machines[0].state().total_used_gas();
        let refund_cap = total_used / Gas::from(2u64);
        let refunded = min(refund_cap, self.machines[0].state().refunded_gas);
        total_used - refunded
    }
}
