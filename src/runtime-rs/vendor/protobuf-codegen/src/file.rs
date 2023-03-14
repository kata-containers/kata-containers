use crate::rust;
use crate::rust_name::RustIdent;
use crate::strx;

// Copy-pasted from libsyntax.
fn ident_start(c: char) -> bool {
    (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || c == '_'
}

// Copy-pasted from libsyntax.
fn ident_continue(c: char) -> bool {
    (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || c == '_'
}

pub(crate) fn proto_path_to_rust_mod(path: &str) -> RustIdent {
    let without_dir = strx::remove_to(path, std::path::is_separator);
    let without_suffix = strx::remove_suffix(without_dir, ".proto");

    let name = without_suffix
        .chars()
        .enumerate()
        .map(|(i, c)| {
            let valid = if i == 0 {
                ident_start(c)
            } else {
                ident_continue(c)
            };
            if valid {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();

    let name = if rust::is_rust_keyword(&name) {
        format!("{}_pb", name)
    } else {
        name
    };
    RustIdent::from(name)
}

#[cfg(test)]
mod test {

    use super::proto_path_to_rust_mod;
    use crate::rust_name::RustIdent;

    #[test]
    fn test_mod_path_proto_ext() {
        assert_eq!(
            RustIdent::from("proto"),
            proto_path_to_rust_mod("proto.proto")
        );
    }

    #[test]
    fn test_mod_path_unknown_ext() {
        assert_eq!(
            RustIdent::from("proto_proto3"),
            proto_path_to_rust_mod("proto.proto3")
        );
    }

    #[test]
    fn test_mod_path_empty_ext() {
        assert_eq!(RustIdent::from("proto"), proto_path_to_rust_mod("proto"));
    }

    #[test]
    fn test_mod_path_dir() {
        assert_eq!(
            RustIdent::from("baz"),
            proto_path_to_rust_mod("foo/bar/baz.proto"),
        )
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_mod_path_dir_backslashes() {
        assert_eq!(
            RustIdent::from("baz"),
            proto_path_to_rust_mod("foo\\bar\\baz.proto"),
        )
    }
}
