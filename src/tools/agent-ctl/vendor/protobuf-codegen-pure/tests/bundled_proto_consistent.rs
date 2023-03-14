use std::fs;
use std::path::Path;
use std::path::PathBuf;

fn list_dir(p: &Path) -> Vec<PathBuf> {
    let mut children = fs::read_dir(p)
        .unwrap()
        .map(|r| r.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    children.sort();
    children
}

fn assert_equal_recursively(a: &Path, b: &Path) {
    assert_eq!(a.is_dir(), b.is_dir());
    assert_eq!(a.is_file(), b.is_file());
    if a.is_dir() {
        let mut a_contents = list_dir(a).into_iter();
        let mut b_contents = list_dir(b).into_iter();
        loop {
            let a_child = a_contents.next();
            let b_child = b_contents.next();
            match (a_child, b_child) {
                (Some(a_child), Some(b_child)) => {
                    assert_eq!(a_child.file_name(), b_child.file_name());
                    assert_equal_recursively(&a_child, &b_child);
                }
                (None, None) => break,
                _ => panic!(
                    "mismatched directories: {} and {}",
                    a.display(),
                    b.display()
                ),
            }
        }
    } else {
        let a_contents = fs::read(a).unwrap();
        let b_contents = fs::read(b).unwrap();
        assert_eq!(a_contents, b_contents);
    }
}

#[test]
fn test_bundled_google_proto_files_consistent() {
    let source = "../protoc-bin-vendored/include/google";
    let our_copy = "src/proto/google";
    assert_equal_recursively(Path::new(source), Path::new(our_copy));
}

#[test]
fn test_bundled_rustproto_proto_consistent() {
    let source = "../proto/rustproto.proto";
    let our_copy = "src/proto/rustproto.proto";
    assert_equal_recursively(Path::new(source), Path::new(our_copy));
}
