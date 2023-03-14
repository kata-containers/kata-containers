extern crate tar;
extern crate tempfile;

use std::fs::{create_dir, File};
use std::io::Read;

use tempfile::Builder;

macro_rules! t {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(e) => panic!("{} returned {}", stringify!($e), e),
        }
    };
}

#[test]
fn absolute_symlink() {
    let mut ar = tar::Builder::new(Vec::new());

    let mut header = tar::Header::new_gnu();
    header.set_size(0);
    header.set_entry_type(tar::EntryType::Symlink);
    t!(header.set_path("foo"));
    t!(header.set_link_name("/bar"));
    header.set_cksum();
    t!(ar.append(&header, &[][..]));

    let bytes = t!(ar.into_inner());
    let mut ar = tar::Archive::new(&bytes[..]);

    let td = t!(Builder::new().prefix("tar").tempdir());
    t!(ar.unpack(td.path()));

    t!(td.path().join("foo").symlink_metadata());

    let mut ar = tar::Archive::new(&bytes[..]);
    let mut entries = t!(ar.entries());
    let entry = t!(entries.next().unwrap());
    assert_eq!(&*entry.link_name_bytes().unwrap(), b"/bar");
}

#[test]
fn absolute_hardlink() {
    let td = t!(Builder::new().prefix("tar").tempdir());
    let mut ar = tar::Builder::new(Vec::new());

    let mut header = tar::Header::new_gnu();
    header.set_size(0);
    header.set_entry_type(tar::EntryType::Regular);
    t!(header.set_path("foo"));
    header.set_cksum();
    t!(ar.append(&header, &[][..]));

    let mut header = tar::Header::new_gnu();
    header.set_size(0);
    header.set_entry_type(tar::EntryType::Link);
    t!(header.set_path("bar"));
    // This absolute path under tempdir will be created at unpack time
    t!(header.set_link_name(td.path().join("foo")));
    header.set_cksum();
    t!(ar.append(&header, &[][..]));

    let bytes = t!(ar.into_inner());
    let mut ar = tar::Archive::new(&bytes[..]);

    t!(ar.unpack(td.path()));
    t!(td.path().join("foo").metadata());
    t!(td.path().join("bar").metadata());
}

#[test]
fn relative_hardlink() {
    let mut ar = tar::Builder::new(Vec::new());

    let mut header = tar::Header::new_gnu();
    header.set_size(0);
    header.set_entry_type(tar::EntryType::Regular);
    t!(header.set_path("foo"));
    header.set_cksum();
    t!(ar.append(&header, &[][..]));

    let mut header = tar::Header::new_gnu();
    header.set_size(0);
    header.set_entry_type(tar::EntryType::Link);
    t!(header.set_path("bar"));
    t!(header.set_link_name("foo"));
    header.set_cksum();
    t!(ar.append(&header, &[][..]));

    let bytes = t!(ar.into_inner());
    let mut ar = tar::Archive::new(&bytes[..]);

    let td = t!(Builder::new().prefix("tar").tempdir());
    t!(ar.unpack(td.path()));
    t!(td.path().join("foo").metadata());
    t!(td.path().join("bar").metadata());
}

#[test]
fn absolute_link_deref_error() {
    let mut ar = tar::Builder::new(Vec::new());

    let mut header = tar::Header::new_gnu();
    header.set_size(0);
    header.set_entry_type(tar::EntryType::Symlink);
    t!(header.set_path("foo"));
    t!(header.set_link_name("/"));
    header.set_cksum();
    t!(ar.append(&header, &[][..]));

    let mut header = tar::Header::new_gnu();
    header.set_size(0);
    header.set_entry_type(tar::EntryType::Regular);
    t!(header.set_path("foo/bar"));
    header.set_cksum();
    t!(ar.append(&header, &[][..]));

    let bytes = t!(ar.into_inner());
    let mut ar = tar::Archive::new(&bytes[..]);

    let td = t!(Builder::new().prefix("tar").tempdir());
    assert!(ar.unpack(td.path()).is_err());
    t!(td.path().join("foo").symlink_metadata());
    assert!(File::open(td.path().join("foo").join("bar")).is_err());
}

