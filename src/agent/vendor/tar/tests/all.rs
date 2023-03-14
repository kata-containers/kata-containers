extern crate filetime;
extern crate tar;
extern crate tempfile;
#[cfg(all(unix, feature = "xattr"))]
extern crate xattr;

use std::fs::{self, File};
use std::io::prelude::*;
use std::io::{self, Cursor};
use std::iter::repeat;
use std::path::{Path, PathBuf};

use filetime::FileTime;
use tar::{Archive, Builder, Entries, EntryType, Header, HeaderMode};
use tempfile::{Builder as TempBuilder, TempDir};

macro_rules! t {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(e) => panic!("{} returned {}", stringify!($e), e),
        }
    };
}

macro_rules! tar {
    ($e:expr) => {
        &include_bytes!(concat!("archives/", $e))[..]
    };
}

mod header;

/// test that we can concatenate the simple.tar archive and extract the same entries twice when we
/// use the ignore_zeros option.
#[test]
fn simple_concat() {
    let bytes = tar!("simple.tar");
    let mut archive_bytes = Vec::new();
    archive_bytes.extend(bytes);

    let original_names: Vec<String> = decode_names(&mut Archive::new(Cursor::new(&archive_bytes)));
    let expected: Vec<&str> = original_names.iter().map(|n| n.as_str()).collect();

    // concat two archives (with null in-between);
    archive_bytes.extend(bytes);

    // test now that when we read the archive, it stops processing at the first zero header.
    let actual = decode_names(&mut Archive::new(Cursor::new(&archive_bytes)));
    assert_eq!(expected, actual);

    // extend expected by itself.
    let expected: Vec<&str> = {
        let mut o = Vec::new();
        o.extend(&expected);
        o.extend(&expected);
        o
    };

    let mut ar = Archive::new(Cursor::new(&archive_bytes));
    ar.set_ignore_zeros(true);

    let actual = decode_names(&mut ar);
    assert_eq!(expected, actual);

    fn decode_names<R>(ar: &mut Archive<R>) -> Vec<String>
    where
        R: Read,
    {
        let mut names = Vec::new();

        for entry in t!(ar.entries()) {
            let e = t!(entry);
            names.push(t!(::std::str::from_utf8(&e.path_bytes())).to_string());
        }

        names
    }
}

#[test]
fn header_impls() {
    let mut ar = Archive::new(Cursor::new(tar!("simple.tar")));
    let hn = Header::new_old();
    let hnb = hn.as_bytes();
    for file in t!(ar.entries()) {
        let file = t!(file);
        let h1 = file.header();
        let h1b = h1.as_bytes();
        let h2 = h1.clone();
        let h2b = h2.as_bytes();
        assert!(h1b[..] == h2b[..] && h2b[..] != hnb[..])
    }
}

#[test]
fn header_impls_missing_last_header() {
    let mut ar = Archive::new(Cursor::new(tar!("simple_missing_last_header.tar")));
    let hn = Header::new_old();
    let hnb = hn.as_bytes();
    for file in t!(ar.entries()) {
        let file = t!(file);
        let h1 = file.header();
        let h1b = h1.as_bytes();
        let h2 = h1.clone();
        let h2b = h2.as_bytes();
        assert!(h1b[..] == h2b[..] && h2b[..] != hnb[..])
    }
}

#[test]
fn reading_files() {
    let rdr = Cursor::new(tar!("reading_files.tar"));
    let mut ar = Archive::new(rdr);
    let mut entries = t!(ar.entries());

    let mut a = t!(entries.next().unwrap());
    assert_eq!(&*a.header().path_bytes(), b"a");
    let mut s = String::new();
    t!(a.read_to_string(&mut s));
    assert_eq!(s, "a\na\na\na\na\na\na\na\na\na\na\n");

    let mut b = t!(entries.next().unwrap());
    assert_eq!(&*b.header().path_bytes(), b"b");
    s.truncate(0);
    t!(b.read_to_string(&mut s));
    assert_eq!(s, "b\nb\nb\nb\nb\nb\nb\nb\nb\nb\nb\n");

    assert!(entries.next().is_none());
}

#[test]
fn writing_files() {
    let mut ar = Builder::new(Vec::new());
    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir());

    let path = td.path().join("test");
    t!(t!(File::create(&path)).write_all(b"test"));

    t!(ar.append_file("test2", &mut t!(File::open(&path))));

    let data = t!(ar.into_inner());
    let mut ar = Archive::new(Cursor::new(data));
    let mut entries = t!(ar.entries());
    let mut f = t!(entries.next().unwrap());

    assert_eq!(&*f.header().path_bytes(), b"test2");
    assert_eq!(f.header().size().unwrap(), 4);
    let mut s = String::new();
    t!(f.read_to_string(&mut s));
    assert_eq!(s, "test");

    assert!(entries.next().is_none());
}

#[test]
fn large_filename() {
    let mut ar = Builder::new(Vec::new());
    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir());

    let path = td.path().join("test");
    t!(t!(File::create(&path)).write_all(b"test"));

    let filename = repeat("abcd/").take(50).collect::<String>();
    let mut header = Header::new_ustar();
    header.set_path(&filename).unwrap();
    header.set_metadata(&t!(fs::metadata(&path)));
    header.set_cksum();
    t!(ar.append(&header, &b"test"[..]));
    let too_long = repeat("abcd").take(200).collect::<String>();
    t!(ar.append_file(&too_long, &mut t!(File::open(&path))));
    t!(ar.append_data(&mut header, &too_long, &b"test"[..]));

    let rd = Cursor::new(t!(ar.into_inner()));
    let mut ar = Archive::new(rd);
    let mut entries = t!(ar.entries());

    // The short entry added with `append`
    let mut f = entries.next().unwrap().unwrap();
    assert_eq!(&*f.header().path_bytes(), filename.as_bytes());
    assert_eq!(f.header().size().unwrap(), 4);
    let mut s = String::new();
    t!(f.read_to_string(&mut s));
    assert_eq!(s, "test");

    // The long entry added with `append_file`
    let mut f = entries.next().unwrap().unwrap();
    assert_eq!(&*f.path_bytes(), too_long.as_bytes());
    assert_eq!(f.header().size().unwrap(), 4);
    let mut s = String::new();
    t!(f.read_to_string(&mut s));
    assert_eq!(s, "test");

    // The long entry added with `append_data`
    let mut f = entries.next().unwrap().unwrap();
    assert!(f.header().path_bytes().len() < too_long.len());
    assert_eq!(&*f.path_bytes(), too_long.as_bytes());
    assert_eq!(f.header().size().unwrap(), 4);
    let mut s = String::new();
    t!(f.read_to_string(&mut s));
    assert_eq!(s, "test");

    assert!(entries.next().is_none());
}

