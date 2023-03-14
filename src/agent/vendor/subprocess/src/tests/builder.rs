use std::borrow::Cow;
use std::env;
use std::fs::File;
use std::sync::Mutex;

use std::io::prelude::*;
use std::sync::MutexGuard;

use crate::{Exec, ExitStatus, NullFile, Redirection};

use lazy_static::lazy_static;
use tempfile::TempDir;

use crate::tests::common::read_whole_file;

#[test]
fn exec_join() {
    let status = Exec::cmd("true").join().unwrap();
    assert_eq!(status, ExitStatus::Exited(0));
}

#[test]
fn null_file() {
    let mut p = Exec::cmd("cat")
        .stdin(NullFile)
        .stdout(Redirection::Pipe)
        .popen()
        .unwrap();
    let (out, _) = p.communicate(None).unwrap();
    assert_eq!(out.unwrap(), "");
}

#[test]
fn stream_stdout() {
    let stream = Exec::cmd("printf").arg("foo").stream_stdout().unwrap();
    assert_eq!(read_whole_file(stream), "foo");
}

#[test]
fn stream_stderr() {
    let stream = Exec::cmd("sh")
        .args(&["-c", "printf foo >&2"])
        .stream_stderr()
        .unwrap();
    assert_eq!(read_whole_file(stream), "foo");
}

#[test]
fn stream_stdin() {
    let tmpdir = TempDir::new().unwrap();
    let tmpname = tmpdir.path().join("output");
    {
        let mut stream = Exec::cmd("cat")
            .stdout(File::create(&tmpname).unwrap())
            .stream_stdin()
            .unwrap();
        stream.write_all(b"foo").unwrap();
    }
    assert_eq!(read_whole_file(File::open(&tmpname).unwrap()), "foo");
}

#[test]
fn communicate_out() {
    let mut comm = Exec::cmd("printf").arg("foo").communicate().unwrap();
    assert_eq!(comm.read().unwrap(), (Some(b"foo".to_vec()), None));
}

#[test]
fn communicate_in_out() {
    let mut comm = Exec::cmd("cat").stdin("foo").communicate().unwrap();
    assert_eq!(comm.read().unwrap(), (Some(b"foo".to_vec()), None));
}

#[test]
fn capture_out() {
    let c = Exec::cmd("printf").arg("foo").capture().unwrap();
    assert_eq!(c.stdout_str(), "foo");
}

#[test]
fn capture_err() {
    let c = Exec::cmd("sh")
        .arg("-c")
        .arg("printf foo >&2")
        .stderr(Redirection::Pipe)
        .capture()
        .unwrap();
    assert_eq!(c.stderr_str(), "foo");
}

#[test]
fn capture_out_with_input_data1() {
    let c = Exec::cmd("cat").stdin("foo").capture().unwrap();
    assert_eq!(c.stdout_str(), "foo");
}

#[test]
fn capture_out_with_input_data2() {
    let c = Exec::cmd("cat").stdin(b"foo".to_vec()).capture().unwrap();
    assert_eq!(c.stdout_str(), "foo");
}

#[test]
fn exec_shell() {
    let stream = Exec::shell("printf foo").stream_stdout().unwrap();
    assert_eq!(read_whole_file(stream), "foo");
}

#[test]
fn pipeline_open() {
    let mut processes = { Exec::cmd("echo").arg("foo\nbar") | Exec::cmd("wc").arg("-l") }
        .stdout(Redirection::Pipe)
        .popen()
        .unwrap();
    let (output, _) = processes[1].communicate(None).unwrap();
    assert_eq!(output.unwrap().trim(), "2");
}

#[test]
fn pipeline_stream_out() {
    let stream = { Exec::cmd("echo").arg("foo\nbar") | Exec::cmd("wc").arg("-l") }
        .stream_stdout()
        .unwrap();
    assert_eq!(read_whole_file(stream).trim(), "2");
}

#[test]
fn pipeline_stream_in() {
    let tmpdir = TempDir::new().unwrap();
    let tmpname = tmpdir.path().join("output");
    {
        let mut stream = { Exec::cmd("cat") | Exec::cmd("wc").arg("-l") }
            .stdout(File::create(&tmpname).unwrap())
            .stream_stdin()
            .unwrap();
        stream.write_all(b"foo\nbar\nbaz\n").unwrap();
    }
    assert_eq!(read_whole_file(File::open(&tmpname).unwrap()).trim(), "3");
}