#[test]
fn relative_link_deref_error() {
    let mut ar = tar::Builder::new(Vec::new());

    let mut header = tar::Header::new_gnu();
    header.set_size(0);
    header.set_entry_type(tar::EntryType::Symlink);
    t!(header.set_path("foo"));
    t!(header.set_link_name("../../../../"));
    header.set_cksum();
    t!(ar.append(&header, &[][..]));

    let mut header = tar::Header::new_gnu();
    header.set_size(0);
    header.set_entry_type(tar::EntryType::Regular);
    t!(header.set_path("foo/bar"));
    header.set_cksum();
    t!(ar.append(&header, &[][..]));

    let bytes = t!(ar.into_inner());
    let mut ar = tar::Archive::new(&bytes[..]);

    let td = t!(Builder::new().prefix("tar").tempdir());
    assert!(ar.unpack(td.path()).is_err());
    t!(td.path().join("foo").symlink_metadata());
    assert!(File::open(td.path().join("foo").join("bar")).is_err());
}

#[test]
#[cfg(unix)]
fn directory_maintains_permissions() {
    use ::std::os::unix::fs::PermissionsExt;

    let mut ar = tar::Builder::new(Vec::new());

    let mut header = tar::Header::new_gnu();
    header.set_size(0);
    header.set_entry_type(tar::EntryType::Directory);
    t!(header.set_path("foo"));
    header.set_mode(0o777);
    header.set_cksum();
    t!(ar.append(&header, &[][..]));

    let bytes = t!(ar.into_inner());
    let mut ar = tar::Archive::new(&bytes[..]);

    let td = t!(Builder::new().prefix("tar").tempdir());
    t!(ar.unpack(td.path()));
    let f = t!(File::open(td.path().join("foo")));
    let md = t!(f.metadata());
    assert!(md.is_dir());
    assert_eq!(md.permissions().mode(), 0o40777);
}

#[test]
#[cfg(not(windows))] // dangling symlinks have weird permissions
fn modify_link_just_created() {
    let mut ar = tar::Builder::new(Vec::new());

    let mut header = tar::Header::new_gnu();
    header.set_size(0);
    header.set_entry_type(tar::EntryType::Symlink);
    t!(header.set_path("foo"));
    t!(header.set_link_name("bar"));
    header.set_cksum();
    t!(ar.append(&header, &[][..]));

    let mut header = tar::Header::new_gnu();
    header.set_size(0);
    header.set_entry_type(tar::EntryType::Regular);
    t!(header.set_path("bar/foo"));
    header.set_cksum();
    t!(ar.append(&header, &[][..]));

    let mut header = tar::Header::new_gnu();
    header.set_size(0);
    header.set_entry_type(tar::EntryType::Regular);
    t!(header.set_path("foo/bar"));
    header.set_cksum();
    t!(ar.append(&header, &[][..]));

    let bytes = t!(ar.into_inner());
    let mut ar = tar::Archive::new(&bytes[..]);

    let td = t!(Builder::new().prefix("tar").tempdir());
    t!(ar.unpack(td.path()));

    t!(File::open(td.path().join("bar/foo")));
    t!(File::open(td.path().join("bar/bar")));
    t!(File::open(td.path().join("foo/foo")));
    t!(File::open(td.path().join("foo/bar")));
}

#[test]
#[cfg(not(windows))] // dangling symlinks have weird permissions
fn modify_outside_with_relative_symlink() {
    let mut ar = tar::Builder::new(Vec::new());

    let mut header = tar::Header::new_gnu();
    header.set_size(0);
    header.set_entry_type(tar::EntryType::Symlink);
    t!(header.set_path("symlink"));
    t!(header.set_link_name(".."));
    header.set_cksum();
    t!(ar.append(&header, &[][..]));

    let mut header = tar::Header::new_gnu();
    header.set_size(0);
    header.set_entry_type(tar::EntryType::Regular);
    t!(header.set_path("symlink/foo/bar"));
    header.set_cksum();
    t!(ar.append(&header, &[][..]));

    let bytes = t!(ar.into_inner());
    let mut ar = tar::Archive::new(&bytes[..]);

    let td = t!(Builder::new().prefix("tar").tempdir());
    let tar_dir = td.path().join("tar");
    create_dir(&tar_dir).unwrap();
    assert!(ar.unpack(tar_dir).is_err());
    assert!(!td.path().join("foo").exists());
}

