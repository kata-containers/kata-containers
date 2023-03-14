// Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Wrapper for interpreting syscall exit codes.

use std::os::raw::c_int;

/// Wrapper to interpret syscall exit codes and provide a rustacean `io::Result`.
pub struct SyscallReturnCode(pub c_int);

impl SyscallReturnCode {
    /// Returns the last OS error if value is -1 or Ok(value) otherwise.
    pub fn into_result(self) -> std::io::Result<c_int> {
        if self.0 == -1 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(self.0)
        }
    }
    /// Returns the last OS error if value is -1 or Ok(()) otherwise.
    pub fn into_empty_result(self) -> std::io::Result<()> {
        self.into_result().map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syscall_ops() {
        let mut syscall_code = SyscallReturnCode(1);
        match syscall_code.into_result() {
            Ok(_value) => (),
            _ => unreachable!(),
        }

        syscall_code = SyscallReturnCode(-1);
        assert!(syscall_code.into_result().is_err());

        syscall_code = SyscallReturnCode(1);
        match syscall_code.into_empty_result() {
            Ok(()) => (),
            _ => unreachable!(),
        }

        syscall_code = SyscallReturnCode(-1);
        assert!(syscall_code.into_empty_result().is_err());
    }
}
