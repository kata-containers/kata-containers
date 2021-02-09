//! Utility functions for working with identifiers.

use heck::{CamelCase, SnakeCase};

/// Converts a `camelCase` or `SCREAMING_SNAKE_CASE` identifier to a `lower_snake` case Rust field
/// identifier.
pub fn to_snake(s: &str) -> String {
    let mut ident = s.to_snake_case();

    // Use a raw identifier if the identifier matches a Rust keyword:
    // https://doc.rust-lang.org/reference/keywords.html.
    match ident.as_str() {
        // 2015 strict keywords.
        | "as" | "break" | "const" | "continue" | "crate" | "else" | "enum" | "extern" | "false"
        | "fn" | "for" | "if" | "impl" | "in" | "let" | "loop" | "match" | "mod" | "move" | "mut"
        | "pub" | "ref" | "return" | "self" | "static" | "struct" | "super" | "trait" | "true"
        | "type" | "unsafe" | "use" | "where" | "while"
        // 2018 strict keywords.
        | "dyn"
        // 2015 reserved keywords.
        | "abstract" | "become" | "box" | "do" | "final" | "macro" | "override" | "priv" | "typeof"
        | "unsized" | "virtual" | "yield"
        // 2018 reserved keywords.
        | "async" | "await" | "try" => ident.insert_str(0, "r#"),
        _ => (),
    }
    ident
}

/// Converts a `snake_case` identifier to an `UpperCamel` case Rust type identifier.
pub fn to_upper_camel(s: &str) -> String {
    let mut ident = s.to_camel_case();

    // Use a raw identifier if the identifier matches a Rust keyword:
    // https://doc.rust-lang.org/reference/keywords.html.
    if ident == "Self" {
        ident.insert_str(0, "r#");
    }
    ident
}

