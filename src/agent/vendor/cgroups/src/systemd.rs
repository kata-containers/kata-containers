// Copyright (c) 2020 Ant Group
//
// SPDX-License-Identifier: Apache-2.0 or MIT
//

//! This module contains the implementation of the `systemd` cgroup subsystem.
//!
use std::path::PathBuf;

use crate::error::ErrorKind::*;
use crate::error::*;

use crate::{ControllIdentifier, ControllerInternal, Controllers, Resources, Subsystem};

/// A controller that allows controlling the `systemd` subsystem of a Cgroup.
///
#[derive(Debug, Clone)]
pub struct SystemdController {
    base: PathBuf,
    path: PathBuf,
    v2: bool,
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
                    ::std::mem::uninitialized()
                }
            }
        }
    }
}

impl SystemdController {
    /// Constructs a new `SystemdController` with `oroot` serving as the root of the control group.
    pub fn new(oroot: PathBuf, v2: bool) -> Self {
        let mut root = oroot;
        if !v2 {
            root.push(Self::controller_type().to_string());
        }
        Self {
            base: root.clone(),
            path: root,
            v2: v2,
        }
    }
}
