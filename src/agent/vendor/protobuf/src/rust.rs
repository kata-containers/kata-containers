#[cfg_attr(rustfmt, rustfmt_skip)]
static RUST_KEYWORDS: &'static [&'static str] = &[
    "as",
    "async",
    "await",
    "break",
    "crate",
    "dyn",
    "else",
    "enum",
    "extern",
    "false",
    "fn",
    "for",
    "if",
    "impl",
    "in",
    "let",
    "loop",
    "match",
    "mod",
    "move",
    "mut",
    "pub",
    "ref",
    "return",
    "static",
    "self",
    "Self",
    "struct",
    "super",
    "true",
    "trait",
    "type",
    "unsafe",
    "use",
    "while",
    "continue",
    "box",
    "const",
    "where",
    "virtual",
    "proc",
    "alignof",
    "become",
    "offsetof",
    "priv",
    "pure",
    "sizeof",
    "typeof",
    "unsized",
    "yield",
    "do",
    "abstract",
    "final",
    "override",
    "macro",
];

pub fn is_rust_keyword(ident: &str) -> bool {
    RUST_KEYWORDS.contains(&ident)
}

fn hex_digit(value: u32) -> char {
    if value < 10 {
        (b'0' + value as u8) as char
    } else if value < 0x10 {
        (b'a' + value as u8 - 10) as char
    } else {
        unreachable!()
    }
}

pub fn quote_escape_str(s: &str) -> String {
    let mut buf = String::new();
    buf.push('"');
    buf.extend(s.chars().flat_map(|c| c.escape_default()));
    buf.push('"');
    buf
}

pub fn quote_escape_bytes(bytes: &[u8]) -> String {
    let mut buf = String::new();
    buf.push('b');
    buf.push('"');
    for &b in bytes {
        match b {
            b'\n' => buf.push_str(r"\n"),
            b'\r' => buf.push_str(r"\r"),
            b'\t' => buf.push_str(r"\t"),
            b'"' => buf.push_str("\\\""),
            b'\\' => buf.push_str(r"\\"),
            b'\x20'..=b'\x7e' => buf.push(b as char),
            _ => {
                buf.push_str(r"\x");
                buf.push(hex_digit((b as u32) >> 4));
                buf.push(hex_digit((b as u32) & 0x0f));
            }
        }
    }
    buf.push('"');
    buf
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_quote_escape_bytes() {
        assert_eq!("b\"\"", quote_escape_bytes(b""));
        assert_eq!("b\"xyZW\"", quote_escape_bytes(b"xyZW"));
        assert_eq!("b\"aa\\\"bb\"", quote_escape_bytes(b"aa\"bb"));
        assert_eq!("b\"aa\\r\\n\\tbb\"", quote_escape_bytes(b"aa\r\n\tbb"));
        assert_eq!(
            "b\"\\x00\\x01\\x12\\xfe\\xff\"",
            quote_escape_bytes(b"\x00\x01\x12\xfe\xff")
        );
    }
}
