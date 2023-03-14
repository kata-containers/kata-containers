// Copyright (c) 2018 Levente Kurusa
// Copyright (c) 2020 Ant Group
//
// SPDX-License-Identifier: Apache-2.0 or MIT
//

//! This module contains the implementation of the `freezer` cgroup subsystem.
//!
//! See the Kernel's documentation for more information about this subsystem, found at:
//!  [Documentation/cgroup-v1/freezer-subsystem.txt](https://www.kernel.org/doc/Documentation/cgroup-v1/freezer-subsystem.txt)
use std::io::{Read, Write};
use std::path::PathBuf;

use crate::error::ErrorKind::*;
use crate::error::*;

use crate::{ControllIdentifier, ControllerInternal, Controllers, Resources, Subsystem};

/// A controller that allows controlling the `freezer` subsystem of a Cgroup.
///
/// In essence, this subsystem allows the user to freeze and thaw (== "un-freeze") the processes in
/// the control group. This is done _transparently_ so that neither the parent, nor the children of
/// the processes can observe the freeze.
///
/// Note that if the control group is currently in the `Frozen` or `Freezing` state, then no
/// processes can be added to it.
#[derive(Debug, Clone)]
pub struct FreezerController {
    base: PathBuf,
    path: PathBuf,
    v2: bool,
}

/// The current state of the control group
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FreezerState {
    /// The processes in the control group are _not_ frozen.
    Thawed,
    /// The processes in the control group are in the processes of being frozen.
    Freezing,
    /// The processes in the control group are frozen.
    Frozen,
}

impl ControllerInternal for FreezerController {
    fn control_type(&self) -> Controllers {
        Controllers::Freezer
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

impl ControllIdentifier for FreezerController {
    fn controller_type() -> Controllers {
        Controllers::Freezer
    }
}

impl<'a> From<&'a Subsystem> for &'a FreezerController {
    fn from(sub: &'a Subsystem) -> &'a FreezerController {
        unsafe {
            match sub {
                Subsystem::Freezer(c) => c,
                _ => {
                    assert_eq!(1, 0);
                    let v = std::mem::MaybeUninit::uninit();
                    v.assume_init()
                }
            }
        }
    }
}

impl FreezerController {
    /// Contructs a new `FreezerController` with `root` serving as the root of the control group.
    pub fn new(root: PathBuf, v2: bool) -> Self {
        Self {
            base: root.clone(),
            path: root,
            v2,
        }
    }

    /// Freezes the processes in the control group.
    pub fn freeze(&self) -> Result<()> {
        let mut file_name = "freezer.state";
        let mut content = "FROZEN".to_string();
        if self.v2 {
            file_name = "cgroup.freeze";
            content = "1".to_string();
        }

        self.open_path(file_name, true).and_then(|mut file| {
            file.write_all(content.as_ref())
                .map_err(|e| Error::with_cause(WriteFailed(file_name.to_string(), content), e))
        })
    }

    /// Thaws, that is, unfreezes the processes in the control group.
    pub fn thaw(&self) -> Result<()> {
        let mut file_name = "freezer.state";
        let mut content = "THAWED".to_string();
        if self.v2 {
            file_name = "cgroup.freeze";
            content = "0".to_string();
        }
        self.open_path(file_name, true).and_then(|mut file| {
            file.write_all(content.as_ref())
                .map_err(|e| Error::with_cause(WriteFailed(file_name.to_string(), content), e))
        })
    }

    /// Retrieve the state of processes in the control group.
    pub fn state(&self) -> Result<FreezerState> {
        let mut file_name = "freezer.state";
        if self.v2 {
            file_name = "cgroup.freeze";
        }
        self.open_path(file_name, false).and_then(|mut file| {
            let mut s = String::new();
            let res = file.read_to_string(&mut s);
            match res {
                Ok(_) => match s.trim() {
                    "FROZEN" => Ok(FreezerState::Frozen),
                    "THAWED" => Ok(FreezerState::Thawed),
                    "1" => Ok(FreezerState::Frozen),
                    "0" => Ok(FreezerState::Thawed),
                    "FREEZING" => Ok(FreezerState::Freezing),
                    _ => Err(Error::new(ParseError)),
                },
                Err(e) => Err(Error::with_cause(ReadFailed(file_name.to_string()), e)),
            }
        })
    }
}
