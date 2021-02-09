extern crate proc_macro2;

use std::str::{self, FromStr};

use proc_macro2::{Ident, Literal, Spacing, Span, TokenStream, TokenTree};

#[test]
fn idents() {
    assert_eq!(
        Ident::new("String", Span::call_site()).to_string(),
        "String"
    );
    assert_eq!(Ident::new("fn", Span::call_site()).to_string(), "fn");
    assert_eq!(Ident::new("_", Span::call_site()).to_string(), "_");
}

#[test]
#[cfg(procmacro2_semver_exempt)]
fn raw_idents() {
    assert_eq!(
        Ident::new_raw("String", Span::call_site()).to_string(),
        "r#String"
    );
    assert_eq!(Ident::new_raw("fn", Span::call_site()).to_string(), "r#fn");
    assert_eq!(Ident::new_raw("_", Span::call_site()).to_string(), "r#_");
}

#[test]
#[should_panic(expected = "Ident is not allowed to be empty; use Option<Ident>")]
fn ident_empty() {
    Ident::new("", Span::call_site());
}

#[test]
#[should_panic(expected = "Ident cannot be a number; use Literal instead")]
fn ident_number() {
    Ident::new("255", Span::call_site());
}

#[test]
#[should_panic(expected = "\"a#\" is not a valid Ident")]
fn ident_invalid() {
    Ident::new("a#", Span::call_site());
}

#[test]
#[should_panic(expected = "not a valid Ident")]
fn raw_ident_empty() {
    Ident::new("r#", Span::call_site());
}

#[test]
#[should_panic(expected = "not a valid Ident")]
fn raw_ident_number() {
    Ident::new("r#255", Span::call_site());
}

#[test]
#[should_panic(expected = "\"r#a#\" is not a valid Ident")]
fn raw_ident_invalid() {
    Ident::new("r#a#", Span::call_site());
}

#[test]
#[should_panic(expected = "not a valid Ident")]
fn lifetime_empty() {
    Ident::new("'", Span::call_site());
}

#[test]
#[should_panic(expected = "not a valid Ident")]
fn lifetime_number() {
    Ident::new("'255", Span::call_site());
}

#[test]
#[should_panic(expected = r#""\'a#" is not a valid Ident"#)]
fn lifetime_invalid() {
    Ident::new("'a#", Span::call_site());
}

#[test]
fn literal_string() {
    assert_eq!(Literal::string("foo").to_string(), "\"foo\"");
    assert_eq!(Literal::string("\"").to_string(), "\"\\\"\"");
    assert_eq!(Literal::string("didn't").to_string(), "\"didn't\"");
}

#[test]
fn literal_character() {
    assert_eq!(Literal::character('x').to_string(), "'x'");
    assert_eq!(Literal::character('\'').to_string(), "'\\''");
    assert_eq!(Literal::character('"').to_string(), "'\"'");
}

#[test]
fn literal_float() {
    assert_eq!(Literal::f32_unsuffixed(10.0).to_string(), "10.0");
}

#[test]
fn roundtrip() {
    fn roundtrip(p: &str) {
        println!("parse: {}", p);
        let s = p.parse::<TokenStream>().unwrap().to_string();
        println!("first: {}", s);
        let s2 = s.to_string().parse::<TokenStream>().unwrap().to_string();
        assert_eq!(s, s2);
    }
    roundtrip("a");
    roundtrip("<<");
    roundtrip("<<=");
    roundtrip(
        "
        1
        1.0
        1f32
        2f64
        1usize
        4isize
        4e10
        1_000
        1_0i32
        8u8
        9
        0
        0xffffffffffffffffffffffffffffffff
    ",
    );
    roundtrip("'a");
    roundtrip("'_");
    roundtrip("'static");
    roundtrip("'\\u{10__FFFF}'");
    roundtrip("\"\\u{10_F0FF__}foo\\u{1_0_0_0__}\"");
}

#[test]
fn fail() {
    fn fail(p: &str) {
        if let Ok(s) = p.parse::<TokenStream>() {
            panic!("should have failed to parse: {}\n{:#?}", p, s);
        }
    }
    fail("1x");
    fail("1u80");
    fail("1f320");
    fail("' static");
    fail("r#1");
    fail("r#_");
}

