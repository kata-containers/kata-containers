#![cfg(windows)]

extern crate path_dedot;

use std::env;
use std::path::Path;

use path_dedot::{update_cwd, ParseDot, ParsePrefix};

#[test]
fn dedot_lv0_1() {
    let p = Path::new(r".\path\to\123\456");

    assert_eq!(
        Path::join(env::current_dir().unwrap().as_path(), Path::new(r"path\to\123\456"))
            .to_str()
            .unwrap(),
        p.parse_dot().unwrap().to_str().unwrap()
    );
}

#[test]
fn dedot_lv0_2() {
    let p = Path::new(r"..\path\to\123\456");

    let cwd = env::current_dir().unwrap();

    let cwd_parent = cwd.parent();

    match cwd_parent {
        Some(cwd_parent) => {
            assert_eq!(
                Path::join(&cwd_parent, Path::new(r"path\to\123\456")).to_str().unwrap(),
                p.parse_dot().unwrap().to_str().unwrap()
            );
        }
        None => {
            assert_eq!(
                Path::join(
                    Path::new(cwd.get_path_prefix().unwrap().as_os_str()),
                    Path::new(r"\path\to\123\456"),
                )
                .to_str()
                .unwrap(),
                p.parse_dot().unwrap().to_str().unwrap()
            );
        }
    }
}

#[test]
#[ignore]
// Ignored because it may not be standard
fn dedot_lv0_3() {
    let cwd = env::current_dir().unwrap();

    let prefix = cwd.get_path_prefix().unwrap();

    let p = Path::join(Path::new(prefix.as_os_str()), Path::new(r".\path\to\123\456"));

    assert_eq!(
        Path::join(&cwd, Path::new(r"path\to\123\456")).to_str().unwrap(),
        p.parse_dot().unwrap().to_str().unwrap()
    );
}

#[test]
#[ignore]
// Ignored because it may not be standard
fn dedot_lv0_4() {
    let cwd = env::current_dir().unwrap();

    let prefix = cwd.get_path_prefix().unwrap();

    let p = Path::join(Path::new(prefix.as_os_str()), Path::new(r"..\path\to\123\456"));

    let cwd_parent = cwd.parent();

    match cwd_parent {
        Some(cwd_parent) => {
            assert_eq!(
                Path::join(&cwd_parent, Path::new(r"path\to\123\456")).to_str().unwrap(),
                p.parse_dot().unwrap().to_str().unwrap()
            );
        }
        None => {
            assert_eq!(
                Path::join(
                    Path::new(cwd.get_path_prefix().unwrap().as_os_str()),
                    Path::new(r"\path\to\123\456"),
                )
                .to_str()
                .unwrap(),
                p.parse_dot().unwrap().to_str().unwrap()
            );
        }
    }
}

#[test]
fn dedot_lv1_1() {
    let p = Path::new(r"\path\to\..\123\456\.\777");

    assert_eq!(r"\path\123\456\777", p.parse_dot().unwrap().to_str().unwrap());
}

#[test]
fn dedot_lv1_2() {
    let p = Path::new(r"C:\path\to\..\123\456\.\777");

    assert_eq!(r"C:\path\123\456\777", p.parse_dot().unwrap().to_str().unwrap());
}

#[test]
fn dedot_lv2_1() {
    let p = Path::new(r"\path\to\..\123\456\.\777\..");

    assert_eq!(r"\path\123\456", p.parse_dot().unwrap().to_str().unwrap());
}

#[test]
fn dedot_lv2_2() {
    let p = Path::new(r"C:\path\to\..\123\456\.\777\..");

    assert_eq!(r"C:\path\123\456", p.parse_dot().unwrap().to_str().unwrap());
}

#[test]
fn dedot_lv3_1() {
    let p = Path::new(r"path\to\..\123\456\.\777\..");

    assert_eq!(r"path\123\456", p.parse_dot().unwrap().to_str().unwrap());
}

#[test]
fn dedot_lv3_2() {
    let p = Path::new(r"C:path\to\..\123\456\.\777\..");

    assert_eq!(r"C:path\123\456", p.parse_dot().unwrap().to_str().unwrap());
}

#[test]
fn dedot_lv4_1() {
    let p = Path::new(r"path\to\..\..\..\..\123\456\.\777\..");

    assert_eq!(r"123\456", p.parse_dot().unwrap().to_str().unwrap());
}

#[test]
fn dedot_lv4_2() {
    let p = Path::new(r"C:path\to\..\..\..\..\123\456\.\777\..");

    assert_eq!(r"C:123\456", p.parse_dot().unwrap().to_str().unwrap());
}

#[test]
fn dedot_lv5_1() {
    let p = Path::new(r"\path\to\..\..\..\..\123\456\.\777\..");

    assert_eq!(r"\123\456", p.parse_dot().unwrap().to_str().unwrap());
}

#[test]
fn dedot_lv5_2() {
    let p = Path::new(r"C:\path\to\..\..\..\..\123\456\.\777\..");

    assert_eq!(r"C:\123\456", p.parse_dot().unwrap().to_str().unwrap());
}

#[ignore]
#[test]
fn dedot_after_updating_cwd() {
    let p = Path::new(r".\path\to\123\456");

    assert_eq!(
        Path::join(env::current_dir().unwrap().as_path(), Path::new(r"path\to\123\456"))
            .to_str()
            .unwrap(),
        p.parse_dot().unwrap().to_str().unwrap()
    );

    let cwd = env::current_dir().unwrap();

    let prefix = cwd.get_path_prefix().unwrap();

    env::set_current_dir(Path::new(prefix.as_os_str())).unwrap();

    unsafe {
        update_cwd();
    }

    assert_eq!(
        Path::join(env::current_dir().unwrap().as_path(), Path::new(r"path\to\123\456"))
            .to_str()
            .unwrap(),
        p.parse_dot().unwrap().to_str().unwrap()
    );
}

#[test]
fn prefix_1() {
    let p = Path::new(r"C:\");

    assert_eq!(r"C:\", p.parse_dot().unwrap().to_str().unwrap());
}

#[test]
fn prefix_2() {
    let p = Path::new(r"C:");

    assert_eq!(r"C:", p.parse_dot().unwrap().to_str().unwrap());
}

#[test]
fn prefix_3() {
    let p = Path::new(r"\\VBOXSRV\test");

    assert_eq!(r"\\VBOXSRV\test\", p.parse_dot().unwrap().to_str().unwrap());
}

#[test]
fn prefix_4() {
    let p = Path::new(r"\\VBOXSRV\test\");

    assert_eq!(r"\\VBOXSRV\test\", p.parse_dot().unwrap().to_str().unwrap());
}
