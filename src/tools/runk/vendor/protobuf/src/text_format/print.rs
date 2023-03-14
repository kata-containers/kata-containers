#[doc(hidden)]
pub fn quote_bytes_to(bytes: &[u8], buf: &mut String) {
    for &c in bytes {
        match c {
            b'\n' => buf.push_str(r"\n"),
            b'\r' => buf.push_str(r"\r"),
            b'\t' => buf.push_str(r"\t"),
            b'\'' => buf.push_str("\\\'"),
            b'"' => buf.push_str("\\\""),
            b'\\' => buf.push_str(r"\\"),
            b'\x20'..=b'\x7e' => buf.push(c as char),
            _ => {
                buf.push('\\');
                buf.push((b'0' + (c >> 6)) as char);
                buf.push((b'0' + ((c >> 3) & 7)) as char);
                buf.push((b'0' + (c & 7)) as char);
            }
        }
    }
}

pub(crate) fn quote_escape_bytes_to(bytes: &[u8], buf: &mut String) {
    buf.push('"');
    quote_bytes_to(bytes, buf);
    buf.push('"');
}

#[doc(hidden)]
pub fn quote_escape_bytes(bytes: &[u8]) -> String {
    let mut r = String::new();
    quote_escape_bytes_to(bytes, &mut r);
    r
}

pub(crate) fn print_str_to(s: &str, buf: &mut String) {
    // TODO: keep printable Unicode
    quote_escape_bytes_to(s.as_bytes(), buf);
}
