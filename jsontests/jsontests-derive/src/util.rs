use quote;
use std::path::Path;
use syn::Ident;

use crate::attr::Config;
use std::ffi::OsStr;

pub fn open_directory_module(dirname: &str, tokens: &mut quote::Tokens) {
    // create identifier
    let dirname = sanitize_ident(dirname);
    let dirname_ident = Ident::from(dirname.as_ref());

    open_module(dirname_ident, tokens);
}

pub fn open_file_module(filepath: impl AsRef<Path>, tokens: &mut quote::Tokens) {
    let filepath = filepath.as_ref();
    // get file name without extension
    let filename = filepath.file_stem().and_then(OsStr::to_str).unwrap();
    // create identifier
    let filename = sanitize_ident(filename);
    let filename_ident = Ident::from(filename.as_ref());

    open_module(filename_ident, tokens);
}

pub fn open_module<I: Into<Ident>>(module_name: I, tokens: &mut quote::Tokens) {
    let module_name = module_name.into();
    // append module opening tokens
    tokens.append(quote! {
        pub(crate) mod #module_name
    });
    tokens.append("{");
}

pub fn close_brace(tokens: &mut quote::Tokens) {
    tokens.append("}")
}

pub fn sanitize_ident(ident: &str) -> String {
    // replace empty ident
    let ident = if ident.is_empty() {
        String::from("unnamed")
    } else {
        ident.to_string()
    };

    // prepend alphabetic character if token starts with non-alphabetic character
    let ident = if ident.chars().nth(0).map(|c| !c.is_alphabetic()).unwrap_or(true) {
        format!("x_{}", ident)
    } else {
        ident
    };

    // replace special characters with _
    escape_as_underscore(&ident, "!@#$%^&*-+=/<>;\'\"()`~")
}

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::time::Instant;

thread_local! {
    static UNDERSCORE_ENCODING_MAP: RefCell<HashMap<char, u8>> = RefCell::new(
        [('-', 1), ('_', 2)].iter().cloned().collect()
    );
    static UNDERSCORE_ENCODING_MAX: Cell<u8> = Cell::new(0);
}

fn escape_as_underscore(s: &str, from: &str) -> String {
    let mut initial = s.to_owned();
    for c in from.chars() {
        let replacement: String = UNDERSCORE_ENCODING_MAP.with(|map| {
            let mut map = map.borrow_mut();
            let cnt = map.entry(c).or_insert_with(|| {
                UNDERSCORE_ENCODING_MAX.with(|max| {
                    let cnt = max.get() + 1;
                    max.set(cnt);
                    cnt
                })
            });
            std::iter::repeat('_').take(*cnt as usize).collect()
        });
        initial = initial.replace(c, &replacement);
    }
    initial
}

pub struct Timer<'a> {
    msg: &'a str,
    start: Instant,
}

impl<'a> Timer<'a> {
    pub fn new(msg: &'a str) -> Self {
        use std::io::Write;
        eprintln!("[start] {}", msg);
        Timer {
            msg,
            start: Instant::now(),
        }
    }
}

impl<'a> Drop for Timer<'a> {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed();
        let elapsed_secs = elapsed.as_secs();
        let elapsed_millis = elapsed.subsec_millis();
        let elapsed_secs_float = (elapsed_secs as f64) + (elapsed_millis as f64) / 1000.0;
        eprintln!("[{:.3}] {} finished", elapsed_secs_float, self.msg);
    }
}