#[test]
fn pipeline_compose_pipelines() {
    let pipe1 = Exec::cmd("echo").arg("foo\nbar\nfoo") | Exec::cmd("sort");
    let pipe2 = Exec::cmd("uniq") | Exec::cmd("wc").arg("-l");
    let pipe = pipe1 | pipe2;
    let stream = pipe.stream_stdout().unwrap();
    assert_eq!(read_whole_file(stream).trim(), "2");
}

trait Crlf {
    fn to_crlf(self) -> Vec<u8>;
}
impl Crlf for Vec<u8> {
    #[cfg(windows)]
    fn to_crlf(self) -> Vec<u8> {
        self.iter()
            .flat_map(|&c| {
                if c == b'\n' {
                    vec![b'\r', b'\n']
                } else {
                    vec![c]
                }
            })
            .collect()
    }
    #[cfg(unix)]
    fn to_crlf(self) -> Vec<u8> {
        self
    }
}

#[test]
fn pipeline_communicate_out() {
    let pipe1 = Exec::cmd("echo").arg("foo\nbar\nfoo") | Exec::cmd("sort");
    let mut comm = pipe1.communicate().unwrap();
    assert_eq!(
        comm.read().unwrap(),
        (Some(b"bar\nfoo\nfoo\n".to_vec().to_crlf()), Some(vec![]))
    );
}

#[test]
fn pipeline_communicate_in_out() {
    let pipe1 = Exec::cmd("grep").arg("foo") | Exec::cmd("sort");
    let mut comm = pipe1.stdin("foobar\nbaz\nfoo\n").communicate().unwrap();
    let (out, _err) = comm.read().unwrap();
    assert_eq!(out, Some(b"foo\nfoobar\n".to_vec().to_crlf()));
}

#[test]
fn pipeline_capture() {
    let c = { Exec::cmd("cat") | Exec::shell("wc -l") }
        .stdin("foo\nbar\nbaz\n")
        .capture()
        .unwrap();
    assert_eq!(c.stdout_str().trim(), "3");
    assert_eq!(c.stderr_str().trim(), "");
}

#[test]
fn pipeline_capture_error_1() {
    let c = {
        Exec::cmd("sh")
            .arg("-c")
            .arg("echo foo >&2; printf 'bar\nbaz\n'")
            | Exec::shell("wc -l")
    }
    .capture()
    .unwrap();
    assert_eq!(c.stdout_str().trim(), "2");
    assert_eq!(c.stderr_str().trim(), "foo");
}

#[test]
fn pipeline_capture_error_2() {
    let c = {
        Exec::cmd("cat")
            | Exec::cmd("sh")
                .arg("-c")
                .arg("cat; echo foo >&2; printf 'four\nfive\n'")
            | Exec::cmd("sh").arg("-c").arg("echo bar >&2; cat")
            | Exec::shell("wc -l")
    }
    .stdin("one\ntwo\nthree\n")
    .capture()
    .unwrap();
    assert_eq!(c.stdout_str().trim(), "5");
    assert!(
        c.stderr_str().trim() == "foo\nbar" || c.stderr_str().trim() == "bar\nfoo",
        "got {:?}",
        c.stderr_str()
    );
}

#[test]
fn pipeline_join() {
    let status = (Exec::cmd("true") | Exec::cmd("true")).join().unwrap();
    assert_eq!(status, ExitStatus::Exited(0));

    let status = (Exec::cmd("false") | Exec::cmd("true")).join().unwrap();
    assert_eq!(status, ExitStatus::Exited(0));

    let status = (Exec::cmd("true") | Exec::cmd("false")).join().unwrap();
    assert_eq!(status, ExitStatus::Exited(1));
}

#[test]
fn pipeline_invalid_1() {
    let p = (Exec::cmd("echo").arg("foo") | Exec::cmd("no-such-command")).join();
    assert!(p.is_err());
}

#[test]
fn pipeline_invalid_2() {
    let p = (Exec::cmd("no-such-command") | Exec::cmd("echo").arg("foo")).join();
    assert!(p.is_err());
}

#[test]
#[should_panic]
fn reject_input_data_popen() {
    Exec::cmd("true").stdin("xxx").popen().unwrap();
}

#[test]
#[should_panic]
fn reject_input_data_join() {
    Exec::cmd("true").stdin("xxx").join().unwrap();
}

#[test]
#[should_panic]
fn reject_input_data_stream_stdout() {
    Exec::cmd("true").stdin("xxx").stream_stdout().unwrap();
}

#[test]
#[should_panic]
fn reject_input_data_stream_stderr() {
    Exec::cmd("true").stdin("xxx").stream_stderr().unwrap();
}

#[test]
#[should_panic]
fn reject_input_data_stream_stdin() {
    Exec::cmd("true").stdin("xxx").stream_stdin().unwrap();
}