fn reading_entries_common<R: Read>(mut entries: Entries<R>) {
    let mut a = t!(entries.next().unwrap());
    assert_eq!(&*a.header().path_bytes(), b"a");
    let mut s = String::new();
    t!(a.read_to_string(&mut s));
    assert_eq!(s, "a\na\na\na\na\na\na\na\na\na\na\n");
    s.truncate(0);
    t!(a.read_to_string(&mut s));
    assert_eq!(s, "");

    let mut b = t!(entries.next().unwrap());
    assert_eq!(&*b.header().path_bytes(), b"b");
    s.truncate(0);
    t!(b.read_to_string(&mut s));
    assert_eq!(s, "b\nb\nb\nb\nb\nb\nb\nb\nb\nb\nb\n");
    assert!(entries.next().is_none());
}

#[test]
fn reading_entries() {
    let rdr = Cursor::new(tar!("reading_files.tar"));
    let mut ar = Archive::new(rdr);
    reading_entries_common(t!(ar.entries()));
}

#[test]
fn reading_entries_with_seek() {
    let rdr = Cursor::new(tar!("reading_files.tar"));
    let mut ar = Archive::new(rdr);
    reading_entries_common(t!(ar.entries_with_seek()));
}

struct LoggingReader<R> {
    inner: R,
    read_bytes: u64,
}

impl<R> LoggingReader<R> {
    fn new(reader: R) -> LoggingReader<R> {
        LoggingReader {
            inner: reader,
            read_bytes: 0,
        }
    }
}

impl<T: Read> Read for LoggingReader<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf).map(|i| {
            self.read_bytes += i as u64;
            i
        })
    }
}

impl<T: Seek> Seek for LoggingReader<T> {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        self.inner.seek(pos)
    }
}

#[test]
fn skipping_entries_with_seek() {
    let mut reader = LoggingReader::new(Cursor::new(tar!("reading_files.tar")));
    let mut ar_reader = Archive::new(&mut reader);
    let files: Vec<_> = t!(ar_reader.entries())
        .map(|entry| entry.unwrap().path().unwrap().to_path_buf())
        .collect();

    let mut seekable_reader = LoggingReader::new(Cursor::new(tar!("reading_files.tar")));
    let mut ar_seekable_reader = Archive::new(&mut seekable_reader);
    let files_seekable: Vec<_> = t!(ar_seekable_reader.entries_with_seek())
        .map(|entry| entry.unwrap().path().unwrap().to_path_buf())
        .collect();

    assert!(files == files_seekable);
    assert!(seekable_reader.read_bytes < reader.read_bytes);
}

fn check_dirtree(td: &TempDir) {
    let dir_a = td.path().join("a");
    let dir_b = td.path().join("a/b");
    let file_c = td.path().join("a/c");
    assert!(fs::metadata(&dir_a).map(|m| m.is_dir()).unwrap_or(false));
    assert!(fs::metadata(&dir_b).map(|m| m.is_dir()).unwrap_or(false));
    assert!(fs::metadata(&file_c).map(|m| m.is_file()).unwrap_or(false));
}

#[test]
fn extracting_directories() {
    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir());
    let rdr = Cursor::new(tar!("directory.tar"));
    let mut ar = Archive::new(rdr);
    t!(ar.unpack(td.path()));
    check_dirtree(&td);
}

#[test]
fn extracting_duplicate_file_fail() {
    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir());
    let path_present = td.path().join("a");
    t!(File::create(path_present));

    let rdr = Cursor::new(tar!("reading_files.tar"));
    let mut ar = Archive::new(rdr);
    ar.set_overwrite(false);
    if let Err(err) = ar.unpack(td.path()) {
        if err.kind() == std::io::ErrorKind::AlreadyExists {
            // as expected with overwrite false
            return;
        }
        panic!("unexpected error: {:?}", err);
    }
    panic!(
        "unpack() should have returned an error of kind {:?}, returned Ok",
        std::io::ErrorKind::AlreadyExists
    )
}

#[test]
fn extracting_duplicate_file_succeed() {
    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir());
    let path_present = td.path().join("a");
    t!(File::create(path_present));

    let rdr = Cursor::new(tar!("reading_files.tar"));
    let mut ar = Archive::new(rdr);
    ar.set_overwrite(true);
    t!(ar.unpack(td.path()));
}

#[test]
#[cfg(unix)]
fn extracting_duplicate_link_fail() {
    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir());
    let path_present = td.path().join("lnk");
    t!(std::os::unix::fs::symlink("file", path_present));

    let rdr = Cursor::new(tar!("link.tar"));
    let mut ar = Archive::new(rdr);
    ar.set_overwrite(false);
    if let Err(err) = ar.unpack(td.path()) {
        if err.kind() == std::io::ErrorKind::AlreadyExists {
            // as expected with overwrite false
            return;
        }
        panic!("unexpected error: {:?}", err);
    }
    panic!(
        "unpack() should have returned an error of kind {:?}, returned Ok",
        std::io::ErrorKind::AlreadyExists
    )
}

#[test]
#[cfg(unix)]
fn extracting_duplicate_link_succeed() {
    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir());
    let path_present = td.path().join("lnk");
    t!(std::os::unix::fs::symlink("file", path_present));

    let rdr = Cursor::new(tar!("link.tar"));
    let mut ar = Archive::new(rdr);
    ar.set_overwrite(true);
    t!(ar.unpack(td.path()));
}

#[test]
#[cfg(all(unix, feature = "xattr"))]
fn xattrs() {
    // If /tmp is a tmpfs, xattr will fail
    // The xattr crate's unit tests also use /var/tmp for this reason
    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir_in("/var/tmp"));
    let rdr = Cursor::new(tar!("xattrs.tar"));
    let mut ar = Archive::new(rdr);
    ar.set_unpack_xattrs(true);
    t!(ar.unpack(td.path()));

    let val = xattr::get(td.path().join("a/b"), "user.pax.flags").unwrap();
    assert_eq!(val.unwrap(), "epm".as_bytes());
}

