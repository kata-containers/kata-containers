extern crate proc_macro2;
extern crate syn;

use std::str::FromStr;

use proc_macro2::TokenStream;
use syn::{Expr, ExprRange};

#[test]
fn test_expr_parse() {
    let code = "..100u32";
    let tt = TokenStream::from_str(code).unwrap();
    let expr: Expr = syn::parse2(tt.clone()).unwrap();
    let expr_range: ExprRange = syn::parse2(tt).unwrap();
    assert_eq!(expr, Expr::Range(expr_range));
}
