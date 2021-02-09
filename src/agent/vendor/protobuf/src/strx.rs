pub fn remove_to<'s>(s: &'s str, c: char) -> &'s str {
    match s.rfind(c) {
        Some(pos) => &s[(pos + 1)..],
        None => s,
    }
}

pub fn remove_suffix<'s>(s: &'s str, suffix: &str) -> &'s str {
    if !s.ends_with(suffix) {
        s
    } else {
        &s[..(s.len() - suffix.len())]
    }
}

#[cfg(test)]
mod test {

    use super::remove_suffix;
    use super::remove_to;

    #[test]
    fn test_remove_to() {
        assert_eq!("aaa", remove_to("aaa", '.'));
        assert_eq!("bbb", remove_to("aaa.bbb", '.'));
        assert_eq!("ccc", remove_to("aaa.bbb.ccc", '.'));
    }

    #[test]
    fn test_remove_suffix() {
        assert_eq!("bbb", remove_suffix("bbbaaa", "aaa"));
        assert_eq!("aaa", remove_suffix("aaa", "bbb"));
    }
}
