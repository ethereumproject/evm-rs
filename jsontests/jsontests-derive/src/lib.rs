#[macro_use]
extern crate quote;
extern crate proc_macro;

mod attr;
mod tests;
mod util;

use failure::Error;
use itertools::Itertools;
use serde_json as json;

use proc_macro::TokenStream;
use syn::Ident;

use self::{
    attr::{extract_attrs, Config, Runtime},
    tests::read_tests_from_dir,
    util::*,
};
use crate::tests::{Test, TestAST, TestASTRunner};
use std::ffi::OsStr;
use std::path::Path;

#[proc_macro_derive(
    JsonTests,
    attributes(
        directory,
        test_with,
        bench_with,
        criterion_config,
        skip,
        should_panic,
        patch,
        runtime
    )
)]
pub fn json_tests(input: TokenStream) -> TokenStream {
    // Construct a string representation of the type definition
    let s = input.to_string();

    // Parse the string representation
    let ast = syn::parse_derive_input(&s).unwrap();

    // Build the impl
    let gen = match impl_json_tests(&ast) {
        Ok(tokens) => tokens,
        Err(err) => panic!("{}", err),
    };

    println!("{}", gen.to_string());

    // Return the generated impl
    gen.parse().unwrap()
}

fn impl_json_tests(ast: &syn::DeriveInput) -> Result<quote::Tokens, Error> {
    let config = extract_attrs(&ast)?;
    let tests = read_tests_from_dir(&config, &config.directory)?;
    let mut tokens = quote::Tokens::new();
    let mut bench_idents = Vec::new();

    // If behchmarking support is requested, import Criterion
    if config.bench_with.is_some() {
        tokens.append(quote! {
            use criterion::Criterion;
        })
    }

    let mut ast_runner = AstRunner {
        config: &config,
        tokens,
        bench_idents: Vec::new(),
    };

    tests.traverse(&mut ast_runner);

    let mut tokens = ast_runner.tokens;

    generate_criterion_macros(&config, &bench_idents, &mut tokens);

    Ok(tokens)
}

struct AstRunner<'a> {
    config: &'a Config,
    tokens: quote::Tokens,
    bench_idents: Vec<Ident>,
}

impl TestASTRunner for AstRunner<'_> {
    fn handle_open_module(&mut self, name: String, nodes: &[TestAST]) {
        open_module(sanitize_ident(&name), &mut self.tokens);
    }

    fn handle_close_module(&mut self) {
        close_brace(&mut self.tokens)
    }

    fn handle_open_test_file(&mut self, name: String, nodes: &[TestAST]) {
        open_module(sanitize_ident(&name), &mut self.tokens)
    }

    fn handle_close_test_file(&mut self) {
        close_brace(&mut self.tokens)
    }

    fn handle_test(&mut self, test: Test) {
        let data = test.data.map(|d| json::to_string(&d).unwrap());
        let name = sanitize_ident(&test.name);
        let name_ident = Ident::from(name.as_ref());
        generate_test(&self.config, &test.path, &name_ident, &data, &mut self.tokens);
        /*
        generate_bench(&config, &name_ident, &data, &mut tokens).map(|mut ident| {
            // prepend dir submodule
            ident = Ident::from(format!("{}::{}", dir_mod_name, ident.as_ref()));
            // prepend file submodule
            if need_file_submodule {
                ident = Ident::from(format!("{}::{}", file_mod_name.as_ref().unwrap(), ident.as_ref()));
            }
            bench_idents.push(ident);
        });
        */
    }
}

fn generate_test(
    config: &Config,
    path: impl AsRef<Path>,
    test_name: &Ident,
    data: &Option<String>,
    tokens: &mut quote::Tokens,
) {
    let test_func_path = &config.test_with.path;
    let test_func_name = &config.test_with.name;
    let test_name_str = test_name.as_ref();
    let (patch_name, patch_path) = derive_patch(config);

    tokens.append(quote! {#[test]});
    if config.should_panic {
        tokens.append(quote! {#[should_panic]});
    }

    match config.runtime {
        Runtime::Static => {
            let data = data.as_ref().unwrap();
            tokens.append(quote! {
                fn #test_name() {
                    use #test_func_path;
                    use #patch_path;
                    let data = #data;
                    #test_func_name::<#patch_name>(#test_name_str, data);
                }
            });
        }
        Runtime::Dynamic => {
            let path = path.as_ref().to_str().unwrap();
            tokens.append(quote! {
                fn #test_name() {
                    use #test_func_path;
                    use #patch_path;
                    jsontests::run_tests_from_file(#path, #test_func_name::<#patch_name>)
                }
            });
        }
    }
}

fn generate_bench(config: &Config, test_name: &Ident, data: &str, tokens: &mut quote::Tokens) -> Option<Ident> {
    if config.bench_with.is_none() {
        return None;
    }

    let bench = config.bench_with.as_ref().unwrap();
    let bench_func_path = &bench.path;
    let bench_func_name = &bench.name;

    let bench_name = format!("bench_{}", test_name.as_ref());
    let bench_ident = Ident::from(bench_name.as_ref());

    let (patch_name, patch_path) = derive_patch(config);

    tokens.append(quote! {
        pub fn #bench_ident(c: &mut Criterion) {
            use #bench_func_path;
            use #patch_path;
            let data = #data;
            #bench_func_name::<#patch_name>(c, #bench_name, data);
        }
    });

    Some(bench_ident)
}

fn generate_criterion_macros(config: &Config, benches: &[Ident], tokens: &mut quote::Tokens) {
    // Generate criterion macros
    if config.bench_with.is_some() {
        let benches = benches.iter().map(AsRef::as_ref).join(" , ");
        let config = config
            .criterion_config
            .as_ref()
            .map(|cfg| cfg.path.clone())
            .unwrap_or_else(|| Ident::from("Criterion::default"));
        let template = quote! {
            criterion_group! {
                name = main;
                config = #config();
                targets = TARGETS
            };
        };
        tokens.append(template.as_ref().replace("TARGETS", &benches));
    }
}

fn derive_patch(config: &Config) -> (Ident, Ident) {
    if let Some(patch) = config.patch.as_ref() {
        (patch.name.clone(), patch.path.clone())
    } else {
        (Ident::from("VMTestPatch"), Ident::from("evm::VMTestPatch"))
    }
}
