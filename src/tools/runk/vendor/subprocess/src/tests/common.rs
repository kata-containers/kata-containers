use tempfile::TempDir;

use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::Write;
use std::io::{self, Read};
use std::time::Duration;

use crate::{ExitStatus, Popen, PopenConfig, PopenError, Redirection};

pub fn read_whole_file<T: Read>(mut f: T) -> String {
    let mut content = String::new();
    f.read_to_string(&mut content).unwrap();
    content
}

#[test]
fn good_cmd() {
    let mut p = Popen::create(&["true"], PopenConfig::default()).unwrap();
    assert!(p.wait().unwrap().success());
}

#[test]
fn bad_cmd() {
    let result = Popen::create(&["nosuchcommand"], PopenConfig::default());
    assert!(result.is_err());
}

#[test]
fn reject_empty_argv() {
    let test = Popen::create(&[""; 0], PopenConfig::default());
    if let Err(PopenError::LogicError(..)) = test {
    } else {
        assert!(false, "didn't get LogicError for empty argv");
    }
}

#[test]
fn err_exit() {
    let mut p = Popen::create(&["sh", "-c", "exit 13"], PopenConfig::default()).unwrap();
    assert_eq!(p.wait().unwrap(), ExitStatus::Exited(13));
}

#[test]
fn terminate() {
    let mut p = Popen::create(&["sleep", "1000"], PopenConfig::default()).unwrap();
    p.terminate().unwrap();
    p.wait().unwrap();
}

#[test]
fn terminate_twice() {
    use std::thread;
    use std::time::Duration;

    let mut p = Popen::create(&["sleep", "1000"], PopenConfig::default()).unwrap();
    p.terminate().unwrap();
    thread::sleep(Duration::from_millis(100));
    p.terminate().unwrap();
}