#[cfg(span_locations)]
#[test]
fn span_test() {
    use proc_macro2::TokenTree;

    fn check_spans(p: &str, mut lines: &[(usize, usize, usize, usize)]) {
        let ts = p.parse::<TokenStream>().unwrap();
        check_spans_internal(ts, &mut lines);
    }

    fn check_spans_internal(ts: TokenStream, lines: &mut &[(usize, usize, usize, usize)]) {
        for i in ts {
            if let Some((&(sline, scol, eline, ecol), rest)) = lines.split_first() {
                *lines = rest;

                let start = i.span().start();
                assert_eq!(start.line, sline, "sline did not match for {}", i);
                assert_eq!(start.column, scol, "scol did not match for {}", i);

                let end = i.span().end();
                assert_eq!(end.line, eline, "eline did not match for {}", i);
                assert_eq!(end.column, ecol, "ecol did not match for {}", i);

                match i {
                    TokenTree::Group(ref g) => {
                        check_spans_internal(g.stream().clone(), lines);
                    }
                    _ => {}
                }
            }
        }
    }

    check_spans(
        "\
/// This is a document comment
testing 123
{
  testing 234
}",
        &[
            (1, 0, 1, 30),  // #
            (1, 0, 1, 30),  // [ ... ]
            (1, 0, 1, 30),  // doc
            (1, 0, 1, 30),  // =
            (1, 0, 1, 30),  // "This is..."
            (2, 0, 2, 7),   // testing
            (2, 8, 2, 11),  // 123
            (3, 0, 5, 1),   // { ... }
            (4, 2, 4, 9),   // testing
            (4, 10, 4, 13), // 234
        ],
    );
}

#[cfg(procmacro2_semver_exempt)]
#[cfg(not(nightly))]
#[test]
fn default_span() {
    let start = Span::call_site().start();
    assert_eq!(start.line, 1);
    assert_eq!(start.column, 0);
    let end = Span::call_site().end();
    assert_eq!(end.line, 1);
    assert_eq!(end.column, 0);
    let source_file = Span::call_site().source_file();
    assert_eq!(source_file.path().to_string_lossy(), "<unspecified>");
    assert!(!source_file.is_real());
}

#[cfg(procmacro2_semver_exempt)]
#[test]
fn span_join() {
    let source1 = "aaa\nbbb"
        .parse::<TokenStream>()
        .unwrap()
        .into_iter()
        .collect::<Vec<_>>();
    let source2 = "ccc\nddd"
        .parse::<TokenStream>()
        .unwrap()
        .into_iter()
        .collect::<Vec<_>>();

    assert!(source1[0].span().source_file() != source2[0].span().source_file());
    assert_eq!(
        source1[0].span().source_file(),
        source1[1].span().source_file()
    );

    let joined1 = source1[0].span().join(source1[1].span());
    let joined2 = source1[0].span().join(source2[0].span());
    assert!(joined1.is_some());
    assert!(joined2.is_none());

    let start = joined1.unwrap().start();
    let end = joined1.unwrap().end();
    assert_eq!(start.line, 1);
    assert_eq!(start.column, 0);
    assert_eq!(end.line, 2);
    assert_eq!(end.column, 3);

    assert_eq!(
        joined1.unwrap().source_file(),
        source1[0].span().source_file()
    );
}

#[test]
fn no_panic() {
    let s = str::from_utf8(b"b\'\xc2\x86  \x00\x00\x00^\"").unwrap();
    assert!(s.parse::<proc_macro2::TokenStream>().is_err());
}