#[test]
#[cfg(all(unix, feature = "xattr"))]
fn no_xattrs() {
    // If /tmp is a tmpfs, xattr will fail
    // The xattr crate's unit tests also use /var/tmp for this reason
    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir_in("/var/tmp"));
    let rdr = Cursor::new(tar!("xattrs.tar"));
    let mut ar = Archive::new(rdr);
    ar.set_unpack_xattrs(false);
    t!(ar.unpack(td.path()));

    assert_eq!(
        xattr::get(td.path().join("a/b"), "user.pax.flags").unwrap(),
        None
    );
}

#[test]
fn writing_and_extracting_directories() {
    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir());

    let mut ar = Builder::new(Vec::new());
    let tmppath = td.path().join("tmpfile");
    t!(t!(File::create(&tmppath)).write_all(b"c"));
    t!(ar.append_dir("a", "."));
    t!(ar.append_dir("a/b", "."));
    t!(ar.append_file("a/c", &mut t!(File::open(&tmppath))));
    t!(ar.finish());

    let rdr = Cursor::new(t!(ar.into_inner()));
    let mut ar = Archive::new(rdr);
    t!(ar.unpack(td.path()));
    check_dirtree(&td);
}

#[test]
fn writing_directories_recursively() {
    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir());

    let base_dir = td.path().join("base");
    t!(fs::create_dir(&base_dir));
    t!(t!(File::create(base_dir.join("file1"))).write_all(b"file1"));
    let sub_dir = base_dir.join("sub");
    t!(fs::create_dir(&sub_dir));
    t!(t!(File::create(sub_dir.join("file2"))).write_all(b"file2"));

    let mut ar = Builder::new(Vec::new());
    t!(ar.append_dir_all("foobar", base_dir));
    let data = t!(ar.into_inner());

    let mut ar = Archive::new(Cursor::new(data));
    t!(ar.unpack(td.path()));
    let base_dir = td.path().join("foobar");
    assert!(fs::metadata(&base_dir).map(|m| m.is_dir()).unwrap_or(false));
    let file1_path = base_dir.join("file1");
    assert!(fs::metadata(&file1_path)
        .map(|m| m.is_file())
        .unwrap_or(false));
    let sub_dir = base_dir.join("sub");
    assert!(fs::metadata(&sub_dir).map(|m| m.is_dir()).unwrap_or(false));
    let file2_path = sub_dir.join("file2");
    assert!(fs::metadata(&file2_path)
        .map(|m| m.is_file())
        .unwrap_or(false));
}

#[test]
fn append_dir_all_blank_dest() {
    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir());

    let base_dir = td.path().join("base");
    t!(fs::create_dir(&base_dir));
    t!(t!(File::create(base_dir.join("file1"))).write_all(b"file1"));
    let sub_dir = base_dir.join("sub");
    t!(fs::create_dir(&sub_dir));
    t!(t!(File::create(sub_dir.join("file2"))).write_all(b"file2"));

    let mut ar = Builder::new(Vec::new());
    t!(ar.append_dir_all("", base_dir));
    let data = t!(ar.into_inner());

    let mut ar = Archive::new(Cursor::new(data));
    t!(ar.unpack(td.path()));
    let base_dir = td.path();
    assert!(fs::metadata(&base_dir).map(|m| m.is_dir()).unwrap_or(false));
    let file1_path = base_dir.join("file1");
    assert!(fs::metadata(&file1_path)
        .map(|m| m.is_file())
        .unwrap_or(false));
    let sub_dir = base_dir.join("sub");
    assert!(fs::metadata(&sub_dir).map(|m| m.is_dir()).unwrap_or(false));
    let file2_path = sub_dir.join("file2");
    assert!(fs::metadata(&file2_path)
        .map(|m| m.is_file())
        .unwrap_or(false));
}

#[test]
fn append_dir_all_does_not_work_on_non_directory() {
    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir());
    let path = td.path().join("test");
    t!(t!(File::create(&path)).write_all(b"test"));

    let mut ar = Builder::new(Vec::new());
    let result = ar.append_dir_all("test", path);
    assert!(result.is_err());
}

#[test]
fn extracting_duplicate_dirs() {
    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir());
    let rdr = Cursor::new(tar!("duplicate_dirs.tar"));
    let mut ar = Archive::new(rdr);
    t!(ar.unpack(td.path()));

    let some_dir = td.path().join("some_dir");
    assert!(fs::metadata(&some_dir).map(|m| m.is_dir()).unwrap_or(false));
}

#[test]
fn unpack_old_style_bsd_dir() {
    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir());

    let mut ar = Builder::new(Vec::new());

    let mut header = Header::new_old();
    header.set_entry_type(EntryType::Regular);
    t!(header.set_path("testdir/"));
    header.set_size(0);
    header.set_cksum();
    t!(ar.append(&header, &mut io::empty()));

    // Extracting
    let rdr = Cursor::new(t!(ar.into_inner()));
    let mut ar = Archive::new(rdr);
    t!(ar.unpack(td.path()));

    // Iterating
    let rdr = Cursor::new(ar.into_inner().into_inner());
    let mut ar = Archive::new(rdr);
    assert!(t!(ar.entries()).all(|fr| fr.is_ok()));

    assert!(td.path().join("testdir").is_dir());
}

#[test]
fn handling_incorrect_file_size() {
    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir());

    let mut ar = Builder::new(Vec::new());

    let path = td.path().join("tmpfile");
    t!(File::create(&path));
    let mut file = t!(File::open(&path));
    let mut header = Header::new_old();
    t!(header.set_path("somepath"));
    header.set_metadata(&t!(file.metadata()));
    header.set_size(2048); // past the end of file null blocks
    header.set_cksum();
    t!(ar.append(&header, &mut file));

    // Extracting
    let rdr = Cursor::new(t!(ar.into_inner()));
    let mut ar = Archive::new(rdr);
    assert!(ar.unpack(td.path()).is_err());

    // Iterating
    let rdr = Cursor::new(ar.into_inner().into_inner());
    let mut ar = Archive::new(rdr);
    assert!(t!(ar.entries()).any(|fr| fr.is_err()));
}

