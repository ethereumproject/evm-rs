#![allow(non_snake_case)]

#[macro_use]
extern crate jsontests_derive;
extern crate evm;
extern crate jsontests;
#[macro_use]
extern crate criterion;

use criterion::Criterion;
use std::time::Duration;

#[derive(JsonTests)]
#[directory = "jsontests/res/files/eth/VMTests/vmPerformance"]
#[test_with = "jsontests::vmtests::run_test"]
#[bench_with = "jsontests::vmtests::run_bench"]
#[criterion_config = "criterion_cfg"]
struct Performance;

pub fn criterion_cfg() -> Criterion {
    // Due to poor SputnikVM performance, there's no chance to get a lot of measurements
    // and higher threshold is needed
    Criterion::default()
        .sample_size(2)
        .measurement_time(Duration::from_secs(10))
        .noise_threshold(0.07)
}

criterion_main!(Performance_bench_main);
