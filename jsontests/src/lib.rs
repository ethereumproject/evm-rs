use evm::Patch;
use serde_json::Value;
use std::fs::File;
use std::path::Path;

pub mod blockchaintests;
pub mod vmtests;

fn load_tests(path: impl AsRef<Path>) -> impl Iterator<Item = (String, serde_json::Value)> {
    let tests = || -> Result<_, failure::Error> {
        let file = File::open(&path)?;
        Ok(serde_json::from_reader(file)?)
    }();

    println!("{}", std::env::current_dir().unwrap().display());

    // Move out the root object
    let tests = match tests {
        Ok(Value::Object(tests)) => tests,
        Ok(_) => panic!("expected a json object at the root of test file"),
        Err(e) => panic!("failed to open test file at {}: {}", path.as_ref().display(), e),
    };

    tests.into_iter()
}

pub fn run_tests_from_file<F>(path: impl AsRef<Path>, test_fn: F)
where
    F: Fn(&str, &str),
{
    let tests = load_tests(path);
    for (name, test) in tests {
        let test_data = serde_json::to_string(&test).unwrap();
        test_fn(&name, &test_data)
    }
}
use criterion::Criterion;

pub fn run_bench_from_file<F>(c: &mut Criterion, path: impl AsRef<Path>, bench_fn: F)
where
    F: Fn(&mut Criterion, &str, &str) + 'static,
{
    let tests = load_tests(path);
    for (name, test) in tests {
        let test_data = serde_json::to_string(&test).unwrap();
        bench_fn(c, &name, &test_data)
    }
}
