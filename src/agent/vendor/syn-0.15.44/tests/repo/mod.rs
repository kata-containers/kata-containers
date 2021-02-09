extern crate walkdir;

use std::process::Command;

use self::walkdir::DirEntry;

pub fn base_dir_filter(entry: &DirEntry) -> bool {
    let path = entry.path();
    if path.is_dir() {
        return true; // otherwise walkdir does not visit the files
    }
    if path.extension().map(|e| e != "rs").unwrap_or(true) {
        return false;
    }
    let path_string = path.to_string_lossy();
    let path_string = if cfg!(windows) {
        path_string.replace('\\', "/").into()
    } else {
        path_string
    };
    // TODO assert that parsing fails on the parse-fail cases
    if path_string.starts_with("tests/rust/src/test/parse-fail")
        || path_string.starts_with("tests/rust/src/test/compile-fail")
        || path_string.starts_with("tests/rust/src/test/rustfix")
    {
        return false;
    }

    if path_string.starts_with("tests/rust/src/test/ui") {
        let stderr_path = path.with_extension("stderr");
        if stderr_path.exists() {
            // Expected to fail in some way
            return false;
        }
    }

    match path_string.as_ref() {
        // Deprecated placement syntax
        "tests/rust/src/test/run-pass/new-box-syntax.rs" |
        "tests/rust/src/test/ui/obsolete-in-place/bad.rs" |
        // 2015-style dyn that libsyntax rejects
        "tests/rust/src/test/ui/dyn-keyword/dyn-2015-no-warnings-without-lints.rs" |
        // not actually test cases
        "tests/rust/src/test/run-pass/macros/auxiliary/macro-comma-support.rs" |
        "tests/rust/src/test/run-pass/macros/auxiliary/macro-include-items-expr.rs" |
        "tests/rust/src/test/ui/issues/auxiliary/issue-21146-inc.rs" => false,
        _ => true,
    }
}

pub fn clone_rust() {
    let result = Command::new("tests/clone.sh").status().unwrap();
    assert!(result.success());
}