#[test]
fn env_set() {
    assert!(Exec::cmd("sh")
        .args(&["-c", r#"test "$SOMEVAR" = "foo""#])
        .env("SOMEVAR", "foo")
        .join()
        .unwrap()
        .success());
}

#[test]
fn env_extend() {
    assert!(Exec::cmd("sh")
        .args(&["-c", r#"test "$VAR1" = "foo" && test "$VAR2" = "bar""#])
        .env_extend(&[("VAR1", "foo"), ("VAR2", "bar")])
        .join()
        .unwrap()
        .success());
}

lazy_static! {
    static ref MUTATE_ENV: Mutex<()> = Mutex::new(());
}

struct TmpEnvVar<'a> {
    varname: &'static str,
    #[allow(dead_code)]
    mutate_guard: MutexGuard<'a, ()>,
}

impl<'a> TmpEnvVar<'a> {
    fn new(varname: &'static str) -> TmpEnvVar<'a> {
        TmpEnvVar {
            varname,
            mutate_guard: MUTATE_ENV.lock().unwrap(),
        }
    }
}

impl Drop for TmpEnvVar<'_> {
    fn drop(&mut self) {
        env::remove_var(self.varname);
    }
}

fn tmp_env_var<'a>(varname: &'static str, tmp_value: &'static str) -> TmpEnvVar<'a> {
    env::set_var(varname, tmp_value);
    TmpEnvVar::new(varname)
}

#[test]
fn env_inherit() {
    // use a unique name to avoid interference with other tests
    let varname = "TEST_ENV_INHERIT_VARNAME";
    let _guard = tmp_env_var(varname, "inherited");
    assert!(Exec::cmd("sh")
        .args(&["-c", &format!(r#"test "${}" = "inherited""#, varname)])
        .join()
        .unwrap()
        .success());
}

#[test]
fn env_inherit_set() {
    // use a unique name to avoid interference with other tests
    let varname = "TEST_ENV_INHERIT_SET_VARNAME";
    let _guard = tmp_env_var(varname, "inherited");
    assert!(Exec::cmd("sh")
        .args(&["-c", &format!(r#"test "${}" = "new""#, varname)])
        .env(varname, "new")
        .join()
        .unwrap()
        .success());
}

// XXX move tests under the builder module so we can call
// Exec::display_escape() instead of copying it.
fn display_escape(s: &str) -> Cow<'_, str> {
    fn nice_char(c: char) -> bool {
        match c {
            '-' | '_' | '.' | ',' | '/' => true,
            c if c.is_ascii_alphanumeric() => true,
            _ => false,
        }
    }
    if !s.chars().all(nice_char) {
        Cow::Owned(format!("'{}'", s.replace("'", r#"'\''"#)))
    } else {
        Cow::Borrowed(s)
    }
}

#[test]
fn exec_to_string() {
    let _guard = MUTATE_ENV.lock().unwrap();
    let cmd = Exec::cmd("sh")
        .arg("arg1")
        .arg("don't")
        .arg("arg3 arg4")
        .arg("?")
        .arg(" ") // regular space
        .arg("\u{009c}"); // STRING TERMINATOR
    assert_eq!(
        format!("{:?}", cmd),
        "Exec { sh arg1 'don'\\''t' 'arg3 arg4' '?' ' ' '\u{009c}' }"
    );
    let cmd = cmd.env("foo", "bar");
    assert_eq!(
        format!("{:?}", cmd),
        "Exec { foo=bar sh arg1 'don'\\''t' 'arg3 arg4' '?' ' ' '\u{009c}' }"
    );
    let cmd = cmd.env("bar", "baz");
    assert_eq!(
        format!("{:?}", cmd),
        "Exec { foo=bar bar=baz sh arg1 'don'\\''t' 'arg3 arg4' '?' ' ' '\u{009c}' }"
    );
    let cmd = cmd.env_clear();
    assert_eq!(
        format!("{:?}", cmd),
        format!(
            "Exec {{ {} sh arg1 'don'\\''t' 'arg3 arg4' '?' ' ' '\u{009c}' }}",
            env::vars()
                .map(|(k, _)| format!("{}=", display_escape(&k)))
                .collect::<Vec<_>>()
                .join(" ")
        )
    );
}

#[test]
fn pipeline_to_string() {
    let pipeline = { Exec::cmd("command with space").arg("arg") | Exec::cmd("wc").arg("-l") };
    assert_eq!(
        format!("{:?}", pipeline),
        "Pipeline { 'command with space' arg | wc -l }"
    )
}
