use failure::Error;
use std::collections::{HashMap, HashSet};
use std::fs::{self, read, DirEntry, File, FileType};
use std::iter;
use std::path::{Path, PathBuf};

use crate::attr::{Config, Runtime};
use json::Value;
use serde_json as json;

pub struct Test {
    pub path: PathBuf,
    pub name: String,
    pub data: Option<Value>,
}

pub enum TestAST {
    Module(String, Vec<TestAST>),
    TestFile(String, Vec<TestAST>),
    Test(Test),
}

impl TestAST {
    pub fn traverse(self, runner: &mut impl TestASTRunner) {
        match self {
            TestAST::Module(name, nodes) => {
                runner.handle_open_module(name, &nodes);
                nodes.into_iter().for_each(|n| Self::traverse(n, runner));
                runner.handle_close_module();
            }
            TestAST::TestFile(name, nodes) => {
                runner.handle_open_test_file(name, &nodes);
                nodes.into_iter().for_each(|n| Self::traverse(n, runner));
                runner.handle_close_test_file();
            }
            TestAST::Test(test) => {
                runner.handle_test(test);
            }
        }
    }
}

pub trait TestASTRunner {
    fn handle_test(&mut self, test: Test);
    fn handle_open_module(&mut self, name: String, nodes: &[TestAST]);
    fn handle_close_module(&mut self);
    fn handle_open_test_file(&mut self, name: String, nodes: &[TestAST]);
    fn handle_close_test_file(&mut self);
}

use crate::util::Timer;
use rayon::prelude::*;
use std::io::Write;
use std::time::Instant;

pub fn read_tests_from_dir<P: AsRef<Path>>(config: &Config, dir_path: P) -> Result<TestAST, Error> {
    let timer_msg = format!("reading tests from {}", dir_path.as_ref().display());
    let timer = Timer::new(&timer_msg);
    read_module(config, dir_path)
}

fn read_dir_and_emit_nodes(config: &Config, path: impl AsRef<Path>) -> Result<Vec<TestAST>, Error> {
    use std::convert::identity;

    // For every directory entry, running in parallel
    fs::read_dir(path)?.par_bridge()
        .map(|entry| -> Result<Option<TestAST>, Error> {
            let entry = entry?;
            let filetype = entry.file_type()?;
            if filetype.is_dir() {
                Ok(Some(read_module(config, entry.path())?))
            } else if filetype.is_file() {
                Ok(read_tests_file(config, entry.path())?)
            } else {
                println!("ommitting symlink {}", entry.path().display());
                Ok(None)
            }
        })
        // Turn Result<Option<T>, E> into Option<Result<T, E>>
        .map(Result::transpose)
        // Remove all Nones (skipped symlinks)
        .filter_map(identity)
        // Collect Iter<Result<T, E>> into Result<Vec<T>, E>
        .collect()
}

fn read_module(config: &Config, path: impl AsRef<Path>) -> Result<TestAST, Error> {
    assert!(path.as_ref().is_dir());

    let name = path
        .as_ref()
        .components()
        .last()
        .ok_or_else(|| failure::err_msg("empty path"))?
        .as_os_str()
        .to_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| failure::err_msg("path is not a valid utf-8"))?;

    let children = read_dir_and_emit_nodes(config, path)?;

    Ok(TestAST::Module(name, children))
}

fn read_tests_file(config: &Config, path: impl AsRef<Path>) -> Result<Option<TestAST>, Error> {
    assert!(path.as_ref().is_file());

    let path = path.as_ref().canonicalize()?.to_owned();

    let name = path
        .file_stem()
        .ok_or_else(|| failure::err_msg("couldn't read file stem"))?
        .to_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| failure::err_msg("path is not a valid utf-8"))?;

    // Skip non-json files
    if !path.extension().map(|e| e == "json").unwrap_or(false) {
        return Ok(None);
    }

    let tests: Vec<_>;

    match config.runtime {
        // Static runtime includes the json test inside the test function body
        // And can operate on per-test basis, unlike the dynamic runtime, which can only operate on files
        Runtime::Static => {
            let file = File::open(&path)?;
            let jsontests: Value = json::from_reader(file)?;

            // Move out the root object
            let jsontests = match jsontests {
                Value::Object(t) => t,
                _ => panic!("expected a json object at the root of test file"),
            };

            let iter = jsontests.into_iter().map(move |(name, data)| Test {
                path: path.clone(),
                name,
                data: Some(data),
            });

            tests = iter.map(TestAST::Test).collect();
        }
        Runtime::Dynamic => {
            let test = Test {
                path,
                name: name.clone(),
                data: None,
            };
            tests = vec![TestAST::Test(test)]
        }
    }

    Ok(Some(TestAST::TestFile(name, tests)))
}
