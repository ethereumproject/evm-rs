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

    // Return the generated impl
    gen.parse().unwrap()
}

fn impl_json_tests(ast: &syn::DeriveInput) -> Result<quote::Tokens, Error> {
    let config = extract_attrs(&ast)?;
    let tests = read_tests_from_dir(&config, &config.directory)?;
    let mut tokens = quote::Tokens::new();
    let struct_ident = &ast.ident;

    // If behchmarking support is requested, import Criterion
    if config.bench_with.is_some() {
        tokens.append(quote! {
            use criterion::Criterion as _;
        })
    }

    let mut ast_runner = AstRunner {
        config: &config,
        tokens,
        bench_idents: Vec::new(),
        modules: Vec::new(),
    };

    tests.traverse(&mut ast_runner);

    let mut tokens = ast_runner.tokens;

    generate_criterion_macros(&config, &struct_ident, &ast_runner.bench_idents, &mut tokens);

    Ok(tokens)
}

struct AstRunner<'a> {
    config: &'a Config,
    tokens: quote::Tokens,
    modules: Vec<String>,
    bench_idents: Vec<Ident>,
}

impl AstRunner<'_> {
    fn push_module(&mut self, name: String) {
        let mod_name = sanitize_ident(&name);
        self.modules.push(mod_name.clone());
        open_module(mod_name, &mut self.tokens);
    }

    fn pop_module(&mut self) {
        self.modules.pop();
        close_brace(&mut self.tokens)
    }
}

impl TestASTRunner for AstRunner<'_> {
    fn handle_test(&mut self, test: Test) {
        let data = test.data.map(|d| json::to_string(&d).unwrap());
        let name = sanitize_ident(&test.name);
        let name_ident = Ident::from(name.as_ref());
        generate_test(&self.config, &test.path, &name_ident, &data, &mut self.tokens);
        generate_bench(&self.config, &test.path, &name_ident, &data, &mut self.tokens).map(|mut ident| {
            // prepare sumbodule path
            let modules_chain = self.modules.join("::");
            let bench_ident = format!("{}::{}", modules_chain, ident);
            self.bench_idents.push(bench_ident.into());
        });
    }

    fn handle_open_module(&mut self, name: String, _nodes: &[TestAST]) {
        self.push_module(name);
    }

    fn handle_close_module(&mut self) {
        self.pop_module()
    }

    fn handle_open_test_file(&mut self, name: String, _nodes: &[TestAST]) {
        self.push_module(name);
    }

    fn handle_close_test_file(&mut self) {
        self.pop_module()
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
                pub(crate) fn #test_name() {
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
                pub(crate) fn #test_name() {
                    use #test_func_path;
                    use #patch_path;
                    jsontests::run_tests_from_file(#path, #test_func_name::<#patch_name>)
                }
            });
        }
    }
}

fn generate_bench(
    config: &Config,
    path: impl AsRef<Path>,
    test_name: &Ident,
    data: &Option<String>,
    tokens: &mut quote::Tokens,
) -> Option<Ident> {
    if config.bench_with.is_none() {
        return None;
    }

    let bench = config.bench_with.as_ref().unwrap();
    let bench_func_path = &bench.path;
    let bench_func_name = &bench.name;

    let bench_name = format!("bench_{}", test_name.as_ref());
    let bench_ident = Ident::from(bench_name.as_ref());

    let (patch_name, patch_path) = derive_patch(config);

    match config.runtime {
        Runtime::Static => {
            let data = data.as_ref().unwrap();
            tokens.append(quote! {
                pub(crate) fn #bench_ident(c: &mut criterion::Criterion) {
                    use #bench_func_path;
                    use #patch_path;
                    let data = #data;
                    #bench_func_name::<#patch_name>(c, #bench_name, data);
                }
            });
        }
        Runtime::Dynamic => {
            let path = path.as_ref().to_str().unwrap();
            tokens.append(quote! {
                pub(crate) fn #bench_ident(c: &mut criterion::Criterion) {
                    use #bench_func_path;
                    use #patch_path;
                    jsontests::run_bench_from_file(c, #path, #bench_func_name::<#patch_name>)
                }
            });
        }
    }

    Some(bench_ident)
}

fn generate_criterion_macros(config: &Config, group_name: &Ident, benches: &[Ident], tokens: &mut quote::Tokens) {
    let group_name = format!("{}_bench_main", group_name);
    let group_name = Ident::from(sanitize_ident(&group_name));
    // Generate criterion macros
    if config.bench_with.is_some() {
        let benches = benches.iter().map(AsRef::as_ref).join(" , ");
        let config = config
            .criterion_config
            .as_ref()
            .map(|cfg| cfg.path.clone())
            .unwrap_or_else(|| Ident::from("criterion::Criterion::default"));
        let template = quote! {
            criterion_group! {
                name = #group_name;
                config = #config();
                targets = TARGETS
            }
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
