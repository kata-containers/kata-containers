/// Formats a slice to a colon-separated hex string (for ex `01:02:ff:ff`)
pub fn format_serial(i: &[u8]) -> String {
    let mut s = i.iter().fold(String::with_capacity(3 * i.len()), |a, b| {
        a + &format!("{:02x}:", b)
    });
    s.pop();
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_serial() {
        let b: &[u8] = &[1, 2, 3, 4, 0xff];
        assert_eq!("01:02:03:04:ff", format_serial(b));
    }
}
