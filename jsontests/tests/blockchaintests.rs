#![allow(non_snake_case)]
#![allow(unused)]

#[macro_use]
extern crate jsontests_derive;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/BlockchainTests/GeneralStateTests"]
#[test_with = "jsontests::blockchaintests::run_test"]
#[runtime = "dynamic"]
struct GeneralStateTest;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/BlockchainTests/TransitionTests"]
#[test_with = "jsontests::blockchaintests::run_test"]
struct TransitionTest;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/BlockchainTests/bcBlockGasLimitTest"]
#[test_with = "jsontests::blockchaintests::run_test"]
struct BlockGasLimitTest;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/BlockchainTests/bcExploitTest"]
#[test_with = "jsontests::blockchaintests::run_test"]
struct ExploitTest;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/BlockchainTests/bcForgedTest"]
#[test_with = "jsontests::blockchaintests::run_test"]
struct ForgedTest;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/BlockchainTests/bcForkStressTest"]
#[test_with = "jsontests::blockchaintests::run_test"]
struct ForkStressTest;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/BlockchainTests/bcGasPricerTest"]
#[test_with = "jsontests::blockchaintests::run_test"]
struct GasPricerTest;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/BlockchainTests/bcInvalidHeaderTest"]
#[test_with = "jsontests::blockchaintests::run_test"]
struct InvalidHeaderTest;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/BlockchainTests/bcMultiChainTest"]
#[test_with = "jsontests::blockchaintests::run_test"]
struct MultiChainTest;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/BlockchainTests/bcRandomBlockhashTest"]
#[test_with = "jsontests::blockchaintests::run_test"]
struct RandomBlockhashTest;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/BlockchainTests/bcStateTests"]
#[test_with = "jsontests::blockchaintests::run_test"]
struct StateTests;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/BlockchainTests/bcTotalDifficultyTest"]
#[test_with = "jsontests::blockchaintests::run_test"]
struct TotalDifficultyTest;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/BlockchainTests/bcUncleHeaderValidity"]
#[test_with = "jsontests::blockchaintests::run_test"]
struct UncleHeaderValidity;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/BlockchainTests/bcUncleTest"]
#[test_with = "jsontests::blockchaintests::run_test"]
struct UncleTest;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/BlockchainTests/bcValidBlockTest"]
#[test_with = "jsontests::blockchaintests::run_test"]
struct ValidBlockTest;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/BlockchainTests/bcWalletTest"]
#[test_with = "jsontests::blockchaintests::run_test"]
struct WalletTest;