#[test]
fn extracting_malicious_tarball() {
    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir());

    let mut evil_tar = Vec::new();

    {
        let mut a = Builder::new(&mut evil_tar);
        let mut append = |path: &str| {
            let mut header = Header::new_gnu();
            assert!(header.set_path(path).is_err(), "was ok: {:?}", path);
            {
                let h = header.as_gnu_mut().unwrap();
                for (a, b) in h.name.iter_mut().zip(path.as_bytes()) {
                    *a = *b;
                }
            }
            header.set_size(1);
            header.set_cksum();
            t!(a.append(&header, io::repeat(1).take(1)));
        };
        append("/tmp/abs_evil.txt");
        append("//tmp/abs_evil2.txt");
        append("///tmp/abs_evil3.txt");
        append("/./tmp/abs_evil4.txt");
        append("//./tmp/abs_evil5.txt");
        append("///./tmp/abs_evil6.txt");
        append("/../tmp/rel_evil.txt");
        append("../rel_evil2.txt");
        append("./../rel_evil3.txt");
        append("some/../../rel_evil4.txt");
        append("");
        append("././//./..");
        append("..");
        append("/////////..");
        append("/////////");
    }

    let mut ar = Archive::new(&evil_tar[..]);
    t!(ar.unpack(td.path()));

    assert!(fs::metadata("/tmp/abs_evil.txt").is_err());
    assert!(fs::metadata("/tmp/abs_evil.txt2").is_err());
    assert!(fs::metadata("/tmp/abs_evil.txt3").is_err());
    assert!(fs::metadata("/tmp/abs_evil.txt4").is_err());
    assert!(fs::metadata("/tmp/abs_evil.txt5").is_err());
    assert!(fs::metadata("/tmp/abs_evil.txt6").is_err());
    assert!(fs::metadata("/tmp/rel_evil.txt").is_err());
    assert!(fs::metadata("/tmp/rel_evil.txt").is_err());
    assert!(fs::metadata(td.path().join("../tmp/rel_evil.txt")).is_err());
    assert!(fs::metadata(td.path().join("../rel_evil2.txt")).is_err());
    assert!(fs::metadata(td.path().join("../rel_evil3.txt")).is_err());
    assert!(fs::metadata(td.path().join("../rel_evil4.txt")).is_err());

    // The `some` subdirectory should not be created because the only
    // filename that references this has '..'.
    assert!(fs::metadata(td.path().join("some")).is_err());

    // The `tmp` subdirectory should be created and within this
    // subdirectory, there should be files named `abs_evil.txt` through
    // `abs_evil6.txt`.
    assert!(fs::metadata(td.path().join("tmp"))
        .map(|m| m.is_dir())
        .unwrap_or(false));
    assert!(fs::metadata(td.path().join("tmp/abs_evil.txt"))
        .map(|m| m.is_file())
        .unwrap_or(false));
    assert!(fs::metadata(td.path().join("tmp/abs_evil2.txt"))
        .map(|m| m.is_file())
        .unwrap_or(false));
    assert!(fs::metadata(td.path().join("tmp/abs_evil3.txt"))
        .map(|m| m.is_file())
        .unwrap_or(false));
    assert!(fs::metadata(td.path().join("tmp/abs_evil4.txt"))
        .map(|m| m.is_file())
        .unwrap_or(false));
    assert!(fs::metadata(td.path().join("tmp/abs_evil5.txt"))
        .map(|m| m.is_file())
        .unwrap_or(false));
    assert!(fs::metadata(td.path().join("tmp/abs_evil6.txt"))
        .map(|m| m.is_file())
        .unwrap_or(false));
}

#[test]
fn octal_spaces() {
    let rdr = Cursor::new(tar!("spaces.tar"));
    let mut ar = Archive::new(rdr);

    let entry = ar.entries().unwrap().next().unwrap().unwrap();
    assert_eq!(entry.header().mode().unwrap() & 0o777, 0o777);
    assert_eq!(entry.header().uid().unwrap(), 0);
    assert_eq!(entry.header().gid().unwrap(), 0);
    assert_eq!(entry.header().size().unwrap(), 2);
    assert_eq!(entry.header().mtime().unwrap(), 0o12440016664);
    assert_eq!(entry.header().cksum().unwrap(), 0o4253);
}

#[test]
fn extracting_malformed_tar_null_blocks() {
    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir());

    let mut ar = Builder::new(Vec::new());

    let path1 = td.path().join("tmpfile1");
    let path2 = td.path().join("tmpfile2");
    t!(File::create(&path1));
    t!(File::create(&path2));
    t!(ar.append_file("tmpfile1", &mut t!(File::open(&path1))));
    let mut data = t!(ar.into_inner());
    let amt = data.len();
    data.truncate(amt - 512);
    let mut ar = Builder::new(data);
    t!(ar.append_file("tmpfile2", &mut t!(File::open(&path2))));
    t!(ar.finish());

    let data = t!(ar.into_inner());
    let mut ar = Archive::new(&data[..]);
    assert!(ar.unpack(td.path()).is_ok());
}

#[test]
fn empty_filename() {
    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir());
    let rdr = Cursor::new(tar!("empty_filename.tar"));
    let mut ar = Archive::new(rdr);
    assert!(ar.unpack(td.path()).is_ok());
}

#[test]
fn file_times() {
    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir());
    let rdr = Cursor::new(tar!("file_times.tar"));
    let mut ar = Archive::new(rdr);
    t!(ar.unpack(td.path()));

    let meta = fs::metadata(td.path().join("a")).unwrap();
    let mtime = FileTime::from_last_modification_time(&meta);
    let atime = FileTime::from_last_access_time(&meta);
    assert_eq!(mtime.unix_seconds(), 1000000000);
    assert_eq!(mtime.nanoseconds(), 0);
    assert_eq!(atime.unix_seconds(), 1000000000);
    assert_eq!(atime.nanoseconds(), 0);
}

#[test]
fn zero_file_times() {
    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir());

    let mut ar = Builder::new(Vec::new());
    ar.mode(HeaderMode::Deterministic);
    let path = td.path().join("tmpfile");
    t!(File::create(&path));
    t!(ar.append_path_with_name(&path, "a"));

    let data = t!(ar.into_inner());
    let mut ar = Archive::new(&data[..]);
    assert!(ar.unpack(td.path()).is_ok());

    let meta = fs::metadata(td.path().join("a")).unwrap();
    let mtime = FileTime::from_last_modification_time(&meta);
    let atime = FileTime::from_last_access_time(&meta);
    assert!(mtime.unix_seconds() != 0);
    assert!(atime.unix_seconds() != 0);
}

