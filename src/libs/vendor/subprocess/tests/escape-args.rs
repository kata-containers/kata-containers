extern crate subprocess;

use std::io::Read;
use std::path::PathBuf;
use subprocess::{Popen, PopenConfig, Redirection};

fn just_echo_path() -> String {
    let prog = PathBuf::from(&::std::env::args().next().unwrap());
    prog.parent()
        .unwrap()
        .join("../just-echo")
        .to_str()
        .unwrap()
        .to_owned()
}

#[test]
fn escape_args() {
    // This is mostly relevant for Windows: test whether
    // assemble_cmdline does a good job with arguments that require
    // escaping.
    for &arg in &[
        "x", "", " ", "  ", r" \ ", r" \\ ", r" \\\ ", r#"""#, r#""""#, r#"\"\\""#, "æ÷", "šđ",
        "本", "❤", "☃",
    ] {
        let mut p = Popen::create(
            &[just_echo_path().as_ref(), arg],
            PopenConfig {
                stdout: Redirection::Pipe,
                ..Default::default()
            },
        )
        .unwrap();
        let mut output = p.stdout.take().unwrap();
        let mut output_str = String::new();
        output.read_to_string(&mut output_str).unwrap();
        assert_eq!(output_str, arg);
    }
}
