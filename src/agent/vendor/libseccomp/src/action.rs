// SPDX-License-Identifier: Apache-2.0 or MIT
//
// Copyright 2021 Sony Group Corporation
//

use crate::error::ErrorKind::*;
use crate::error::{Result, SeccompError};
use libseccomp_sys::*;
use std::convert::TryInto;

/// Represents an action to be taken on a filter rule match in the libseccomp.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ScmpAction {
    /// Kills the process.
    KillProcess,
    /// Kills the thread.
    KillThread,
    /// Throws a SIGSYS signal.
    Trap,
    /// Triggers a userspace notification.
    /// NOTE: This action is only usable when the libseccomp API level 6
    /// or higher is supported.
    Notify,
    /// Returns the specified error code.
    /// NOTE: You can only use integers from 0 to `u16::MAX`.
    Errno(i32),
    /// Notifies a tracing process with the specified value.
    Trace(u16),
    /// Allows the syscall to be executed after the action has been logged.
    Log,
    /// Allows the syscall to be executed.
    Allow,
}

impl ScmpAction {
    pub(crate) fn to_sys(self) -> u32 {
        match self {
            Self::KillProcess => SCMP_ACT_KILL_PROCESS,
            Self::KillThread => SCMP_ACT_KILL_THREAD,
            Self::Trap => SCMP_ACT_TRAP,
            Self::Notify => SCMP_ACT_NOTIFY,
            Self::Errno(x) => SCMP_ACT_ERRNO(x as u16),
            Self::Trace(x) => SCMP_ACT_TRACE(x),
            Self::Log => SCMP_ACT_LOG,
            Self::Allow => SCMP_ACT_ALLOW,
        }
    }

    pub(crate) fn from_sys(val: u32) -> Result<Self> {
        match val & SCMP_ACT_MASK {
            SCMP_ACT_KILL_PROCESS => Ok(Self::KillProcess),
            SCMP_ACT_KILL_THREAD => Ok(Self::KillThread),
            SCMP_ACT_TRAP => Ok(Self::Trap),
            SCMP_ACT_NOTIFY => Ok(Self::Notify),
            SCMP_ACT_ERRNO_MASK => Ok(Self::Errno(val as u16 as i32)),
            SCMP_ACT_TRACE_MASK => Ok(Self::Trace(val as u16)),
            SCMP_ACT_LOG => Ok(Self::Log),
            SCMP_ACT_ALLOW => Ok(Self::Allow),
            _ => Err(SeccompError::new(ParseError)),
        }
    }

    /// Converts string seccomp action to `ScmpAction`.
    ///
    /// # Arguments
    ///
    /// * `action` - A string action, e.g. `SCMP_ACT_*`.
    ///
    /// See the [`seccomp_rule_add(3)`] man page for details on valid action values.
    ///
    /// [`seccomp_rule_add(3)`]: https://www.man7.org/linux/man-pages/man3/seccomp_rule_add.3.html
    ///
    /// # Errors
    ///
    /// If an invalid action is specified or a value on `"SCMP_ACT_TRACE"` is not in the
    /// range from 0 to `u16::MAX`, an error will be returned.
    pub fn from_str(action: &str, val: Option<i32>) -> Result<Self> {
        match action {
            "SCMP_ACT_KILL_PROCESS" => Ok(Self::KillProcess),
            "SCMP_ACT_KILL_THREAD" | "SCMP_ACT_KILL" => Ok(Self::KillThread),
            "SCMP_ACT_TRAP" => Ok(Self::Trap),
            "SCMP_ACT_NOTIFY" => Ok(Self::Notify),
            "SCMP_ACT_ERRNO" => match val {
                Some(v) => Ok(Self::Errno(v)),
                None => Err(SeccompError::new(ParseError)),
            },
            "SCMP_ACT_TRACE" => match val {
                Some(v) => Ok(Self::Trace(v.try_into()?)),
                None => Err(SeccompError::new(ParseError)),
            },
            "SCMP_ACT_LOG" => Ok(Self::Log),
            "SCMP_ACT_ALLOW" => Ok(Self::Allow),
            _ => Err(SeccompError::new(ParseError)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_action() {
        let test_data = [
            ("SCMP_ACT_KILL_PROCESS", ScmpAction::KillProcess),
            ("SCMP_ACT_KILL_THREAD", ScmpAction::KillThread),
            ("SCMP_ACT_KILL", ScmpAction::KillThread),
            ("SCMP_ACT_TRAP", ScmpAction::Trap),
            ("SCMP_ACT_NOTIFY", ScmpAction::Notify),
            ("SCMP_ACT_ERRNO", ScmpAction::Errno(10)),
            ("SCMP_ACT_TRACE", ScmpAction::Trace(10)),
            ("SCMP_ACT_LOG", ScmpAction::Log),
            ("SCMP_ACT_ALLOW", ScmpAction::Allow),
        ];

        for data in test_data {
            if data.0 == "SCMP_ACT_ERRNO" || data.0 == "SCMP_ACT_TRACE" {
                assert_eq!(
                    ScmpAction::from_sys(ScmpAction::from_str(data.0, Some(10)).unwrap().to_sys())
                        .unwrap(),
                    data.1
                );
            } else {
                assert_eq!(
                    ScmpAction::from_sys(ScmpAction::from_str(data.0, None).unwrap().to_sys())
                        .unwrap(),
                    data.1
                );
            }
        }
        assert!(ScmpAction::from_str("SCMP_ACT_ERRNO", None).is_err());
        assert!(ScmpAction::from_str("SCMP_ACT_TRACE", None).is_err());
        assert!(ScmpAction::from_str("SCMP_INVALID_FLAG", None).is_err());
        assert!(ScmpAction::from_sys(0x00010000).is_err());
    }
}
