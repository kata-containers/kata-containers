// Copyright (c) 2018 Levente Kurusa
//
// SPDX-License-Identifier: Apache-2.0 or MIT
//

//! This module contains the implementation of the `cpuacct` cgroup subsystem.
//!
//! See the Kernel's documentation for more information about this subsystem, found at:
//!  [Documentation/cgroup-v1/cpuacct.txt](https://www.kernel.org/doc/Documentation/cgroup-v1/cpuacct.txt)
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;

use crate::error::ErrorKind::*;
use crate::error::*;

use crate::{ControllIdentifier, ControllerInternal, Controllers, Resources, Subsystem};

/// A controller that allows controlling the `cpuacct` subsystem of a Cgroup.
///
/// In essence, this control group provides accounting (hence the name `cpuacct`) for CPU usage of
/// the tasks in the control group.
#[derive(Debug, Clone)]
pub struct CpuAcctController {
    base: PathBuf,
    path: PathBuf,
}

/// Represents the statistics retrieved from the control group.
pub struct CpuAcct {
    /// Divides the time used by the tasks into `user` time and `system` time.
    pub stat: String,
    /// Total CPU time (in nanoseconds) spent by the tasks.
    pub usage: u64,
    /// Total CPU time (in nanoseconds) spent by the tasks, broken down by CPU and by whether the
    /// time spent is `user` time or `system` time.
    ///
    /// An example is as follows:
    /// ```text
    /// cpu user system
    /// 0 8348363768 0
    /// 1 8324369100 0
    /// 2 8598185449 0
    /// 3 8648262473 0
    /// ```
    pub usage_all: String,
    /// CPU time (in nanoseconds) spent by the tasks, broken down by each CPU.
    /// Times spent in each CPU are separated by a space.
    pub usage_percpu: String,
    /// As for `usage_percpu`, but the `system` time spent.
    pub usage_percpu_sys: String,
    /// As for `usage_percpu`, but the `user` time spent.
    pub usage_percpu_user: String,
    /// CPU time (in nanoseconds) spent by the tasks that counted for `system` time.
    pub usage_sys: u64,
    /// CPU time (in nanoseconds) spent by the tasks that counted for `user` time.
    pub usage_user: u64,
}

impl ControllerInternal for CpuAcctController {
    fn control_type(&self) -> Controllers {
        Controllers::CpuAcct
    }
    fn get_path(&self) -> &PathBuf {
        &self.path
    }
    fn get_path_mut(&mut self) -> &mut PathBuf {
        &mut self.path
    }
    fn get_base(&self) -> &PathBuf {
        &self.base
    }

    fn apply(&self, _res: &Resources) -> Result<()> {
        Ok(())
    }
}

impl ControllIdentifier for CpuAcctController {
    fn controller_type() -> Controllers {
        Controllers::CpuAcct
    }
}

impl<'a> From<&'a Subsystem> for &'a CpuAcctController {
    fn from(sub: &'a Subsystem) -> &'a CpuAcctController {
        unsafe {
            match sub {
                Subsystem::CpuAcct(c) => c,
                _ => {
                    assert_eq!(1, 0);
                    ::std::mem::uninitialized()
                }
            }
        }
    }
}

fn read_u64_from(mut file: File) -> Result<u64> {
    let mut string = String::new();
    let res = file.read_to_string(&mut string);
    match res {
        Ok(_) => match string.trim().parse() {
            Ok(e) => Ok(e),
            Err(e) => Err(Error::with_cause(ParseError, e)),
        },
        Err(e) => Err(Error::with_cause(ReadFailed, e)),
    }
}

fn read_string_from(mut file: File) -> Result<String> {
    let mut string = String::new();
    match file.read_to_string(&mut string) {
        Ok(_) => Ok(string.trim().to_string()),
        Err(e) => Err(Error::with_cause(ReadFailed, e)),
    }
}

impl CpuAcctController {
    /// Contructs a new `CpuAcctController` with `oroot` serving as the root of the control group.
    pub fn new(oroot: PathBuf) -> Self {
        let mut root = oroot;
        root.push(Self::controller_type().to_string());
        Self {
            base: root.clone(),
            path: root,
        }
    }

    /// Gathers the statistics that are available in the control group into a `CpuAcct` structure.
    pub fn cpuacct(&self) -> CpuAcct {
        CpuAcct {
            stat: self
                .open_path("cpuacct.stat", false)
                .and_then(|file| read_string_from(file))
                .unwrap_or("".to_string()),
            usage: self
                .open_path("cpuacct.usage", false)
                .and_then(|file| read_u64_from(file))
                .unwrap_or(0),
            usage_all: self
                .open_path("cpuacct.usage_all", false)
                .and_then(|file| read_string_from(file))
                .unwrap_or("".to_string()),
            usage_percpu: self
                .open_path("cpuacct.usage_percpu", false)
                .and_then(|file| read_string_from(file))
                .unwrap_or("".to_string()),
            usage_percpu_sys: self
                .open_path("cpuacct.usage_percpu_sys", false)
                .and_then(|file| read_string_from(file))
                .unwrap_or("".to_string()),
            usage_percpu_user: self
                .open_path("cpuacct.usage_percpu_user", false)
                .and_then(|file| read_string_from(file))
                .unwrap_or("".to_string()),
            usage_sys: self
                .open_path("cpuacct.usage_sys", false)
                .and_then(|file| read_u64_from(file))
                .unwrap_or(0),
            usage_user: self
                .open_path("cpuacct.usage_user", false)
                .and_then(|file| read_u64_from(file))
                .unwrap_or(0),
        }
    }

    /// Reset the statistics the kernel has gathered about the control group.
    pub fn reset(&self) -> Result<()> {
        self.open_path("cpuacct.usage", true).and_then(|mut file| {
            file.write_all(b"0")
                .map_err(|e| Error::with_cause(WriteFailed, e))
        })
    }
}
