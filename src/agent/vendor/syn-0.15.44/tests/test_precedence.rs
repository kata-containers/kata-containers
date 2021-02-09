#![cfg(not(syn_disable_nightly_tests))]
#![recursion_limit = "1024"]
#![feature(rustc_private)]

//! The tests in this module do the following:
//!
//! 1. Parse a given expression in both `syn` and `libsyntax`.
//! 2. Fold over the expression adding brackets around each subexpression (with
//!    some complications - see the `syn_brackets` and `libsyntax_brackets`
//!    methods).
//! 3. Serialize the `syn` expression back into a string, and re-parse it with
//!    `libsyntax`.
//! 4. Respan all of the expressions, replacing the spans with the default
//!    spans.
//! 5. Compare the expressions with one another, if they are not equal fail.

extern crate quote;
extern crate rayon;
extern crate regex;
extern crate rustc_data_structures;
extern crate smallvec;
extern crate syn;
extern crate syntax;
extern crate syntax_pos;
extern crate walkdir;

mod features;

use quote::quote;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use regex::Regex;
use smallvec::smallvec;
use syntax::ast;
use syntax::ptr::P;
use syntax_pos::edition::Edition;
use walkdir::{DirEntry, WalkDir};

use std::fs::File;
use std::io::Read;
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};

use common::eq::SpanlessEq;
use common::parse;

#[macro_use]
mod macros;

#[allow(dead_code)]
mod common;

mod repo;

/// Test some pre-set expressions chosen by us.
#[test]
fn test_simple_precedence() {
    const EXPRS: &[&str] = &[
        "1 + 2 * 3 + 4",
        "1 + 2 * ( 3 + 4 )",
        "{ for i in r { } *some_ptr += 1; }",
        "{ loop { break 5; } }",
        "{ if true { () }.mthd() }",
        "{ for i in unsafe { 20 } { } }",
    ];

    let mut failed = 0;

    for input in EXPRS {
        let expr = if let Some(expr) = parse::syn_expr(input) {
            expr
        } else {
            failed += 1;
            continue;
        };

        let pf = match test_expressions(vec![expr]) {
            (1, 0) => "passed",
            (0, 1) => {
                failed += 1;
                "failed"
            }
            _ => unreachable!(),
        };
        errorf!("=== {}: {}\n", input, pf);
    }

    if failed > 0 {
        panic!("Failed {} tests", failed);
    }
}

/// Test expressions from rustc, like in `test_round_trip`.
#[test]
#[cfg_attr(target_os = "windows", ignore = "requires nix .sh")]
fn test_rustc_precedence() {
    repo::clone_rust();
    let abort_after = common::abort_after();
    if abort_after == 0 {
        panic!("Skipping all precedence tests");
    }

    let passed = AtomicUsize::new(0);
    let failed = AtomicUsize::new(0);

    // 2018 edition is hard
    let edition_regex = Regex::new(r"\b(async|try)[!(]").unwrap();

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

            // Our version of `libsyntax` can't parse this tests
            if path
                .to_str()
                .unwrap()
                .ends_with("optional_comma_in_match_arm.rs")
            {
                return;
            }

            let mut file = File::open(path).unwrap();
            let mut content = String::new();
            file.read_to_string(&mut content).unwrap();
            let content = edition_regex.replace_all(&content, "_$0");

            let (l_passed, l_failed) = match syn::parse_file(&content) {
                Ok(file) => {
                    let exprs = collect_exprs(file);
                    test_expressions(exprs)
                }
                Err(msg) => {
                    errorf!("syn failed to parse\n{:?}\n", msg);
                    (0, 1)
                }
            };

            errorf!(
                "=== {}: {} passed | {} failed\n",
                path.display(),
                l_passed,
                l_failed
            );

            passed.fetch_add(l_passed, Ordering::SeqCst);
            let prev_failed = failed.fetch_add(l_failed, Ordering::SeqCst);

            if prev_failed + l_failed >= abort_after {
                process::exit(1);
            }
        });

    let passed = passed.load(Ordering::SeqCst);
    let failed = failed.load(Ordering::SeqCst);

    errorf!("\n===== Precedence Test Results =====\n");
    errorf!("{} passed | {} failed\n", passed, failed);

    if failed > 0 {
        panic!("{} failures", failed);
    }
}