#[test]
fn parent_paths_error() {
    let mut ar = tar::Builder::new(Vec::new());

    let mut header = tar::Header::new_gnu();
    header.set_size(0);
    header.set_entry_type(tar::EntryType::Symlink);
    t!(header.set_path("foo"));
    t!(header.set_link_name(".."));
    header.set_cksum();
    t!(ar.append(&header, &[][..]));

    let mut header = tar::Header::new_gnu();
    header.set_size(0);
    header.set_entry_type(tar::EntryType::Regular);
    t!(header.set_path("foo/bar"));
    header.set_cksum();
    t!(ar.append(&header, &[][..]));

    let bytes = t!(ar.into_inner());
    let mut ar = tar::Archive::new(&bytes[..]);

    let td = t!(Builder::new().prefix("tar").tempdir());
    assert!(ar.unpack(td.path()).is_err());
    t!(td.path().join("foo").symlink_metadata());
    assert!(File::open(td.path().join("foo").join("bar")).is_err());
}

#[test]
#[cfg(unix)]
fn good_parent_paths_ok() {
    use std::path::PathBuf;
    let mut ar = tar::Builder::new(Vec::new());

    let mut header = tar::Header::new_gnu();
    header.set_size(0);
    header.set_entry_type(tar::EntryType::Symlink);
    t!(header.set_path(PathBuf::from("foo").join("bar")));
    t!(header.set_link_name(PathBuf::from("..").join("bar")));
    header.set_cksum();
    t!(ar.append(&header, &[][..]));

    let mut header = tar::Header::new_gnu();
    header.set_size(0);
    header.set_entry_type(tar::EntryType::Regular);
    t!(header.set_path("bar"));
    header.set_cksum();
    t!(ar.append(&header, &[][..]));

    let bytes = t!(ar.into_inner());
    let mut ar = tar::Archive::new(&bytes[..]);

    let td = t!(Builder::new().prefix("tar").tempdir());
    t!(ar.unpack(td.path()));
    t!(td.path().join("foo").join("bar").read_link());
    let dst = t!(td.path().join("foo").join("bar").canonicalize());
    t!(File::open(dst));
}

#[test]
fn modify_hard_link_just_created() {
    let mut ar = tar::Builder::new(Vec::new());

    let mut header = tar::Header::new_gnu();
    header.set_size(0);
    header.set_entry_type(tar::EntryType::Link);
    t!(header.set_path("foo"));
    t!(header.set_link_name("../test"));
    header.set_cksum();
    t!(ar.append(&header, &[][..]));

    let mut header = tar::Header::new_gnu();
    header.set_size(1);
    header.set_entry_type(tar::EntryType::Regular);
    t!(header.set_path("foo"));
    header.set_cksum();
    t!(ar.append(&header, &b"x"[..]));

    let bytes = t!(ar.into_inner());
    let mut ar = tar::Archive::new(&bytes[..]);

    let td = t!(Builder::new().prefix("tar").tempdir());

    let test = td.path().join("test");
    t!(File::create(&test));

    let dir = td.path().join("dir");
    assert!(ar.unpack(&dir).is_err());

    let mut contents = Vec::new();
    t!(t!(File::open(&test)).read_to_end(&mut contents));
    assert_eq!(contents.len(), 0);
}

#[test]
fn modify_symlink_just_created() {
    let mut ar = tar::Builder::new(Vec::new());

    let mut header = tar::Header::new_gnu();
    header.set_size(0);
    header.set_entry_type(tar::EntryType::Symlink);
    t!(header.set_path("foo"));
    t!(header.set_link_name("../test"));
    header.set_cksum();
    t!(ar.append(&header, &[][..]));

    let mut header = tar::Header::new_gnu();
    header.set_size(1);
    header.set_entry_type(tar::EntryType::Regular);
    t!(header.set_path("foo"));
    header.set_cksum();
    t!(ar.append(&header, &b"x"[..]));

    let bytes = t!(ar.into_inner());
    let mut ar = tar::Archive::new(&bytes[..]);

    let td = t!(Builder::new().prefix("tar").tempdir());

    let test = td.path().join("test");
    t!(File::create(&test));

    let dir = td.path().join("dir");
    t!(ar.unpack(&dir));

    let mut contents = Vec::new();
    t!(t!(File::open(&test)).read_to_end(&mut contents));
    assert_eq!(contents.len(), 0);
}
