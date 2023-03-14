// Copyright (c) 2018 Levente Kurusa
// Copyright (c) 2020 Ant Group
//
// SPDX-License-Identifier: Apache-2.0 or MIT
//

//! This module contains the implementation of the `pids` cgroup subsystem.
//!
//! See the Kernel's documentation for more information about this subsystem, found at:
//!  [Documentation/cgroups-v1/pids.txt](https://www.kernel.org/doc/Documentation/cgroup-v1/pids.txt)
use std::io::{Read, Write};
use std::path::PathBuf;

use crate::error::ErrorKind::*;
use crate::error::*;

use crate::read_u64_from;
use crate::{
    parse_max_value, ControllIdentifier, ControllerInternal, Controllers, MaxValue, PidResources,
    Resources, Subsystem,
};

/// A controller that allows controlling the `pids` subsystem of a Cgroup.
#[derive(Debug, Clone)]
pub struct PidController {
    base: PathBuf,
    path: PathBuf,
    v2: bool,
}

impl ControllerInternal for PidController {
    fn control_type(&self) -> Controllers {
        Controllers::Pids
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

    fn is_v2(&self) -> bool {
        self.v2
    }

    fn apply(&self, res: &Resources) -> Result<()> {
        // get the resources that apply to this controller
        let pidres: &PidResources = &res.pid;

        // apply pid_max
        update_and_test!(
            self,
            set_pid_max,
            pidres.maximum_number_of_processes,
            get_pid_max
        );

        Ok(())
    }
}

// impl<'a> ControllIdentifier for &'a PidController {
//     fn controller_type() -> Controllers {
//         Controllers::Pids
//     }
// }

impl ControllIdentifier for PidController {
    fn controller_type() -> Controllers {
        Controllers::Pids
    }
}

impl<'a> From<&'a Subsystem> for &'a PidController {
    fn from(sub: &'a Subsystem) -> &'a PidController {
        unsafe {
            match sub {
                Subsystem::Pid(c) => c,
                _ => {
                    assert_eq!(1, 0);
                    let v = std::mem::MaybeUninit::uninit();
                    v.assume_init()
                }
            }
        }
    }
}

impl PidController {
    /// Constructors a new `PidController` instance, with `root` serving as the controller's root
    /// directory.
    pub fn new(root: PathBuf, v2: bool) -> Self {
        Self {
            base: root.clone(),
            path: root,
            v2,
        }
    }

    /// The number of times `fork` failed because the limit was hit.
    pub fn get_pid_events(&self) -> Result<u64> {
        self.open_path("pids.events", false).and_then(|mut file| {
            let mut string = String::new();
            match file.read_to_string(&mut string) {
                Ok(_) => match string.split_whitespace().nth(1) {
                    Some(elem) => match elem.parse() {
                        Ok(val) => Ok(val),
                        Err(e) => Err(Error::with_cause(ParseError, e)),
                    },
                    None => Err(Error::new(ParseError)),
                },
                Err(e) => Err(Error::with_cause(ReadFailed("pids.events".to_string()), e)),
            }
        })
    }

    /// The number of processes currently.
    pub fn get_pid_current(&self) -> Result<u64> {
        self.open_path("pids.current", false)
            .and_then(read_u64_from)
    }

    /// The maximum number of processes that can exist at one time in the control group.
    pub fn get_pid_max(&self) -> Result<MaxValue> {
        self.open_path("pids.max", false).and_then(|mut file| {
            let mut string = String::new();
            let res = file.read_to_string(&mut string);
            match res {
                Ok(_) => parse_max_value(&string),
                Err(e) => Err(Error::with_cause(ReadFailed("pids.max".to_string()), e)),
            }
        })
    }

    /// Set the maximum number of processes that can exist in this control group.
    ///
    /// Note that if `get_pid_current()` returns a higher number than what you
    /// are about to set (`max_pid`), then no processess will be killed. Additonally, attaching
    /// extra processes to a control group disregards the limit.
    pub fn set_pid_max(&self, max_pid: MaxValue) -> Result<()> {
        self.open_path("pids.max", true).and_then(|mut file| {
            let string_to_write = max_pid.to_string();
            match file.write_all(string_to_write.as_ref()) {
                Ok(_) => Ok(()),
                Err(e) => Err(Error::with_cause(
                    WriteFailed("pids.max".to_string(), format!("{:?}", max_pid)),
                    e,
                )),
            }
        })
    }
}