fn test_expressions(exprs: Vec<syn::Expr>) -> (usize, usize) {
    let mut passed = 0;
    let mut failed = 0;

    syntax::with_globals(Edition::Edition2018, || {
        for expr in exprs {
            let raw = quote!(#expr).to_string();

            let libsyntax_ast = if let Some(e) = libsyntax_parse_and_rewrite(&raw) {
                e
            } else {
                failed += 1;
                errorf!("\nFAIL - libsyntax failed to parse raw\n");
                continue;
            };

            let syn_expr = syn_brackets(expr);
            let syn_ast = if let Some(e) = parse::libsyntax_expr(&quote!(#syn_expr).to_string()) {
                e
            } else {
                failed += 1;
                errorf!("\nFAIL - libsyntax failed to parse bracketed\n");
                continue;
            };

            if SpanlessEq::eq(&syn_ast, &libsyntax_ast) {
                passed += 1;
            } else {
                failed += 1;
                errorf!("\nFAIL\n{:?}\n!=\n{:?}\n", syn_ast, libsyntax_ast);
            }
        }
    });

    (passed, failed)
}

fn libsyntax_parse_and_rewrite(input: &str) -> Option<P<ast::Expr>> {
    parse::libsyntax_expr(input).and_then(libsyntax_brackets)
}

/// Wrap every expression which is not already wrapped in parens with parens, to
/// reveal the precidence of the parsed expressions, and produce a stringified
/// form of the resulting expression.
///
/// This method operates on libsyntax objects.
fn libsyntax_brackets(mut libsyntax_expr: P<ast::Expr>) -> Option<P<ast::Expr>> {
    use rustc_data_structures::thin_vec::ThinVec;
    use smallvec::SmallVec;
    use std::mem;
    use syntax::ast::{AwaitOrigin, Expr, ExprKind, Field, Mac, Pat, Stmt, StmtKind, Ty};
    use syntax::mut_visit::{self, MutVisitor};
    use syntax_pos::DUMMY_SP;

    struct BracketsVisitor {
        failed: bool,
    };

    impl BracketsVisitor {
        fn recurse_expr(&mut self, e: &mut Expr) {
            match e.node {
                ExprKind::Await(AwaitOrigin::MacroLike, _) => {
                    // Syn sees await!() as macro and doesn't recurse inside, so
                    // skip it here too.
                }
                _ => mut_visit::noop_visit_expr(e, self),
            }
        }
    }

    impl MutVisitor for BracketsVisitor {
        fn visit_expr(&mut self, e: &mut P<Expr>) {
            self.recurse_expr(e);
            match e.node {
                ExprKind::If(..) | ExprKind::Block(..) | ExprKind::Let(..) => {}
                _ => {
                    let inner = mem::replace(
                        e,
                        P(Expr {
                            id: ast::DUMMY_NODE_ID,
                            node: ExprKind::Err,
                            span: DUMMY_SP,
                            attrs: ThinVec::new(),
                        }),
                    );
                    e.node = ExprKind::Paren(inner);
                }
            }
        }

        fn visit_field(&mut self, f: &mut Field) {
            if f.is_shorthand {
                self.recurse_expr(&mut f.expr);
            } else {
                self.visit_expr(&mut f.expr);
            }
        }

        // We don't want to look at expressions that might appear in patterns or
        // types yet. We'll look into comparing those in the future. For now
        // focus on expressions appearing in other places.
        fn visit_pat(&mut self, pat: &mut P<Pat>) {
            let _ = pat;
        }

        fn visit_ty(&mut self, ty: &mut P<Ty>) {
            let _ = ty;
        }

        fn flat_map_stmt(&mut self, stmt: Stmt) -> SmallVec<[Stmt; 1]> {
            let node = match stmt.node {
                // Don't wrap toplevel expressions in statements.
                StmtKind::Expr(mut e) => {
                    self.recurse_expr(&mut e);
                    StmtKind::Expr(e)
                }
                StmtKind::Semi(mut e) => {
                    self.recurse_expr(&mut e);
                    StmtKind::Semi(e)
                }
                s => s,
            };

            smallvec![Stmt { node, ..stmt }]
        }

        fn visit_mac(&mut self, mac: &mut Mac) {
            // By default when folding over macros, libsyntax panics. This is
            // because it's usually not what you want, you want to run after
            // macro expansion. We do want to do that (syn doesn't do macro
            // expansion), so we implement visit_mac to just return the macro
            // unchanged.
            let _ = mac;
        }
    }

    let mut folder = BracketsVisitor { failed: false };
    folder.visit_expr(&mut libsyntax_expr);
    if folder.failed {
        None
    } else {
        Some(libsyntax_expr)
    }
}

/// Wrap every expression which is not already wrapped in parens with parens, to
/// reveal the precedence of the parsed expressions, and produce a stringified
/// form of the resulting expression.
fn syn_brackets(syn_expr: syn::Expr) -> syn::Expr {
    use syn::fold::*;
    use syn::*;

    struct ParenthesizeEveryExpr;
    impl Fold for ParenthesizeEveryExpr {
        fn fold_expr(&mut self, expr: Expr) -> Expr {
            match expr {
                Expr::Group(_) => unreachable!(),
                Expr::If(..) | Expr::Unsafe(..) | Expr::Block(..) | Expr::Let(..) => {
                    fold_expr(self, expr)
                }
                node => Expr::Paren(ExprParen {
                    attrs: Vec::new(),
                    expr: Box::new(fold_expr(self, node)),
                    paren_token: token::Paren::default(),
                }),
            }
        }

        fn fold_stmt(&mut self, stmt: Stmt) -> Stmt {
            match stmt {
                // Don't wrap toplevel expressions in statements.
                Stmt::Expr(e) => Stmt::Expr(fold_expr(self, e)),
                Stmt::Semi(e, semi) => Stmt::Semi(fold_expr(self, e), semi),
                s => s,
            }
        }

        // We don't want to look at expressions that might appear in patterns or
        // types yet. We'll look into comparing those in the future. For now
        // focus on expressions appearing in other places.
        fn fold_pat(&mut self, pat: Pat) -> Pat {
            pat
        }

        fn fold_type(&mut self, ty: Type) -> Type {
            ty
        }
    }

    let mut folder = ParenthesizeEveryExpr;
    folder.fold_expr(syn_expr)
}

/// Walk through a crate collecting all expressions we can find in it.
fn collect_exprs(file: syn::File) -> Vec<syn::Expr> {
    use syn::fold::*;
    use syn::punctuated::Punctuated;
    use syn::*;

    struct CollectExprs(Vec<Expr>);
    impl Fold for CollectExprs {
        fn fold_expr(&mut self, expr: Expr) -> Expr {
            self.0.push(expr);

            Expr::Tuple(ExprTuple {
                attrs: vec![],
                elems: Punctuated::new(),
                paren_token: token::Paren::default(),
            })
        }
    }

    let mut folder = CollectExprs(vec![]);
    folder.fold_file(file);
    folder.0
}