#[test]
fn backslash_treated_well() {
    // Insert a file into an archive with a backslash
    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir());
    let mut ar = Builder::new(Vec::<u8>::new());
    t!(ar.append_dir("foo\\bar", td.path()));
    let mut ar = Archive::new(Cursor::new(t!(ar.into_inner())));
    let f = t!(t!(ar.entries()).next().unwrap());
    if cfg!(unix) {
        assert_eq!(t!(f.header().path()).to_str(), Some("foo\\bar"));
    } else {
        assert_eq!(t!(f.header().path()).to_str(), Some("foo/bar"));
    }

    // Unpack an archive with a backslash in the name
    let mut ar = Builder::new(Vec::<u8>::new());
    let mut header = Header::new_gnu();
    header.set_metadata(&t!(fs::metadata(td.path())));
    header.set_size(0);
    for (a, b) in header.as_old_mut().name.iter_mut().zip(b"foo\\bar\x00") {
        *a = *b;
    }
    header.set_cksum();
    t!(ar.append(&header, &mut io::empty()));
    let data = t!(ar.into_inner());
    let mut ar = Archive::new(&data[..]);
    let f = t!(t!(ar.entries()).next().unwrap());
    assert_eq!(t!(f.header().path()).to_str(), Some("foo\\bar"));

    let mut ar = Archive::new(&data[..]);
    t!(ar.unpack(td.path()));
    assert!(fs::metadata(td.path().join("foo\\bar")).is_ok());
}

#[cfg(unix)]
#[test]
fn nul_bytes_in_path() {
    use std::ffi::OsStr;
    use std::os::unix::prelude::*;

    let nul_path = OsStr::from_bytes(b"foo\0");
    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir());
    let mut ar = Builder::new(Vec::<u8>::new());
    let err = ar.append_dir(nul_path, td.path()).unwrap_err();
    assert!(err.to_string().contains("contains a nul byte"));
}

#[test]
fn links() {
    let mut ar = Archive::new(Cursor::new(tar!("link.tar")));
    let mut entries = t!(ar.entries());
    let link = t!(entries.next().unwrap());
    assert_eq!(
        t!(link.header().link_name()).as_ref().map(|p| &**p),
        Some(Path::new("file"))
    );
    let other = t!(entries.next().unwrap());
    assert!(t!(other.header().link_name()).is_none());
}

#[test]
#[cfg(unix)] // making symlinks on windows is hard
fn unpack_links() {
    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir());
    let mut ar = Archive::new(Cursor::new(tar!("link.tar")));
    t!(ar.unpack(td.path()));

    let md = t!(fs::symlink_metadata(td.path().join("lnk")));
    assert!(md.file_type().is_symlink());
    assert_eq!(
        &*t!(fs::read_link(td.path().join("lnk"))),
        Path::new("file")
    );
    t!(File::open(td.path().join("lnk")));
}

#[test]
fn pax_size() {
    let mut ar = Archive::new(tar!("pax_size.tar"));
    let mut entries = t!(ar.entries());
    let mut entry = t!(entries.next().unwrap());
    let mut attributes = t!(entry.pax_extensions()).unwrap();

    let _first = t!(attributes.next().unwrap());
    let _second = t!(attributes.next().unwrap());
    let _third = t!(attributes.next().unwrap());
    let fourth = t!(attributes.next().unwrap());
    assert!(attributes.next().is_none());

    assert_eq!(fourth.key(), Ok("size"));
    assert_eq!(fourth.value(), Ok("4"));

    assert_eq!(entry.header().size().unwrap(), 0);
    assert_eq!(entry.size(), 4);
}

#[test]
fn pax_simple() {
    let mut ar = Archive::new(tar!("pax.tar"));
    let mut entries = t!(ar.entries());

    let mut first = t!(entries.next().unwrap());
    let mut attributes = t!(first.pax_extensions()).unwrap();
    let first = t!(attributes.next().unwrap());
    let second = t!(attributes.next().unwrap());
    let third = t!(attributes.next().unwrap());
    assert!(attributes.next().is_none());

    assert_eq!(first.key(), Ok("mtime"));
    assert_eq!(first.value(), Ok("1453146164.953123768"));
    assert_eq!(second.key(), Ok("atime"));
    assert_eq!(second.value(), Ok("1453251915.24892486"));
    assert_eq!(third.key(), Ok("ctime"));
    assert_eq!(third.value(), Ok("1453146164.953123768"));
}

#[test]
fn pax_path() {
    let mut ar = Archive::new(tar!("pax2.tar"));
    let mut entries = t!(ar.entries());

    let first = t!(entries.next().unwrap());
    assert!(first.path().unwrap().ends_with("aaaaaaaaaaaaaaa"));
}

#[test]
fn pax_linkpath() {
    let mut ar = Archive::new(tar!("pax2.tar"));
    let mut links = t!(ar.entries()).skip(3).take(2);

    let long_symlink = t!(links.next().unwrap());
    let link_name = long_symlink.link_name().unwrap().unwrap();
    assert!(link_name.to_str().unwrap().len() > 99);
    assert!(link_name.ends_with("bbbbbbbbbbbbbbb"));

    let long_hardlink = t!(links.next().unwrap());
    let link_name = long_hardlink.link_name().unwrap().unwrap();
    assert!(link_name.to_str().unwrap().len() > 99);
    assert!(link_name.ends_with("ccccccccccccccc"));
}

#[test]
fn long_name_trailing_nul() {
    let mut b = Builder::new(Vec::<u8>::new());

    let mut h = Header::new_gnu();
    t!(h.set_path("././@LongLink"));
    h.set_size(4);
    h.set_entry_type(EntryType::new(b'L'));
    h.set_cksum();
    t!(b.append(&h, "foo\0".as_bytes()));
    let mut h = Header::new_gnu();

    t!(h.set_path("bar"));
    h.set_size(6);
    h.set_entry_type(EntryType::file());
    h.set_cksum();
    t!(b.append(&h, "foobar".as_bytes()));

    let contents = t!(b.into_inner());
    let mut a = Archive::new(&contents[..]);

    let e = t!(t!(a.entries()).next().unwrap());
    assert_eq!(&*e.path_bytes(), b"foo");
}

#[test]
fn long_linkname_trailing_nul() {
    let mut b = Builder::new(Vec::<u8>::new());

    let mut h = Header::new_gnu();
    t!(h.set_path("././@LongLink"));
    h.set_size(4);
    h.set_entry_type(EntryType::new(b'K'));
    h.set_cksum();
    t!(b.append(&h, "foo\0".as_bytes()));
    let mut h = Header::new_gnu();

    t!(h.set_path("bar"));
    h.set_size(6);
    h.set_entry_type(EntryType::file());
    h.set_cksum();
    t!(b.append(&h, "foobar".as_bytes()));

    let contents = t!(b.into_inner());
    let mut a = Archive::new(&contents[..]);

    let e = t!(t!(a.entries()).next().unwrap());
    assert_eq!(&*e.link_name_bytes().unwrap(), b"foo");
}

