extern crate proc_macro2;
extern crate quote;
extern crate syn;

mod features;

use proc_macro2::{TokenStream, TokenTree};
use quote::ToTokens;
use std::str::FromStr;
use syn::{FloatSuffix, IntSuffix, Lit};

fn lit(s: &str) -> Lit {
    match TokenStream::from_str(s)
        .unwrap()
        .into_iter()
        .next()
        .unwrap()
    {
        TokenTree::Literal(lit) => Lit::new(lit),
        _ => panic!(),
    }
}

#[test]
fn strings() {
    fn test_string(s: &str, value: &str) {
        match lit(s) {
            Lit::Str(lit) => {
                assert_eq!(lit.value(), value);
                let again = lit.into_token_stream().to_string();
                if again != s {
                    test_string(&again, value);
                }
            }
            wrong => panic!("{:?}", wrong),
        }
    }

    test_string("\"a\"", "a");
    test_string("\"\\n\"", "\n");
    test_string("\"\\r\"", "\r");
    test_string("\"\\t\"", "\t");
    test_string("\"🐕\"", "🐕"); // NOTE: This is an emoji
    test_string("\"\\\"\"", "\"");
    test_string("\"'\"", "'");
    test_string("\"\"", "");
    test_string("\"\\u{1F415}\"", "\u{1F415}");
    test_string(
        "\"contains\nnewlines\\\nescaped newlines\"",
        "contains\nnewlinesescaped newlines",
    );
    test_string("r\"raw\nstring\\\nhere\"", "raw\nstring\\\nhere");
}

#[test]
fn byte_strings() {
    fn test_byte_string(s: &str, value: &[u8]) {
        match lit(s) {
            Lit::ByteStr(lit) => {
                assert_eq!(lit.value(), value);
                let again = lit.into_token_stream().to_string();
                if again != s {
                    test_byte_string(&again, value);
                }
            }
            wrong => panic!("{:?}", wrong),
        }
    }

    test_byte_string("b\"a\"", b"a");
    test_byte_string("b\"\\n\"", b"\n");
    test_byte_string("b\"\\r\"", b"\r");
    test_byte_string("b\"\\t\"", b"\t");
    test_byte_string("b\"\\\"\"", b"\"");
    test_byte_string("b\"'\"", b"'");
    test_byte_string("b\"\"", b"");
    test_byte_string(
        "b\"contains\nnewlines\\\nescaped newlines\"",
        b"contains\nnewlinesescaped newlines",
    );
    test_byte_string("br\"raw\nstring\\\nhere\"", b"raw\nstring\\\nhere");
}

#[test]
fn bytes() {
    fn test_byte(s: &str, value: u8) {
        match lit(s) {
            Lit::Byte(lit) => {
                assert_eq!(lit.value(), value);
                let again = lit.into_token_stream().to_string();
                assert_eq!(again, s);
            }
            wrong => panic!("{:?}", wrong),
        }
    }

    test_byte("b'a'", b'a');
    test_byte("b'\\n'", b'\n');
    test_byte("b'\\r'", b'\r');
    test_byte("b'\\t'", b'\t');
    test_byte("b'\\''", b'\'');
    test_byte("b'\"'", b'"');
}

#[test]
fn chars() {
    fn test_char(s: &str, value: char) {
        match lit(s) {
            Lit::Char(lit) => {
                assert_eq!(lit.value(), value);
                let again = lit.into_token_stream().to_string();
                if again != s {
                    test_char(&again, value);
                }
            }
            wrong => panic!("{:?}", wrong),
        }
    }

    test_char("'a'", 'a');
    test_char("'\\n'", '\n');
    test_char("'\\r'", '\r');
    test_char("'\\t'", '\t');
    test_char("'🐕'", '🐕'); // NOTE: This is an emoji
    test_char("'\\''", '\'');
    test_char("'\"'", '"');
    test_char("'\\u{1F415}'", '\u{1F415}');
}

#[test]
fn ints() {
    fn test_int(s: &str, value: u64, suffix: IntSuffix) {
        match lit(s) {
            Lit::Int(lit) => {
                assert_eq!(lit.value(), value);
                assert_eq!(lit.suffix(), suffix);
                let again = lit.into_token_stream().to_string();
                if again != s {
                    test_int(&again, value, suffix);
                }
            }
            wrong => panic!("{:?}", wrong),
        }
    }

    use syn::IntSuffix::*;
    test_int("5", 5, None);
    test_int("5u32", 5, U32);
    test_int("5_0", 50, None);
    test_int("5_____0_____", 50, None);
    test_int("0x7f", 127, None);
    test_int("0x7F", 127, None);
    test_int("0b1001", 9, None);
    test_int("0o73", 59, None);
    test_int("0x7Fu8", 127, U8);
    test_int("0b1001i8", 9, I8);
    test_int("0o73u32", 59, U32);
    test_int("0x__7___f_", 127, None);
    test_int("0x__7___F_", 127, None);
    test_int("0b_1_0__01", 9, None);
    test_int("0o_7__3", 59, None);
    test_int("0x_7F__u8", 127, U8);
    test_int("0b__10__0_1i8", 9, I8);
    test_int("0o__7__________________3u32", 59, U32);
}

#[test]
fn floats() {
    #[cfg_attr(feature = "cargo-clippy", allow(float_cmp))]
    fn test_float(s: &str, value: f64, suffix: FloatSuffix) {
        match lit(s) {
            Lit::Float(lit) => {
                assert_eq!(lit.value(), value);
                assert_eq!(lit.suffix(), suffix);
                let again = lit.into_token_stream().to_string();
                if again != s {
                    test_float(&again, value, suffix);
                }
            }
            wrong => panic!("{:?}", wrong),
        }
    }

    use syn::FloatSuffix::*;
    test_float("5.5", 5.5, None);
    test_float("5.5E12", 5.5e12, None);
    test_float("5.5e12", 5.5e12, None);
    test_float("1.0__3e-12", 1.03e-12, None);
    test_float("1.03e+12", 1.03e12, None);
}
