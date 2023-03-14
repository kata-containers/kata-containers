// Copyright (c) 2020 Ant Group
//
// SPDX-License-Identifier: Apache-2.0 or MIT
//

//! This module contains the implementation of the `systemd` cgroup subsystem.
//!
use std::path::PathBuf;

use crate::error::*;

use crate::{ControllIdentifier, ControllerInternal, Controllers, Resources, Subsystem};

/// A controller that allows controlling the `systemd` subsystem of a Cgroup.
///
#[derive(Debug, Clone)]
pub struct SystemdController {
    base: PathBuf,
    path: PathBuf,
    _v2: bool,
}

impl ControllerInternal for SystemdController {
    fn control_type(&self) -> Controllers {
        Controllers::Systemd
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

impl ControllIdentifier for SystemdController {
    fn controller_type() -> Controllers {
        Controllers::Systemd
    }
}

impl<'a> From<&'a Subsystem> for &'a SystemdController {
    fn from(sub: &'a Subsystem) -> &'a SystemdController {
        unsafe {
            match sub {
                Subsystem::Systemd(c) => c,
                _ => {
                    assert_eq!(1, 0);
                    let v = std::mem::MaybeUninit::uninit();
                    v.assume_init()
                }
            }
        }
    }
}

impl SystemdController {
    /// Constructs a new `SystemdController` with `root` serving as the root of the control group.
    pub fn new(root: PathBuf, v2: bool) -> Self {
        Self {
            base: root.clone(),
            path: root,
            _v2: v2,
        }
    }
}
