// Copyright The ocicrypt Authors.
// SPDX-License-Identifier: Apache-2.0

use std::fmt::Debug;

use anyhow::Result;

#[cfg(feature = "keywrap-keyprovider-cmd")]
pub mod runner;

#[cfg(feature = "keywrap-keyprovider-grpc")]
pub mod grpc;

#[cfg(feature = "keywrap-keyprovider-ttrpc")]
pub mod ttrpc;

/// CommandExecuter trait which requires implementation for command exec, first argument is the command name, like /usr/bin/<command-name>,
/// the second is the list of args to pass to it
pub trait CommandExecuter: Send + Sync {
    fn exec(&self, cmd: String, args: &[String], input: Vec<u8>) -> Result<Vec<u8>>;
}

impl Debug for dyn CommandExecuter {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "CommandExecuter")
    }
}

impl<W: CommandExecuter + ?Sized> CommandExecuter for Box<W> {
    #[inline]
    fn exec(&self, cmd: String, args: &[std::string::String], input: Vec<u8>) -> Result<Vec<u8>> {
        (**self).exec(cmd, args, input)
    }
}