#[test]
fn long_linkname_gnu() {
    for t in [tar::EntryType::Symlink, tar::EntryType::Link] {
        let mut b = Builder::new(Vec::<u8>::new());
        let mut h = Header::new_gnu();
        h.set_entry_type(t);
        h.set_size(0);
        let path = "usr/lib/.build-id/05/159ed904e45ff5100f7acd3d3b99fa7e27e34f";
        let target = "../../../../usr/lib64/qt5/plugins/wayland-graphics-integration-server/libqt-wayland-compositor-xcomposite-egl.so";
        t!(b.append_link(&mut h, path, target));

        let contents = t!(b.into_inner());
        let mut a = Archive::new(&contents[..]);

        let e = &t!(t!(a.entries()).next().unwrap());
        assert_eq!(e.header().entry_type(), t);
        assert_eq!(e.path().unwrap().to_str().unwrap(), path);
        assert_eq!(e.link_name().unwrap().unwrap().to_str().unwrap(), target);
    }
}

#[test]
fn linkname_literal() {
    for t in [tar::EntryType::Symlink, tar::EntryType::Link] {
        let mut b = Builder::new(Vec::<u8>::new());
        let mut h = Header::new_gnu();
        h.set_entry_type(t);
        h.set_size(0);
        let path = "usr/lib/systemd/systemd-sysv-install";
        let target = "../../..//sbin/chkconfig";
        h.set_link_name_literal(target).unwrap();
        t!(b.append_data(&mut h, path, std::io::empty()));

        let contents = t!(b.into_inner());
        let mut a = Archive::new(&contents[..]);

        let e = &t!(t!(a.entries()).next().unwrap());
        assert_eq!(e.header().entry_type(), t);
        assert_eq!(e.path().unwrap().to_str().unwrap(), path);
        assert_eq!(e.link_name().unwrap().unwrap().to_str().unwrap(), target);
    }
}

#[test]
fn encoded_long_name_has_trailing_nul() {
    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir());
    let path = td.path().join("foo");
    t!(t!(File::create(&path)).write_all(b"test"));

    let mut b = Builder::new(Vec::<u8>::new());
    let long = repeat("abcd").take(200).collect::<String>();

    t!(b.append_file(&long, &mut t!(File::open(&path))));

    let contents = t!(b.into_inner());
    let mut a = Archive::new(&contents[..]);

    let mut e = t!(t!(a.entries()).raw(true).next().unwrap());
    let mut name = Vec::new();
    t!(e.read_to_end(&mut name));
    assert_eq!(name[name.len() - 1], 0);

    let header_name = &e.header().as_gnu().unwrap().name;
    assert!(header_name.starts_with(b"././@LongLink\x00"));
}

#[test]
fn reading_sparse() {
    let rdr = Cursor::new(tar!("sparse.tar"));
    let mut ar = Archive::new(rdr);
    let mut entries = t!(ar.entries());

    let mut a = t!(entries.next().unwrap());
    let mut s = String::new();
    assert_eq!(&*a.header().path_bytes(), b"sparse_begin.txt");
    t!(a.read_to_string(&mut s));
    assert_eq!(&s[..5], "test\n");
    assert!(s[5..].chars().all(|x| x == '\u{0}'));

    let mut a = t!(entries.next().unwrap());
    let mut s = String::new();
    assert_eq!(&*a.header().path_bytes(), b"sparse_end.txt");
    t!(a.read_to_string(&mut s));
    assert!(s[..s.len() - 9].chars().all(|x| x == '\u{0}'));
    assert_eq!(&s[s.len() - 9..], "test_end\n");

    let mut a = t!(entries.next().unwrap());
    let mut s = String::new();
    assert_eq!(&*a.header().path_bytes(), b"sparse_ext.txt");
    t!(a.read_to_string(&mut s));
    assert!(s[..0x1000].chars().all(|x| x == '\u{0}'));
    assert_eq!(&s[0x1000..0x1000 + 5], "text\n");
    assert!(s[0x1000 + 5..0x3000].chars().all(|x| x == '\u{0}'));
    assert_eq!(&s[0x3000..0x3000 + 5], "text\n");
    assert!(s[0x3000 + 5..0x5000].chars().all(|x| x == '\u{0}'));
    assert_eq!(&s[0x5000..0x5000 + 5], "text\n");
    assert!(s[0x5000 + 5..0x7000].chars().all(|x| x == '\u{0}'));
    assert_eq!(&s[0x7000..0x7000 + 5], "text\n");
    assert!(s[0x7000 + 5..0x9000].chars().all(|x| x == '\u{0}'));
    assert_eq!(&s[0x9000..0x9000 + 5], "text\n");
    assert!(s[0x9000 + 5..0xb000].chars().all(|x| x == '\u{0}'));
    assert_eq!(&s[0xb000..0xb000 + 5], "text\n");

    let mut a = t!(entries.next().unwrap());
    let mut s = String::new();
    assert_eq!(&*a.header().path_bytes(), b"sparse.txt");
    t!(a.read_to_string(&mut s));
    assert!(s[..0x1000].chars().all(|x| x == '\u{0}'));
    assert_eq!(&s[0x1000..0x1000 + 6], "hello\n");
    assert!(s[0x1000 + 6..0x2fa0].chars().all(|x| x == '\u{0}'));
    assert_eq!(&s[0x2fa0..0x2fa0 + 6], "world\n");
    assert!(s[0x2fa0 + 6..0x4000].chars().all(|x| x == '\u{0}'));

    assert!(entries.next().is_none());
}

