pub fn remove_to<'s, P>(s: &'s str, pattern: P) -> &'s str
where
    P: Fn(char) -> bool,
{
    match s.rfind(pattern) {
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

pub fn capitalize(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    let mut char_indices = s.char_indices();
    char_indices.next().unwrap();
    match char_indices.next() {
        None => s.to_uppercase(),
        Some((i, _)) => s[..i].to_uppercase() + &s[i..],
    }
}

#[cfg(test)]
mod test {

    use super::capitalize;
    use super::remove_suffix;
    use super::remove_to;

    #[test]
    fn test_remove_to() {
        assert_eq!("aaa", remove_to("aaa", |c| c == '.'));
        assert_eq!("bbb", remove_to("aaa.bbb", |c| c == '.'));
        assert_eq!("ccc", remove_to("aaa.bbb.ccc", |c| c == '.'));
    }

    #[test]
    fn test_remove_suffix() {
        assert_eq!("bbb", remove_suffix("bbbaaa", "aaa"));
        assert_eq!("aaa", remove_suffix("aaa", "bbb"));
    }

    #[test]
    fn test_capitalize() {
        assert_eq!("", capitalize(""));
        assert_eq!("F", capitalize("f"));
        assert_eq!("Foo", capitalize("foo"));
    }
}
