#![cfg_attr(feature = "bench", feature(test))]
#![allow(non_snake_case)]
#![allow(unused)]

#[macro_use]
extern crate jsontests_derive;

#[cfg(feature = "bench")]
extern crate test;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/BlockchainTests/GeneralStateTests"]
#[test_with = "jsontests::vmtests::run_test"]
#[cfg_attr(feature = "bench", bench_with = "jsontests::vmtests::run_bench")]
struct GeneralStateTest;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/BlockchainTests/TransitionTests"]
#[test_with = "jsontests::vmtests::run_test"]
#[cfg_attr(feature = "bench", bench_with = "jsontests::vmtests::run_bench")]
struct TransitionTest;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/BlockchainTests/bcBlockGasLimitTest"]
#[test_with = "jsontests::vmtests::run_test"]
#[cfg_attr(feature = "bench", bench_with = "jsontests::vmtests::run_bench")]
struct BlockGasLimitTest;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/BlockchainTests/bcExploitTest"]
#[test_with = "jsontests::vmtests::run_test"]
#[cfg_attr(feature = "bench", bench_with = "jsontests::vmtests::run_bench")]
struct ExploitTest;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/BlockchainTests/bcForgedTest"]
#[test_with = "jsontests::vmtests::run_test"]
#[cfg_attr(feature = "bench", bench_with = "jsontests::vmtests::run_bench")]
struct ForgedTest;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/BlockchainTests/bcForkStressTest"]
#[test_with = "jsontests::vmtests::run_test"]
#[cfg_attr(feature = "bench", bench_with = "jsontests::vmtests::run_bench")]
struct ForkStressTest;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/BlockchainTests/bcGasPricerTest"]
#[test_with = "jsontests::vmtests::run_test"]
#[cfg_attr(feature = "bench", bench_with = "jsontests::vmtests::run_bench")]
struct GasPricerTest;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/BlockchainTests/bcInvalidHeaderTest"]
#[test_with = "jsontests::vmtests::run_test"]
#[cfg_attr(feature = "bench", bench_with = "jsontests::vmtests::run_bench")]
struct InvalidHeaderTest;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/BlockchainTests/bcMultiChainTest"]
#[test_with = "jsontests::vmtests::run_test"]
#[cfg_attr(feature = "bench", bench_with = "jsontests::vmtests::run_bench")]
struct MultiChainTest;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/BlockchainTests/bcRandomBlockhashTest"]
#[test_with = "jsontests::vmtests::run_test"]
#[cfg_attr(feature = "bench", bench_with = "jsontests::vmtests::run_bench")]
struct RandomBlockhashTest;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/BlockchainTests/bcStateTests"]
#[test_with = "jsontests::vmtests::run_test"]
#[cfg_attr(feature = "bench", bench_with = "jsontests::vmtests::run_bench")]
struct StateTests;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/BlockchainTests/bcTotalDifficultyTest"]
#[test_with = "jsontests::vmtests::run_test"]
#[cfg_attr(feature = "bench", bench_with = "jsontests::vmtests::run_bench")]
struct TotalDifficultyTest;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/BlockchainTests/bcUncleHeaderValidity"]
#[test_with = "jsontests::vmtests::run_test"]
#[cfg_attr(feature = "bench", bench_with = "jsontests::vmtests::run_bench")]
struct UncleHeaderValidity;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/BlockchainTests/bcUncleTest"]
#[test_with = "jsontests::vmtests::run_test"]
#[cfg_attr(feature = "bench", bench_with = "jsontests::vmtests::run_bench")]
struct UncleTest;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/BlockchainTests/bcValidBlockTest"]
#[test_with = "jsontests::vmtests::run_test"]
#[cfg_attr(feature = "bench", bench_with = "jsontests::vmtests::run_bench")]
struct ValidBlockTest;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/BlockchainTests/bcWalletTest"]
#[test_with = "jsontests::vmtests::run_test"]
#[cfg_attr(feature = "bench", bench_with = "jsontests::vmtests::run_bench")]
struct WalletTest;
