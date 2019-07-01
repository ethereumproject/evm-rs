extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate lazy_static;
extern crate bigint;
extern crate block;
extern crate ethereumvm;
extern crate ethereumvm_network_classic;
extern crate ethereumvm_stateful;
extern crate rand;
extern crate sha3;
extern crate trie;

use bigint::{Address, Gas, H256, U256};
use block::TransactionAction;
use ethereumvm::{HeaderParams, SeqTransactionVM, VMStatus, ValidTransaction, VM};
use ethereumvm_network_classic::MainnetEIP160Patch;
use ethereumvm_stateful::{LiteralAccount, MemoryStateful};
use rand::Rng;
use sha3::{Digest, Keccak256};
use std::collections::HashMap;
use std::rc::Rc;
use std::str::FromStr;
use trie::{Database, MemoryDatabase};

#[derive(Serialize, Deserialize, Debug)]
struct JSONAccount {
    balance: String,
}

lazy_static! {
    static ref GENESIS_ACCOUNTS: HashMap<String, JSONAccount> =
        serde_json::from_str(include_str!("../res/genesis.json")).unwrap();
}

lazy_static! {
    static ref MORDEN_ACCOUNTS: HashMap<String, JSONAccount> =
        serde_json::from_str(include_str!("../res/morden.json")).unwrap();
}

#[test]
fn secure_trie() {
    let database = MemoryDatabase::new();
    let mut trie = database.create_empty();

    trie.insert_raw(
        Keccak256::digest("doe".as_bytes()).as_slice().into(),
        "reindeer".as_bytes().into(),
    );
    trie.insert_raw(
        Keccak256::digest("dog".as_bytes()).as_slice().into(),
        "puppy".as_bytes().into(),
    );
    trie.insert_raw(
        Keccak256::digest("dogglesworth".as_bytes()).as_slice().into(),
        "cat".as_bytes().into(),
    );

    assert_eq!(
        trie.root(),
        H256::from_str("0xd4cd937e4a4368d7931a9cf51686b7e10abb3dce38a39000fd7902a092b64585").unwrap()
    );
}

#[test]
fn morden_state_root() {
    let database = MemoryDatabase::default();
    let mut stateful = MemoryStateful::empty(&database);
    let mut rng = rand::thread_rng();

    let mut accounts: Vec<(&String, &JSONAccount)> = MORDEN_ACCOUNTS.iter().collect();
    rng.shuffle(&mut accounts);

    for (key, value) in accounts {
        let address = Address::from_str(key).unwrap();
        let balance = U256::from_dec_str(&value.balance).unwrap();

        stateful.sets(&[(
            address,
            LiteralAccount {
                nonce: U256::from(2u64.pow(20)),
                storage: HashMap::new(),
                code: Vec::new(),
                balance,
            },
        )]);
    }

    assert_eq!(
        stateful.root(),
        H256::from("0xf3f4696bbf3b3b07775128eb7a3763279a394e382130f27c21e70233e04946a9")
    );
}

#[test]
fn genesis_state_root() {
    let database = MemoryDatabase::default();
    let mut stateful = MemoryStateful::empty(&database);
    let mut rng = rand::thread_rng();

    let mut accounts: Vec<(&String, &JSONAccount)> = GENESIS_ACCOUNTS.iter().collect();
    rng.shuffle(&mut accounts);
    let empty_input = Rc::new(Vec::new());

    for (key, value) in accounts {
        let address = Address::from_str(key).unwrap();
        let balance = U256::from_dec_str(&value.balance).unwrap();
        let patch = MainnetEIP160Patch::default();
        let vm: SeqTransactionVM<_> = stateful.execute(
            &patch,
            ValidTransaction {
                caller: None,
                gas_price: Gas::zero(),
                gas_limit: Gas::from(100000u64),
                action: TransactionAction::Call(address),
                value: balance,
                input: empty_input.clone(),
                nonce: U256::zero(),
            },
            &HeaderParams {
                beneficiary: Address::default(),
                timestamp: 0,
                number: U256::zero(),
                difficulty: U256::zero(),
                gas_limit: Gas::max_value(),
            },
            &[],
        );
        match vm.status() {
            VMStatus::ExitedOk => (),
            _ => panic!(),
        }
    }

    assert_eq!(
        stateful.root(),
        H256::from("0xd7f8974fb5ac78d9ac099b9ad5018bedc2ce0a72dad1827a1709da30580f0544")
    );
}
