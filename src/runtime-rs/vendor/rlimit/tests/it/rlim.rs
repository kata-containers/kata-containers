#![cfg(any(target_os = "fuchsia", target_os = "emscripten", target_os = "linux",))]

use std::fs;
use std::process::{Command, Stdio};

fn exec(cmd: &[&str]) -> String {
    let mut child = Command::new(cmd[0]);
    child.args(&cmd[1..]);
    child.stdout(Stdio::piped());
    let output = child.spawn().unwrap().wait_with_output().unwrap();
    String::from_utf8(output.stdout).unwrap()
}

const CPP_CODE: &str = r#"
    #include<iostream>
    #include<cstdint>
    #include<sys/resource.h>
    using namespace std;

    int main(){
        cout<<RLIM_INFINITY<<'\n';
        cout<<RLIM_SAVED_CUR<<'\n';
        cout<<RLIM_SAVED_MAX<<'\n';
        return 0;
    }
"#;

#[test]
fn rlim_value() {
    let cpp_path = "/tmp/__rlim_value_test.cpp";
    let exe_path = "/tmp/__rlim_value_test";
    fs::write(cpp_path, CPP_CODE).unwrap();

    exec(&["g++", cpp_path, "-std=c++11", "-o", exe_path]);

    let c_output = exec(&[exe_path]);

    let rs_output = format!(
        "{}\n{}\n{}\n",
        rlimit::INFINITY,
        rlimit::SAVED_CUR,
        rlimit::SAVED_MAX
    );

    assert_eq!(c_output, rs_output);

    fs::remove_file(cpp_path).ok();
    fs::remove_file(exe_path).ok();
}
