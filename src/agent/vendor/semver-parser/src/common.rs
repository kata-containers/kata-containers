use version::Identifier;
use recognize::{Recognize, Alt, OneOrMore, Inclusive, OneByte};
use std::str::from_utf8;

// by the time we get here, we know that it's all valid characters, so this doesn't need to return
// a result or anything
fn parse_meta(s: &str) -> Vec<Identifier> {
    // Originally, I wanted to implement this method via calling parse, but parse is tolerant of
    // leading zeroes, and we want anything with leading zeroes to be considered alphanumeric, not
    // numeric. So the strategy is to check with a recognizer first, and then call parse once
    // we've determined that it's a number without a leading zero.
    s.split(".")
        .map(|part| {
            // another wrinkle: we made sure that any number starts with a
            // non-zero. But there's a problem: an actual zero is a number, yet
            // gets left out by this heuristic. So let's also check for the
            // single, lone zero.
            if is_alpha_numeric(part) {
                Identifier::AlphaNumeric(part.to_string())
            } else {
                // we can unwrap here because we know it is only digits due to the regex
                Identifier::Numeric(part.parse().unwrap())
            }
        }).collect()
}

// parse optional metadata (preceded by the prefix character)
pub fn parse_optional_meta(s: &[u8], prefix_char: u8)-> Result<(Vec<Identifier>, usize), String> {
    if let Some(len) = prefix_char.p(s) {
        let start = len;
        if let Some(len) = letters_numbers_dash_dot(&s[start..]) {
            let end = start + len;
            Ok((parse_meta(from_utf8(&s[start..end]).unwrap()), end))
        } else {
            Err("Error parsing prerelease".to_string())
        }
    } else {
        Ok((Vec::new(), 0))
    }
}

pub fn is_alpha_numeric(s: &str) -> bool {
    if let Some((_val, len)) = numeric_identifier(s.as_bytes()) {
        // Return true for number with leading zero
        // Note: doing it this way also handily makes overflow fail over.
        len != s.len()
    } else {
        true
    }
}

// Note: could plumb overflow error up to return value as Result
pub fn numeric_identifier(s: &[u8]) -> Option<(u64, usize)> {
    if let Some(len) = Alt(b'0', OneOrMore(Inclusive(b'0'..b'9'))).p(s) {
        from_utf8(&s[0..len]).unwrap().parse().ok().map(|val| (val, len))
    } else {
        None
    }
}

pub fn letters_numbers_dash_dot(s: &[u8]) -> Option<usize> {
    OneOrMore(OneByte(|c| c == b'-' || c == b'.' ||
        (b'0' <= c && c <= b'9') ||
        (b'a' <= c && c <= b'z') ||
        (b'A' <= c && c <= b'Z'))).p(s)
}
