#![allow(non_snake_case)]
#![allow(unused)]

#[macro_use]
extern crate jsontests_derive;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/VMTestsHarnessCorrectnessTests"]
#[test_with = "jsontests::vmtests::run_test"]
#[should_panic]
struct VMTestsHarnessCorrectness;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/BlockchainTestsHarnessCorrectnessTests"]
#[test_with = "jsontests::vmtests::run_test"]
#[should_panic]
struct BlockchainTestsHarnessCorrectionTests;