#[test]
fn extract_sparse() {
    let rdr = Cursor::new(tar!("sparse.tar"));
    let mut ar = Archive::new(rdr);
    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir());
    t!(ar.unpack(td.path()));

    let mut s = String::new();
    t!(t!(File::open(td.path().join("sparse_begin.txt"))).read_to_string(&mut s));
    assert_eq!(&s[..5], "test\n");
    assert!(s[5..].chars().all(|x| x == '\u{0}'));

    s.truncate(0);
    t!(t!(File::open(td.path().join("sparse_end.txt"))).read_to_string(&mut s));
    assert!(s[..s.len() - 9].chars().all(|x| x == '\u{0}'));
    assert_eq!(&s[s.len() - 9..], "test_end\n");

    s.truncate(0);
    t!(t!(File::open(td.path().join("sparse_ext.txt"))).read_to_string(&mut s));
    assert!(s[..0x1000].chars().all(|x| x == '\u{0}'));
    assert_eq!(&s[0x1000..0x1000 + 5], "text\n");
    assert!(s[0x1000 + 5..0x3000].chars().all(|x| x == '\u{0}'));
    assert_eq!(&s[0x3000..0x3000 + 5], "text\n");
    assert!(s[0x3000 + 5..0x5000].chars().all(|x| x == '\u{0}'));
    assert_eq!(&s[0x5000..0x5000 + 5], "text\n");
    assert!(s[0x5000 + 5..0x7000].chars().all(|x| x == '\u{0}'));
    assert_eq!(&s[0x7000..0x7000 + 5], "text\n");
    assert!(s[0x7000 + 5..0x9000].chars().all(|x| x == '\u{0}'));
    assert_eq!(&s[0x9000..0x9000 + 5], "text\n");
    assert!(s[0x9000 + 5..0xb000].chars().all(|x| x == '\u{0}'));
    assert_eq!(&s[0xb000..0xb000 + 5], "text\n");

    s.truncate(0);
    t!(t!(File::open(td.path().join("sparse.txt"))).read_to_string(&mut s));
    assert!(s[..0x1000].chars().all(|x| x == '\u{0}'));
    assert_eq!(&s[0x1000..0x1000 + 6], "hello\n");
    assert!(s[0x1000 + 6..0x2fa0].chars().all(|x| x == '\u{0}'));
    assert_eq!(&s[0x2fa0..0x2fa0 + 6], "world\n");
    assert!(s[0x2fa0 + 6..0x4000].chars().all(|x| x == '\u{0}'));
}

#[test]
fn sparse_with_trailing() {
    let rdr = Cursor::new(tar!("sparse-1.tar"));
    let mut ar = Archive::new(rdr);
    let mut entries = t!(ar.entries());
    let mut a = t!(entries.next().unwrap());
    let mut s = String::new();
    t!(a.read_to_string(&mut s));
    assert_eq!(0x100_00c, s.len());
    assert_eq!(&s[..0xc], "0MB through\n");
    assert!(s[0xc..0x100_000].chars().all(|x| x == '\u{0}'));
    assert_eq!(&s[0x100_000..], "1MB through\n");
}

#[test]
fn path_separators() {
    let mut ar = Builder::new(Vec::new());
    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir());

    let path = td.path().join("test");
    t!(t!(File::create(&path)).write_all(b"test"));

    let short_path: PathBuf = repeat("abcd").take(2).collect();
    let long_path: PathBuf = repeat("abcd").take(50).collect();

    // Make sure UStar headers normalize to Unix path separators
    let mut header = Header::new_ustar();

    t!(header.set_path(&short_path));
    assert_eq!(t!(header.path()), short_path);
    assert!(!header.path_bytes().contains(&b'\\'));

    t!(header.set_path(&long_path));
    assert_eq!(t!(header.path()), long_path);
    assert!(!header.path_bytes().contains(&b'\\'));

    // Make sure GNU headers normalize to Unix path separators,
    // including the `@LongLink` fallback used by `append_file`.
    t!(ar.append_file(&short_path, &mut t!(File::open(&path))));
    t!(ar.append_file(&long_path, &mut t!(File::open(&path))));

    let rd = Cursor::new(t!(ar.into_inner()));
    let mut ar = Archive::new(rd);
    let mut entries = t!(ar.entries());

    let entry = t!(entries.next().unwrap());
    assert_eq!(t!(entry.path()), short_path);
    assert!(!entry.path_bytes().contains(&b'\\'));

    let entry = t!(entries.next().unwrap());
    assert_eq!(t!(entry.path()), long_path);
    assert!(!entry.path_bytes().contains(&b'\\'));

    assert!(entries.next().is_none());
}

#[test]
#[cfg(unix)]
fn append_path_symlink() {
    use std::borrow::Cow;
    use std::env;
    use std::os::unix::fs::symlink;

    let mut ar = Builder::new(Vec::new());
    ar.follow_symlinks(false);
    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir());

    let long_linkname = repeat("abcd").take(30).collect::<String>();
    let long_pathname = repeat("dcba").take(30).collect::<String>();
    t!(env::set_current_dir(td.path()));
    // "short" path name / short link name
    t!(symlink("testdest", "test"));
    t!(ar.append_path("test"));
    // short path name / long link name
    t!(symlink(&long_linkname, "test2"));
    t!(ar.append_path("test2"));
    // long path name / long link name
    t!(symlink(&long_linkname, &long_pathname));
    t!(ar.append_path(&long_pathname));

    let rd = Cursor::new(t!(ar.into_inner()));
    let mut ar = Archive::new(rd);
    let mut entries = t!(ar.entries());

    let entry = t!(entries.next().unwrap());
    assert_eq!(t!(entry.path()), Path::new("test"));
    assert_eq!(
        t!(entry.link_name()),
        Some(Cow::from(Path::new("testdest")))
    );
    assert_eq!(t!(entry.header().size()), 0);

    let entry = t!(entries.next().unwrap());
    assert_eq!(t!(entry.path()), Path::new("test2"));
    assert_eq!(
        t!(entry.link_name()),
        Some(Cow::from(Path::new(&long_linkname)))
    );
    assert_eq!(t!(entry.header().size()), 0);

    let entry = t!(entries.next().unwrap());
    assert_eq!(t!(entry.path()), Path::new(&long_pathname));
    assert_eq!(
        t!(entry.link_name()),
        Some(Cow::from(Path::new(&long_linkname)))
    );
    assert_eq!(t!(entry.header().size()), 0);

    assert!(entries.next().is_none());
}

#[test]
fn name_with_slash_doesnt_fool_long_link_and_bsd_compat() {
    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir());

    let mut ar = Builder::new(Vec::new());

    let mut h = Header::new_gnu();
    t!(h.set_path("././@LongLink"));
    h.set_size(4);
    h.set_entry_type(EntryType::new(b'L'));
    h.set_cksum();
    t!(ar.append(&h, "foo\0".as_bytes()));

    let mut header = Header::new_gnu();
    header.set_entry_type(EntryType::Regular);
    t!(header.set_path("testdir/"));
    header.set_size(0);
    header.set_cksum();
    t!(ar.append(&header, &mut io::empty()));

    // Extracting
    let rdr = Cursor::new(t!(ar.into_inner()));
    let mut ar = Archive::new(rdr);
    t!(ar.unpack(td.path()));

    // Iterating
    let rdr = Cursor::new(ar.into_inner().into_inner());
    let mut ar = Archive::new(rdr);
    assert!(t!(ar.entries()).all(|fr| fr.is_ok()));

    assert!(td.path().join("foo").is_file());
}

