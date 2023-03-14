#![cfg(not(windows))]

extern crate path_absolutize;

use std::env;
use std::io::ErrorKind;
use std::path::Path;

use path_absolutize::{update_cwd, Absolutize};

#[test]
fn absolutize_lv0_1() {
    let p = Path::new("/path/to/123/456");

    assert_eq!("/path/to/123/456", p.absolutize().unwrap().to_str().unwrap());
}

#[test]
fn absolutize_lv0_2() {
    let p = Path::new("/path/to/./123/../456");

    assert_eq!("/path/to/456", p.absolutize().unwrap().to_str().unwrap());
}

#[test]
fn absolutize_lv1_1() {
    let p = Path::new("./path/to/123/456");

    assert_eq!(
        Path::join(env::current_dir().unwrap().as_path(), Path::new("path/to/123/456"))
            .to_str()
            .unwrap(),
        p.absolutize().unwrap().to_str().unwrap()
    );
}

#[test]
fn absolutize_lv1_2() {
    let p = Path::new("../path/to/123/456");

    let cwd = env::current_dir().unwrap();

    let cwd_parent = cwd.parent();

    match cwd_parent {
        Some(cwd_parent) => {
            assert_eq!(
                Path::join(&cwd_parent, Path::new("path/to/123/456")).to_str().unwrap(),
                p.absolutize().unwrap().to_str().unwrap()
            );
        }
        None => {
            assert_eq!(
                Path::join(Path::new("/"), Path::new("path/to/123/456")).to_str().unwrap(),
                p.absolutize().unwrap().to_str().unwrap()
            );
        }
    }
}

#[test]
fn absolutize_lv2() {
    let p = Path::new("path/to/123/456");

    assert_eq!(
        Path::join(env::current_dir().unwrap().as_path(), Path::new("path/to/123/456"))
            .to_str()
            .unwrap(),
        p.absolutize().unwrap().to_str().unwrap()
    );
}

#[test]
fn absolutize_lv3() {
    let p = Path::new("path/../../to/123/456");

    let cwd = env::current_dir().unwrap();

    let cwd_parent = cwd.parent();

    match cwd_parent {
        Some(cwd_parent) => {
            assert_eq!(
                Path::join(&cwd_parent, Path::new("to/123/456")).to_str().unwrap(),
                p.absolutize().unwrap().to_str().unwrap()
            );
        }
        None => {
            assert_eq!(
                Path::join(Path::new("/"), Path::new("to/123/456")).to_str().unwrap(),
                p.absolutize().unwrap().to_str().unwrap()
            );
        }
    }
}

#[ignore]
#[test]
fn absolutize_after_updating_cwd() {
    let p = Path::new("path/to/123/456");

    assert_eq!(
        Path::join(env::current_dir().unwrap().as_path(), Path::new("path/to/123/456"))
            .to_str()
            .unwrap(),
        p.absolutize().unwrap().to_str().unwrap()
    );

    env::set_current_dir("/").unwrap();

    unsafe {
        update_cwd();
    }

    assert_eq!(
        Path::join(env::current_dir().unwrap().as_path(), Path::new("path/to/123/456"))
            .to_str()
            .unwrap(),
        p.absolutize().unwrap().to_str().unwrap()
    );
}

#[test]
fn virtually_absolutize_lv0_1() {
    let p = Path::new("/path/to/123/456");

    assert_eq!("/path/to/123/456", p.absolutize_virtually("/").unwrap().to_str().unwrap());
}

#[test]
fn virtually_absolutize_lv0_2() {
    let p = Path::new("/path/to/./123/../456");

    assert_eq!("/path/to/456", p.absolutize_virtually("/").unwrap().to_str().unwrap());
}

#[test]
fn virtually_absolutize_lv0_3() {
    let p = Path::new("/path/to/123/456");

    assert_eq!(
        ErrorKind::InvalidInput,
        p.absolutize_virtually("/virtual/root").unwrap_err().kind()
    );
}

#[test]
fn virtually_absolutize_lv1_1() {
    let p = Path::new("./path/to/123/456");

    assert_eq!(
        ErrorKind::InvalidInput,
        p.absolutize_virtually("/virtual/root").unwrap_err().kind()
    );
}

#[test]
fn virtually_absolutize_lv1_2() {
    let p = Path::new("../path/to/123/456");

    assert_eq!(
        ErrorKind::InvalidInput,
        p.absolutize_virtually("/virtual/root").unwrap_err().kind()
    );
}

#[test]
fn virtually_absolutize_lv2() {
    let p = Path::new("path/to/123/456");

    assert_eq!(
        "/virtual/root/path/to/123/456",
        p.absolutize_virtually("/virtual/root").unwrap().to_str().unwrap()
    );
}

#[test]
fn virtually_absolutize_lv3() {
    let p = Path::new("path/to/../../../../123/456");

    assert_eq!(
        "/virtual/root/123/456",
        p.absolutize_virtually("/virtual/root").unwrap().to_str().unwrap()
    );
}
