#![cfg(not(syn_disable_nightly_tests))]
#![recursion_limit = "1024"]
#![feature(rustc_private)]

extern crate quote;
extern crate rayon;
extern crate syn;
extern crate syntax;
extern crate syntax_pos;
extern crate walkdir;

mod features;

use quote::quote;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use syntax::ast;
use syntax::parse::{self, PResult, ParseSess};
use syntax::source_map::FilePathMapping;
use syntax_pos::edition::Edition;
use syntax_pos::FileName;
use walkdir::{DirEntry, WalkDir};

use std::fs::File;
use std::io::Read;
use std::panic;
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

#[macro_use]
mod macros;

#[allow(dead_code)]
mod common;

mod repo;

use common::eq::SpanlessEq;

#[test]
#[cfg_attr(target_os = "windows", ignore = "requires nix .sh")]
fn test_round_trip() {
    repo::clone_rust();
    let abort_after = common::abort_after();
    if abort_after == 0 {
        panic!("Skipping all round_trip tests");
    }

    let failed = AtomicUsize::new(0);

    WalkDir::new("tests/rust")
        .sort_by(|a, b| a.file_name().cmp(b.file_name()))
        .into_iter()
        .filter_entry(repo::base_dir_filter)
        .collect::<Result<Vec<DirEntry>, walkdir::Error>>()
        .unwrap()
        .into_par_iter()
        .for_each(|entry| {
            let path = entry.path();
            if path.is_dir() {
                return;
            }

            let mut file = File::open(path).unwrap();
            let mut content = String::new();
            file.read_to_string(&mut content).unwrap();

            let start = Instant::now();
            let (krate, elapsed) = match syn::parse_file(&content) {
                Ok(krate) => (krate, start.elapsed()),
                Err(msg) => {
                    errorf!("=== {}: syn failed to parse\n{:?}\n", path.display(), msg);
                    let prev_failed = failed.fetch_add(1, Ordering::SeqCst);
                    if prev_failed + 1 >= abort_after {
                        process::exit(1);
                    }
                    return;
                }
            };
            let back = quote!(#krate).to_string();

            let equal = panic::catch_unwind(|| {
                syntax::with_globals(Edition::Edition2018, || {
                    let sess = ParseSess::new(FilePathMapping::empty());
                    let before = match libsyntax_parse(content, &sess) {
                        Ok(before) => before,
                        Err(mut diagnostic) => {
                            diagnostic.cancel();
                            if diagnostic
                                .message()
                                .starts_with("file not found for module")
                            {
                                errorf!("=== {}: ignore\n", path.display());
                            } else {
                                errorf!(
                                "=== {}: ignore - libsyntax failed to parse original content: {}\n",
                                path.display(),
                                diagnostic.message()
                            );
                            }
                            return true;
                        }
                    };
                    let after = match libsyntax_parse(back, &sess) {
                        Ok(after) => after,
                        Err(mut diagnostic) => {
                            errorf!("=== {}: libsyntax failed to parse", path.display());
                            diagnostic.emit();
                            return false;
                        }
                    };

                    if SpanlessEq::eq(&before, &after) {
                        errorf!(
                            "=== {}: pass in {}ms\n",
                            path.display(),
                            elapsed.as_secs() * 1000
                                + u64::from(elapsed.subsec_nanos()) / 1_000_000
                        );
                        true
                    } else {
                        errorf!(
                            "=== {}: FAIL\nbefore: {:#?}\nafter: {:#?}\n",
                            path.display(),
                            before,
                            after,
                        );
                        false
                    }
                })
            });
            match equal {
                Err(_) => errorf!("=== {}: ignoring libsyntax panic\n", path.display()),
                Ok(true) => {}
                Ok(false) => {
                    let prev_failed = failed.fetch_add(1, Ordering::SeqCst);
                    if prev_failed + 1 >= abort_after {
                        process::exit(1);
                    }
                }
            }
        });

    let failed = failed.load(Ordering::SeqCst);
    if failed > 0 {
        panic!("{} failures", failed);
    }
}

fn libsyntax_parse(content: String, sess: &ParseSess) -> PResult<ast::Crate> {
    let name = FileName::Custom("test_round_trip".to_string());
    parse::parse_crate_from_source_str(name, content, sess)
}