#[test]
fn read_from_stdout() {
    let mut p = Popen::create(
        &["echo", "foo"],
        PopenConfig {
            stdout: Redirection::Pipe,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(read_whole_file(p.stdout.take().unwrap()), "foo\n");
    assert!(p.wait().unwrap().success());
}

#[test]
fn input_from_file() {
    let tmpdir = TempDir::new().unwrap();
    let tmpname = tmpdir.path().join("input");
    {
        let mut outfile = File::create(&tmpname).unwrap();
        outfile.write_all(b"foo").unwrap();
    }
    let mut p = Popen::create(
        &["cat", tmpname.to_str().unwrap()],
        PopenConfig {
            stdin: Redirection::File(File::open(&tmpname).unwrap()),
            stdout: Redirection::Pipe,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(read_whole_file(p.stdout.take().unwrap()), "foo");
    assert!(p.wait().unwrap().success());
}

#[test]
fn output_to_file() {
    let tmpdir = TempDir::new().unwrap();
    let tmpname = tmpdir.path().join("output");
    let outfile = File::create(&tmpname).unwrap();
    let mut p = Popen::create(
        &["printf", "foo"],
        PopenConfig {
            stdout: Redirection::File(outfile),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(p.wait().unwrap().success());
    assert_eq!(read_whole_file(File::open(&tmpname).unwrap()), "foo");
}

#[test]
fn input_output_from_file() {
    let tmpdir = TempDir::new().unwrap();
    let tmpname_in = tmpdir.path().join("input");
    let tmpname_out = tmpdir.path().join("output");
    {
        let mut f = File::create(&tmpname_in).unwrap();
        f.write_all(b"foo").unwrap();
    }
    let mut p = Popen::create(
        &["cat"],
        PopenConfig {
            stdin: Redirection::File(File::open(&tmpname_in).unwrap()),
            stdout: Redirection::File(File::create(&tmpname_out).unwrap()),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(p.wait().unwrap().success());
    assert_eq!(read_whole_file(File::open(&tmpname_out).unwrap()), "foo");
}

#[test]
fn write_to_subprocess() {
    let tmpdir = TempDir::new().unwrap();
    let tmpname = tmpdir.path().join("output");
    let mut p = Popen::create(
        &[r"uniq", "-", tmpname.to_str().unwrap()],
        PopenConfig {
            stdin: Redirection::Pipe,
            ..Default::default()
        },
    )
    .unwrap();
    p.stdin
        .take()
        .unwrap()
        .write_all(b"foo\nfoo\nbar\n")
        .unwrap();
    assert_eq!(p.wait().unwrap(), ExitStatus::Exited(0));
    assert_eq!(read_whole_file(File::open(tmpname).unwrap()), "foo\nbar\n");
}

#[test]
fn communicate_input() {
    let tmpdir = TempDir::new().unwrap();
    let tmpname = tmpdir.path().join("input");
    let mut p = Popen::create(
        &["cat"],
        PopenConfig {
            stdin: Redirection::Pipe,
            stdout: Redirection::File(File::create(&tmpname).unwrap()),
            ..Default::default()
        },
    )
    .unwrap();
    if let (None, None) = p.communicate_bytes(Some(b"hello world")).unwrap() {
    } else {
        assert!(false);
    }
    assert!(p.wait().unwrap().success());
    assert_eq!(
        read_whole_file(File::open(&tmpname).unwrap()),
        "hello world"
    );
}

#[test]
fn communicate_output() {
    let mut p = Popen::create(
        &["sh", "-c", "echo foo; echo bar >&2"],
        PopenConfig {
            stdout: Redirection::Pipe,
            stderr: Redirection::Pipe,
            ..Default::default()
        },
    )
    .unwrap();
    if let (Some(out), Some(err)) = p.communicate_bytes(None).unwrap() {
        assert_eq!(out, b"foo\n");
        assert_eq!(err, b"bar\n");
    } else {
        assert!(false);
    }
    assert!(p.wait().unwrap().success());
}

#[test]
fn communicate_input_output() {
    let mut p = Popen::create(
        &["sh", "-c", "cat; echo foo >&2"],
        PopenConfig {
            stdin: Redirection::Pipe,
            stdout: Redirection::Pipe,
            stderr: Redirection::Pipe,
            ..Default::default()
        },
    )
    .unwrap();
    if let (Some(out), Some(err)) = p.communicate_bytes(Some(b"hello world")).unwrap() {
        assert_eq!(out, b"hello world");
        assert_eq!(err, b"foo\n");
    } else {
        assert!(false);
    }
    assert!(p.wait().unwrap().success());
}

#[test]
fn communicate_input_output_long() {
    let mut p = Popen::create(
        &["sh", "-c", "cat; printf '%100000s' '' >&2"],
        PopenConfig {
            stdin: Redirection::Pipe,
            stdout: Redirection::Pipe,
            stderr: Redirection::Pipe,
            ..Default::default()
        },
    )
    .unwrap();
    let input = [65u8; 1_000_000];
    if let (Some(out), Some(err)) = p.communicate_bytes(Some(&input)).unwrap() {
        assert_eq!(&out[..], &input[..]);
        assert_eq!(&err[..], &[32u8; 100_000][..]);
    } else {
        assert!(false);
    }
    assert!(p.wait().unwrap().success());
}

#[test]
fn communicate_timeout() {
    let mut p = Popen::create(
        &["sh", "-c", "printf foo; sleep 1"],
        PopenConfig {
            stdout: Redirection::Pipe,
            stderr: Redirection::Pipe,
            ..Default::default()
        },
    )
    .unwrap();
    match p
        .communicate_start(None)
        .limit_time(Duration::from_millis(100))
        .read()
    {
        Err(e) => {
            assert_eq!(e.kind(), io::ErrorKind::TimedOut);
            assert_eq!(e.capture, (Some(b"foo".to_vec()), Some(vec![])));
        }
        other => panic!("unexpected result {:?}", other),
    }
    p.kill().unwrap();
}

#[test]
fn communicate_size_limit_small() {
    let mut p = Popen::create(
        &["sh", "-c", "printf '%5s' a"],
        PopenConfig {
            stdout: Redirection::Pipe,
            stderr: Redirection::Pipe,
            ..Default::default()
        },
    )
    .unwrap();
    let mut comm = p.communicate_start(None).limit_size(2);
    assert_eq!(comm.read().unwrap(), (Some(vec![32; 2]), Some(vec![])));
    assert_eq!(comm.read().unwrap(), (Some(vec![32; 2]), Some(vec![])));
    assert_eq!(comm.read().unwrap(), (Some(vec!['a' as u8]), Some(vec![])));
    p.kill().unwrap();
}

fn check_vec(v: Option<Vec<u8>>, size: usize, content: u8) {
    assert_eq!(v.as_ref().unwrap().len(), size);
    assert!(v.as_ref().unwrap().iter().all(|&c| c == content));
}

#[test]
fn communicate_size_limit_large() {
    let mut p = Popen::create(
        &["sh", "-c", "printf '%20001s' a"],
        PopenConfig {
            stdout: Redirection::Pipe,
            stderr: Redirection::Pipe,
            ..Default::default()
        },
    )
    .unwrap();
    let mut comm = p.communicate_start(None).limit_size(10_000);

    let (out, err) = comm.read().unwrap();
    check_vec(out, 10_000, 32);
    assert_eq!(err, Some(vec![]));

    let (out, err) = comm.read().unwrap();
    check_vec(out, 10_000, 32);
    assert_eq!(err, Some(vec![]));

    assert_eq!(comm.read().unwrap(), (Some(vec!['a' as u8]), Some(vec![])));
    p.kill().unwrap();
}

#[test]
fn communicate_size_limit_different_sizes() {
    let mut p = Popen::create(
        &["sh", "-c", "printf '%20001s' a"],
        PopenConfig {
            stdout: Redirection::Pipe,
            stderr: Redirection::Pipe,
            ..Default::default()
        },
    )
    .unwrap();
    let comm = p.communicate_start(None);

    let mut comm = comm.limit_size(100);
    let (out, err) = comm.read().unwrap();
    check_vec(out, 100, 32);
    assert_eq!(err, Some(vec![]));

    let mut comm = comm.limit_size(1_000);
    let (out, err) = comm.read().unwrap();
    check_vec(out, 1_000, 32);
    assert_eq!(err, Some(vec![]));

    let mut comm = comm.limit_size(10_000);
    let (out, err) = comm.read().unwrap();
    check_vec(out, 10_000, 32);
    assert_eq!(err, Some(vec![]));

    let mut comm = comm.limit_size(8_900);
    let (out, err) = comm.read().unwrap();
    check_vec(out, 8_900, 32);
    assert_eq!(err, Some(vec![]));

    assert_eq!(comm.read().unwrap(), (Some(vec!['a' as u8]), Some(vec![])));
    assert_eq!(comm.read().unwrap(), (Some(vec![]), Some(vec![])));
    p.kill().unwrap();
}

#[test]
fn null_byte_in_cmd() {
    let try_p = Popen::create(&["echo\0foo"], PopenConfig::default());
    assert!(try_p.is_err());
}

#[test]
fn merge_err_to_out_pipe() {
    let mut p = Popen::create(
        &["sh", "-c", "echo foo; echo bar >&2"],
        PopenConfig {
            stdout: Redirection::Pipe,
            stderr: Redirection::Merge,
            ..Default::default()
        },
    )
    .unwrap();
    if let (Some(out), None) = p.communicate_bytes(None).unwrap() {
        assert_eq!(out, b"foo\nbar\n");
    } else {
        assert!(false);
    }
    assert!(p.wait().unwrap().success());
}

#[test]
fn merge_out_to_err_pipe() {
    let mut p = Popen::create(
        &["sh", "-c", "echo foo; echo bar >&2"],
        PopenConfig {
            stdout: Redirection::Merge,
            stderr: Redirection::Pipe,
            ..Default::default()
        },
    )
    .unwrap();
    if let (None, Some(err)) = p.communicate_bytes(None).unwrap() {
        assert_eq!(err, b"foo\nbar\n");
    } else {
        assert!(false);
    }
    assert!(p.wait().unwrap().success());
}

#[test]
fn merge_err_to_out_file() {
    let tmpdir = TempDir::new().unwrap();
    let tmpname = tmpdir.path().join("output");
    let mut p = Popen::create(
        &["sh", "-c", "printf foo; printf bar >&2"],
        PopenConfig {
            stdout: Redirection::File(File::create(&tmpname).unwrap()),
            stderr: Redirection::Merge,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(p.wait().unwrap().success());
    assert_eq!(read_whole_file(File::open(&tmpname).unwrap()), "foobar");
}

#[test]
fn simple_pipe() {
    let mut c1 = Popen::create(
        &["printf", "foo\\nbar\\nbaz\\n"],
        PopenConfig {
            stdout: Redirection::Pipe,
            ..Default::default()
        },
    )
    .unwrap();
    let mut c2 = Popen::create(
        &["wc", "-l"],
        PopenConfig {
            stdin: Redirection::File(c1.stdout.take().unwrap()),
            stdout: Redirection::Pipe,
            ..Default::default()
        },
    )
    .unwrap();
    let (wcout, _) = c2.communicate(None).unwrap();
    assert_eq!(wcout.unwrap().trim(), "3");
}

#[test]
fn wait_timeout() {
    let mut p = Popen::create(&["sleep", "0.5"], PopenConfig::default()).unwrap();
    let ret = p.wait_timeout(Duration::from_millis(100)).unwrap();
    assert!(ret.is_none());
    let ret = p.wait_timeout(Duration::from_millis(450)).unwrap();
    assert_eq!(ret, Some(ExitStatus::Exited(0)));
}

#[test]
fn setup_executable() {
    let mut p = Popen::create(
        &["foobar", "-c", r#"printf %s "$0""#],
        PopenConfig {
            executable: Some(OsStr::new("sh").to_owned()),
            stdout: Redirection::Pipe,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(read_whole_file(p.stdout.take().unwrap()), "foobar");
}

#[test]
fn env_add() {
    let mut env = PopenConfig::current_env();
    env.push((OsString::from("SOMEVAR"), OsString::from("foo")));
    let mut p = Popen::create(
        &["sh", "-c", r#"test "$SOMEVAR" = "foo""#],
        PopenConfig {
            env: Some(env),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(p.wait().unwrap().success());
}

#[test]
fn env_dup() {
    let dups = vec![
        (OsString::from("SOMEVAR"), OsString::from("foo")),
        (OsString::from("SOMEVAR"), OsString::from("bar")),
    ];
    let mut p = Popen::create(
        &["sh", "-c", r#"test "$SOMEVAR" = "bar""#],
        PopenConfig {
            stdout: Redirection::Pipe,
            env: Some(dups),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(p.wait().unwrap().success());
}

#[test]
fn cwd() {
    let tmpdir = TempDir::new().unwrap();
    let tmpdir_name = tmpdir.path().as_os_str().to_owned();

    // Test that CWD works by cwd-ing into an empty temporary
    // directory and creating a file there.  Trying to print the
    // directory's name and compare it to tmpdir fails due to MinGW
    // interference on Windows and symlinks on Unix.

    Popen::create(
        &["touch", "here"],
        PopenConfig {
            stdout: Redirection::Pipe,
            cwd: Some(tmpdir_name),
            ..Default::default()
        },
    )
    .unwrap();

    assert!(tmpdir.path().join("here").exists());
}

#[test]
fn failed_cwd() {
    use crate::popen::PopenError::IoError;
    let ret = Popen::create(
        &["anything"],
        PopenConfig {
            stdout: Redirection::Pipe,
            cwd: Some("/nosuchdir".into()),
            ..Default::default()
        },
    );
    let err_num = match ret {
        Err(IoError(e)) => e.raw_os_error().unwrap_or(-1),
        _ => panic!("expected error return"),
    };
    assert_eq!(err_num, libc::ENOENT);
}