#[test]
fn tricky_doc_comment() {
    let stream = "/**/".parse::<proc_macro2::TokenStream>().unwrap();
    let tokens = stream.into_iter().collect::<Vec<_>>();
    assert!(tokens.is_empty(), "not empty -- {:?}", tokens);

    let stream = "/// doc".parse::<proc_macro2::TokenStream>().unwrap();
    let tokens = stream.into_iter().collect::<Vec<_>>();
    assert!(tokens.len() == 2, "not length 2 -- {:?}", tokens);
    match tokens[0] {
        proc_macro2::TokenTree::Punct(ref tt) => assert_eq!(tt.as_char(), '#'),
        _ => panic!("wrong token {:?}", tokens[0]),
    }
    let mut tokens = match tokens[1] {
        proc_macro2::TokenTree::Group(ref tt) => {
            assert_eq!(tt.delimiter(), proc_macro2::Delimiter::Bracket);
            tt.stream().into_iter()
        }
        _ => panic!("wrong token {:?}", tokens[0]),
    };

    match tokens.next().unwrap() {
        proc_macro2::TokenTree::Ident(ref tt) => assert_eq!(tt.to_string(), "doc"),
        t => panic!("wrong token {:?}", t),
    }
    match tokens.next().unwrap() {
        proc_macro2::TokenTree::Punct(ref tt) => assert_eq!(tt.as_char(), '='),
        t => panic!("wrong token {:?}", t),
    }
    match tokens.next().unwrap() {
        proc_macro2::TokenTree::Literal(ref tt) => {
            assert_eq!(tt.to_string(), "\" doc\"");
        }
        t => panic!("wrong token {:?}", t),
    }
    assert!(tokens.next().is_none());

    let stream = "//! doc".parse::<proc_macro2::TokenStream>().unwrap();
    let tokens = stream.into_iter().collect::<Vec<_>>();
    assert!(tokens.len() == 3, "not length 3 -- {:?}", tokens);
}

#[test]
fn op_before_comment() {
    let mut tts = TokenStream::from_str("~// comment").unwrap().into_iter();
    match tts.next().unwrap() {
        TokenTree::Punct(tt) => {
            assert_eq!(tt.as_char(), '~');
            assert_eq!(tt.spacing(), Spacing::Alone);
        }
        wrong => panic!("wrong token {:?}", wrong),
    }
}

#[test]
fn raw_identifier() {
    let mut tts = TokenStream::from_str("r#dyn").unwrap().into_iter();
    match tts.next().unwrap() {
        TokenTree::Ident(raw) => assert_eq!("r#dyn", raw.to_string()),
        wrong => panic!("wrong token {:?}", wrong),
    }
    assert!(tts.next().is_none());
}

#[test]
fn test_debug_ident() {
    let ident = Ident::new("proc_macro", Span::call_site());

    #[cfg(not(procmacro2_semver_exempt))]
    let expected = "Ident(proc_macro)";

    #[cfg(procmacro2_semver_exempt)]
    let expected = "Ident { sym: proc_macro, span: bytes(0..0) }";

    assert_eq!(expected, format!("{:?}", ident));
}

#[test]
fn test_debug_tokenstream() {
    let tts = TokenStream::from_str("[a + 1]").unwrap();

    #[cfg(not(procmacro2_semver_exempt))]
    let expected = "\
TokenStream [
    Group {
        delimiter: Bracket,
        stream: TokenStream [
            Ident {
                sym: a,
            },
            Punct {
                op: '+',
                spacing: Alone,
            },
            Literal {
                lit: 1,
            },
        ],
    },
]\
    ";

    #[cfg(not(procmacro2_semver_exempt))]
    let expected_before_trailing_commas = "\
TokenStream [
    Group {
        delimiter: Bracket,
        stream: TokenStream [
            Ident {
                sym: a
            },
            Punct {
                op: '+',
                spacing: Alone
            },
            Literal {
                lit: 1
            }
        ]
    }
]\
    ";

    #[cfg(procmacro2_semver_exempt)]
    let expected = "\
TokenStream [
    Group {
        delimiter: Bracket,
        stream: TokenStream [
            Ident {
                sym: a,
                span: bytes(2..3),
            },
            Punct {
                op: '+',
                spacing: Alone,
                span: bytes(4..5),
            },
            Literal {
                lit: 1,
                span: bytes(6..7),
            },
        ],
        span: bytes(1..8),
    },
]\
    ";

    #[cfg(procmacro2_semver_exempt)]
    let expected_before_trailing_commas = "\
TokenStream [
    Group {
        delimiter: Bracket,
        stream: TokenStream [
            Ident {
                sym: a,
                span: bytes(2..3)
            },
            Punct {
                op: '+',
                spacing: Alone,
                span: bytes(4..5)
            },
            Literal {
                lit: 1,
                span: bytes(6..7)
            }
        ],
        span: bytes(1..8)
    }
]\
    ";

    let actual = format!("{:#?}", tts);
    if actual.ends_with(",\n]") {
        assert_eq!(expected, actual);
    } else {
        assert_eq!(expected_before_trailing_commas, actual);
    }
}

#[test]
fn default_tokenstream_is_empty() {
    let default_token_stream: TokenStream = Default::default();

    assert!(default_token_stream.is_empty());
}
