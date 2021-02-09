extern crate quote;
extern crate syn;

mod features;

use quote::quote;
use syn::Pat;

#[test]
fn test_pat_ident() {
    match syn::parse2(quote!(self)).unwrap() {
        Pat::Ident(_) => (),
        value => panic!("expected PatIdent, got {:?}", value),
    }
}

#[test]
fn test_pat_path() {
    match syn::parse2(quote!(self::CONST)).unwrap() {
        Pat::Path(_) => (),
        value => panic!("expected PatPath, got {:?}", value),
    }
}
