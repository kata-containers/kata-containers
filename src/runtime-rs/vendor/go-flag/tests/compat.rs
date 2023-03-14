use std::borrow::Cow;
use std::io;
use std::process::Command;

#[test]
fn test_noflags() -> io::Result<()> {
    let commands = compile_commands("noflags")?;
    for command in &commands {
        let output = Command::new(command).output()?;
        assert!(output.status.success());
        assert_eq!(output.stdout(), "");

        let output = Command::new(command).args(&["foo", "bar", "-f"]).output()?;
        assert!(output.status.success());
        assert_eq!(output.stdout(), "foo\nbar\n-f\n");

        let output = Command::new(command).args(&["--", "-f", "foo"]).output()?;
        assert!(output.status.success());
        assert_eq!(output.stdout(), "-f\nfoo\n");

        let output = Command::new(command).args(&["-", "foo"]).output()?;
        assert!(output.status.success());
        assert_eq!(output.stdout(), "-\nfoo\n");

        let output = Command::new(command).args(&["-f", "foo"]).output()?;
        assert!(!output.status.success());
        assert!(output.stderr().contains("flag provided but not defined"));

        let output = Command::new(command).args(&["--f", "foo"]).output()?;
        assert!(!output.status.success());
        assert!(output.stderr().contains("flag provided but not defined"));

        let output = Command::new(command).args(&["---f", "foo"]).output()?;
        assert!(!output.status.success());
        assert!(output.stderr().contains("bad flag syntax"));
    }

    Ok(())
}

#[test]
fn test_someflags() -> io::Result<()> {
    let commands = compile_commands("someflags")?;

    for command in &commands {
        let output = Command::new(command).output()?;
        assert!(output.status.success());
        assert_eq!(output.stdout(), "force = false\nlines = 10\n");

        let output = Command::new(command).args(&["-f"]).output()?;
        assert!(output.status.success());
        assert_eq!(output.stdout(), "force = true\nlines = 10\n");

        let output = Command::new(command).args(&["-f", "--lines=20"]).output()?;
        assert!(output.status.success());
        assert_eq!(output.stdout(), "force = true\nlines = 20\n");
    }

    Ok(())
}

fn compile_commands(name: &str) -> io::Result<[String; 2]> {
    let go_cmdname = format!("examples/{}-go", name);
    let output = Command::new("go")
        .args(&["build", "-o"])
        .arg(&go_cmdname)
        .arg(&format!("examples/{}.go", name))
        .output()?;
    assert!(output.status.success());

    let rust_cmdname = format!("target/debug/examples/{}", name);
    let output = Command::new("cargo")
        .args(&["build", "--example", name])
        .output()?;
    assert!(output.status.success());

    Ok([rust_cmdname, go_cmdname])
}

trait OutputExt {
    fn stdout(&self) -> Cow<'_, str>;
    fn stderr(&self) -> Cow<'_, str>;
}

impl OutputExt for std::process::Output {
    fn stdout(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.stdout)
    }
    fn stderr(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.stderr)
    }
}
