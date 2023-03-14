// Copyright The ocicrypt Authors.
// SPDX-License-Identifier: Apache-2.0

use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Result};

use crate::utils::CommandExecuter;

#[derive(Debug)]
pub struct Runner {}

impl CommandExecuter for Runner {
    /// ExecuteCommand is used to execute a linux command line command and return the output of the command with an error if it exists.
    fn exec(&self, cmd: String, args: &[std::string::String], input: Vec<u8>) -> Result<Vec<u8>> {
        if cmd.is_empty() {
            return Err(anyhow!("keyprovider command name is empty"));
        }
        let mut child = Command::new(cmd)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("Failed to open stdin"))?;
        std::thread::spawn(move || {
            stdin
                .write_all(input.clone().as_mut_slice())
                .map_err(|_| anyhow!("Failed to write to stdin"))
        });
        let output = child.wait_with_output()?;

        Ok(output.stdout)
    }
}