#[test]
fn insert_local_file_different_name() {
    let mut ar = Builder::new(Vec::new());
    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir());
    let path = td.path().join("directory");
    t!(fs::create_dir(&path));
    ar.append_path_with_name(&path, "archive/dir").unwrap();
    let path = td.path().join("file");
    t!(t!(File::create(&path)).write_all(b"test"));
    ar.append_path_with_name(&path, "archive/dir/f").unwrap();

    let rd = Cursor::new(t!(ar.into_inner()));
    let mut ar = Archive::new(rd);
    let mut entries = t!(ar.entries());
    let entry = t!(entries.next().unwrap());
    assert_eq!(t!(entry.path()), Path::new("archive/dir"));
    let entry = t!(entries.next().unwrap());
    assert_eq!(t!(entry.path()), Path::new("archive/dir/f"));
    assert!(entries.next().is_none());
}

#[test]
#[cfg(unix)]
fn tar_directory_containing_symlink_to_directory() {
    use std::os::unix::fs::symlink;

    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir());
    let dummy_src = t!(TempBuilder::new().prefix("dummy_src").tempdir());
    let dummy_dst = td.path().join("dummy_dst");
    let mut ar = Builder::new(Vec::new());
    t!(symlink(dummy_src.path().display().to_string(), &dummy_dst));

    assert!(dummy_dst.read_link().is_ok());
    assert!(dummy_dst.read_link().unwrap().is_dir());
    ar.append_dir_all("symlinks", td.path()).unwrap();
    ar.finish().unwrap();
}

#[test]
fn long_path() {
    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir());
    let rdr = Cursor::new(tar!("7z_long_path.tar"));
    let mut ar = Archive::new(rdr);
    assert!(ar.unpack(td.path()).is_ok());
}

#[test]
fn unpack_path_larger_than_windows_max_path() {
    let dir_name = "iamaprettylongnameandtobepreciseiam91characterslongwhichsomethinkisreallylongandothersdonot";
    // 183 character directory name
    let really_long_path = format!("{}{}", dir_name, dir_name);
    let td = t!(TempBuilder::new().prefix(&really_long_path).tempdir());
    // directory in 7z_long_path.tar is over 100 chars
    let rdr = Cursor::new(tar!("7z_long_path.tar"));
    let mut ar = Archive::new(rdr);
    // should unpack path greater than windows MAX_PATH length of 260 characters
    assert!(ar.unpack(td.path()).is_ok());
}

#[test]
fn append_long_multibyte() {
    let mut x = tar::Builder::new(Vec::new());
    let mut name = String::new();
    let data: &[u8] = &[];
    for _ in 0..512 {
        name.push('a');
        name.push('𑢮');
        x.append_data(&mut Header::new_gnu(), &name, data).unwrap();
        name.pop();
    }
}

#[test]
fn read_only_directory_containing_files() {
    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir());

    let mut b = Builder::new(Vec::<u8>::new());

    let mut h = Header::new_gnu();
    t!(h.set_path("dir/"));
    h.set_size(0);
    h.set_entry_type(EntryType::dir());
    h.set_mode(0o444);
    h.set_cksum();
    t!(b.append(&h, "".as_bytes()));

    let mut h = Header::new_gnu();
    t!(h.set_path("dir/file"));
    h.set_size(2);
    h.set_entry_type(EntryType::file());
    h.set_cksum();
    t!(b.append(&h, "hi".as_bytes()));

    let contents = t!(b.into_inner());
    let mut ar = Archive::new(&contents[..]);
    assert!(ar.unpack(td.path()).is_ok());
}

// This test was marked linux only due to macOS CI can't handle `set_current_dir` correctly
#[test]
#[cfg(target_os = "linux")]
fn tar_directory_containing_special_files() {
    use std::env;
    use std::ffi::CString;

    let td = t!(TempBuilder::new().prefix("tar-rs").tempdir());
    let fifo = td.path().join("fifo");

    unsafe {
        let fifo_path = t!(CString::new(fifo.to_str().unwrap()));
        let ret = libc::mknod(fifo_path.as_ptr(), libc::S_IFIFO | 0o644, 0);
        if ret != 0 {
            libc::perror(fifo_path.as_ptr());
            panic!("Failed to create a FIFO file");
        }
    }

    t!(env::set_current_dir(td.path()));
    let mut ar = Builder::new(Vec::new());
    // append_path has a different logic for processing files, so we need to test it as well
    t!(ar.append_path("fifo"));
    t!(ar.append_dir_all("special", td.path()));
    // unfortunately, block device file cannot be created by non-root users
    // as a substitute, just test the file that exists on most Unix systems
    t!(env::set_current_dir("/dev/"));
    t!(ar.append_path("loop0"));
    // CI systems seem to have issues with creating a chr device
    t!(ar.append_path("null"));
    t!(ar.finish());
}

#[test]
fn header_size_overflow() {
    // maximal file size doesn't overflow anything
    let mut ar = Builder::new(Vec::new());
    let mut header = Header::new_gnu();
    header.set_size(u64::MAX);
    header.set_cksum();
    ar.append(&mut header, "x".as_bytes()).unwrap();
    let result = t!(ar.into_inner());
    let mut ar = Archive::new(&result[..]);
    let mut e = ar.entries().unwrap();
    let err = e.next().unwrap().err().unwrap();
    assert!(
        err.to_string().contains("size overflow"),
        "bad error: {}",
        err
    );

    // back-to-back entries that would overflow also don't panic
    let mut ar = Builder::new(Vec::new());
    let mut header = Header::new_gnu();
    header.set_size(1_000);
    header.set_cksum();
    ar.append(&mut header, &[0u8; 1_000][..]).unwrap();
    let mut header = Header::new_gnu();
    header.set_size(u64::MAX - 513);
    header.set_cksum();
    ar.append(&mut header, "x".as_bytes()).unwrap();
    let result = t!(ar.into_inner());
    let mut ar = Archive::new(&result[..]);
    let mut e = ar.entries().unwrap();
    e.next().unwrap().unwrap();
    let err = e.next().unwrap().err().unwrap();
    assert!(
        err.to_string().contains("size overflow"),
        "bad error: {}",
        err
    );
}