/// Matches a 'matcher' against a fully qualified identifier.
pub fn match_ident(matcher: &str, msg: &str, field: Option<&str>) -> bool {
    assert_eq!(b'.', msg.as_bytes()[0]);

    if matcher.is_empty() {
        return false;
    } else if matcher == "." {
        return true;
    }

    let match_paths = matcher.split('.').collect::<Vec<_>>();
    let field_paths = {
        let mut paths = msg.split('.').collect::<Vec<_>>();
        if let Some(field) = field {
            paths.push(field);
        }
        paths
    };

    if &matcher[..1] == "." {
        // Prefix match.
        if match_paths.len() > field_paths.len() {
            false
        } else {
            match_paths[..] == field_paths[..match_paths.len()]
        }
    // Suffix match.
    } else if match_paths.len() > field_paths.len() {
        false
    } else {
        match_paths[..] == field_paths[field_paths.len() - match_paths.len()..]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_snake() {
        assert_eq!("foo_bar", &to_snake("FooBar"));
        assert_eq!("foo_bar_baz", &to_snake("FooBarBAZ"));
        assert_eq!("foo_bar_baz", &to_snake("FooBarBAZ"));
        assert_eq!("xml_http_request", &to_snake("XMLHttpRequest"));
        assert_eq!("r#while", &to_snake("While"));
        assert_eq!("fuzz_buster", &to_snake("FUZZ_BUSTER"));
        assert_eq!("foo_bar_baz", &to_snake("foo_bar_baz"));
        assert_eq!("fuzz_buster", &to_snake("FUZZ_buster"));
        assert_eq!("fuzz", &to_snake("_FUZZ"));
        assert_eq!("fuzz", &to_snake("_fuzz"));
        assert_eq!("fuzz", &to_snake("_Fuzz"));
        assert_eq!("fuzz", &to_snake("FUZZ_"));
        assert_eq!("fuzz", &to_snake("fuzz_"));
        assert_eq!("fuzz", &to_snake("Fuzz_"));
        assert_eq!("fuz_z", &to_snake("FuzZ_"));

        // From test_messages_proto3.proto.
        assert_eq!("fieldname1", &to_snake("fieldname1"));
        assert_eq!("field_name2", &to_snake("field_name2"));
        assert_eq!("field_name3", &to_snake("_field_name3"));
        assert_eq!("field_name4", &to_snake("field__name4_"));
        assert_eq!("field0name5", &to_snake("field0name5"));
        assert_eq!("field_0_name6", &to_snake("field_0_name6"));
        assert_eq!("field_name7", &to_snake("fieldName7"));
        assert_eq!("field_name8", &to_snake("FieldName8"));
        assert_eq!("field_name9", &to_snake("field_Name9"));
        assert_eq!("field_name10", &to_snake("Field_Name10"));

        // TODO(withoutboats/heck#3)
        //assert_eq!("field_name11", &to_snake("FIELD_NAME11"));
        assert_eq!("field_name12", &to_snake("FIELD_name12"));
        assert_eq!("field_name13", &to_snake("__field_name13"));
        assert_eq!("field_name14", &to_snake("__Field_name14"));
        assert_eq!("field_name15", &to_snake("field__name15"));
        assert_eq!("field_name16", &to_snake("field__Name16"));
        assert_eq!("field_name17", &to_snake("field_name17__"));
        assert_eq!("field_name18", &to_snake("Field_name18__"));
    }

    #[test]
    fn test_to_upper_camel() {
        assert_eq!("", &to_upper_camel(""));
        assert_eq!("F", &to_upper_camel("F"));
        assert_eq!("Foo", &to_upper_camel("FOO"));
        assert_eq!("FooBar", &to_upper_camel("FOO_BAR"));
        assert_eq!("FooBar", &to_upper_camel("_FOO_BAR"));
        assert_eq!("FooBar", &to_upper_camel("FOO_BAR_"));
        assert_eq!("FooBar", &to_upper_camel("_FOO_BAR_"));
        assert_eq!("FuzzBuster", &to_upper_camel("fuzzBuster"));
        assert_eq!("FuzzBuster", &to_upper_camel("FuzzBuster"));
        assert_eq!("r#Self", &to_upper_camel("self"));
    }

    #[test]
    fn test_match_ident() {
        // Prefix matches
        assert!(match_ident(".", ".foo.bar.Baz", Some("buzz")));
        assert!(match_ident(".foo", ".foo.bar.Baz", Some("buzz")));
        assert!(match_ident(".foo.bar", ".foo.bar.Baz", Some("buzz")));
        assert!(match_ident(".foo.bar.Baz", ".foo.bar.Baz", Some("buzz")));
        assert!(match_ident(
            ".foo.bar.Baz.buzz",
            ".foo.bar.Baz",
            Some("buzz")
        ));

        assert!(!match_ident(".fo", ".foo.bar.Baz", Some("buzz")));
        assert!(!match_ident(".foo.", ".foo.bar.Baz", Some("buzz")));
        assert!(!match_ident(".buzz", ".foo.bar.Baz", Some("buzz")));
        assert!(!match_ident(".Baz.buzz", ".foo.bar.Baz", Some("buzz")));

        // Suffix matches
        assert!(match_ident("buzz", ".foo.bar.Baz", Some("buzz")));
        assert!(match_ident("Baz.buzz", ".foo.bar.Baz", Some("buzz")));
        assert!(match_ident("bar.Baz.buzz", ".foo.bar.Baz", Some("buzz")));
        assert!(match_ident(
            "foo.bar.Baz.buzz",
            ".foo.bar.Baz",
            Some("buzz")
        ));

        assert!(!match_ident("buz", ".foo.bar.Baz", Some("buzz")));
        assert!(!match_ident("uz", ".foo.bar.Baz", Some("buzz")));

        // Type names
        assert!(match_ident("Baz", ".foo.bar.Baz", None));
        assert!(match_ident(".", ".foo.bar.Baz", None));
        assert!(match_ident(".foo.bar", ".foo.bar.Baz", None));
        assert!(match_ident(".foo.bar.Baz", ".foo.bar.Baz", None));
        assert!(!match_ident(".fo", ".foo.bar.Baz", None));
        assert!(!match_ident(".buzz.Baz", ".foo.bar.Baz", None));
    }
}
