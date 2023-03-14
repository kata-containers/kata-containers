// Copyright 2020 Ant Group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use std::io::{Result, Write};
use std::process::{Command, Stdio};

pub fn exec(cmd: &str, output: bool, input: &[u8]) -> Result<String> {
    debug!("exec `{}`", cmd);
    let has_input = !input.is_empty();
    let mut basic_cmd = Command::new("sh");
    let mut cmd_object = basic_cmd.arg("-c").arg(cmd).env("RUST_BACKTRACE", "1");

    if has_input {
        cmd_object = cmd_object.stdin(Stdio::piped());
    } else {
        cmd_object = cmd_object.stdin(Stdio::null());
    }

    if output {
        cmd_object = cmd_object.stdout(Stdio::piped()).stderr(Stdio::piped());
    } else {
        cmd_object = cmd_object.stdout(Stdio::inherit()).stderr(Stdio::inherit())
    }
    let mut child = cmd_object.spawn()?;

    if has_input {
        let mut input_stream = child.stdin.take().unwrap();
        input_stream.write_all(input)?;
        drop(input_stream);
    }

    if output {
        let output = child.wait_with_output()?;
        if !output.status.success() {
            return Err(eother!("exit with non-zero status"));
        }
        let stdout = std::str::from_utf8(&output.stdout).map_err(|e| einval!(e))?;
        return Ok(stdout.to_string());
    }

    let status = child.wait()?;
    if !status.success() {
        return Err(eother!("exit with non-zero status"));
    }

    Ok(String::from(""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exec() {
        let val = exec("echo hello", true, b"").unwrap();
        assert_eq!(val, "hello\n");

        let val = exec("echo hello", false, b"").unwrap();
        assert_eq!(val, "");

        let val = exec("cat -", true, b"test").unwrap();
        assert_eq!(val, "test");

        let val = exec("cat -", false, b"test").unwrap();
        assert_eq!(val, "");
    }
}
