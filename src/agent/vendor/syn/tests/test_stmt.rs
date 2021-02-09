#[macro_use]
mod macros;

use syn::Stmt;

#[test]
fn test_raw_operator() {
    let stmt = syn::parse_str::<Stmt>("let _ = &raw const x;").unwrap();

    snapshot!(stmt, @r###"
    Local(Local {
        pat: Pat::Wild,
        init: Some(Verbatim(`& raw const x`)),
    })
    "###);
}

#[test]
fn test_raw_variable() {
    let stmt = syn::parse_str::<Stmt>("let _ = &raw;").unwrap();

    snapshot!(stmt, @r###"
    Local(Local {
        pat: Pat::Wild,
        init: Some(Expr::Reference {
            expr: Expr::Path {
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "raw",
                            arguments: None,
                        },
                    ],
                },
            },
        }),
    })
    "###);
}

#[test]
fn test_raw_invalid() {
    assert!(syn::parse_str::<Stmt>("let _ = &raw x;").is_err());
}
