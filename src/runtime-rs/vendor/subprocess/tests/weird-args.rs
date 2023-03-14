extern crate subprocess;

use std::io::Read;
use std::path::Path;
use subprocess::{Popen, PopenConfig, Redirection};

fn just_echo_path() -> String {
    let prog = Path::new(&::std::env::args().next().unwrap()).to_owned();
    let prog = prog.parent().unwrap(); // dirname
    let prog = prog.parent().unwrap(); // parent dir
    prog.join("just-echo").to_str().unwrap().to_owned()
}

#[test]
fn weird_args() {
    // This is mostly relevant for Windows: test whether
    // assemble_cmdline does a good job with arguments with
    // metacharacters.
    for &arg in [
        "x", "", " ", "  ", r" \ ", r" \\ ", r" \\\ ", r#"""#, r#""""#, r#"\"\\""#,
    ]
    .iter()
    {
        println!("running {:?} {:?}", arg, just_echo_path());
        let mut p = Popen::create(
            &[just_echo_path(), arg.to_owned()],
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
