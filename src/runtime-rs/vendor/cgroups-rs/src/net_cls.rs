// Copyright (c) 2018 Levente Kurusa
//
// SPDX-License-Identifier: Apache-2.0 or MIT
//

//! This module contains the implementation of the `net_cls` cgroup subsystem.
//!
//! See the Kernel's documentation for more information about this subsystem, found at:
//!  [Documentation/cgroup-v1/net_cls.txt](https://www.kernel.org/doc/Documentation/cgroup-v1/net_cls.txt)
use std::io::Write;
use std::path::PathBuf;

use crate::error::ErrorKind::*;
use crate::error::*;

use crate::read_u64_from;
use crate::{
    ControllIdentifier, ControllerInternal, Controllers, NetworkResources, Resources, Subsystem,
};

/// A controller that allows controlling the `net_cls` subsystem of a Cgroup.
///
/// In esssence, using the `net_cls` controller, one can attach a custom class to the network
/// packets emitted by the control group's tasks. This can then later be used in iptables to have
/// custom firewall rules, QoS, etc.
#[derive(Debug, Clone)]
pub struct NetClsController {
    base: PathBuf,
    path: PathBuf,
}

impl ControllerInternal for NetClsController {
    fn control_type(&self) -> Controllers {
        Controllers::NetCls
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

    fn apply(&self, res: &Resources) -> Result<()> {
        // get the resources that apply to this controller
        let res: &NetworkResources = &res.network;

        update_and_test!(self, set_class, res.class_id, get_class);

        Ok(())
    }
}

impl ControllIdentifier for NetClsController {
    fn controller_type() -> Controllers {
        Controllers::NetCls
    }
}

impl<'a> From<&'a Subsystem> for &'a NetClsController {
    fn from(sub: &'a Subsystem) -> &'a NetClsController {
        unsafe {
            match sub {
                Subsystem::NetCls(c) => c,
                _ => {
                    assert_eq!(1, 0);
                    let v = std::mem::MaybeUninit::uninit();
                    v.assume_init()
                }
            }
        }
    }
}

impl NetClsController {
    /// Constructs a new `NetClsController` with `root` serving as the root of the control group.
    pub fn new(root: PathBuf) -> Self {
        Self {
            base: root.clone(),
            path: root,
        }
    }

    /// Set the network class id of the outgoing packets of the control group's tasks.
    pub fn set_class(&self, class: u64) -> Result<()> {
        self.open_path("net_cls.classid", true)
            .and_then(|mut file| {
                let s = format!("{:#08X}", class);
                file.write_all(s.as_ref()).map_err(|e| {
                    Error::with_cause(WriteFailed("net_cls.classid".to_string(), s), e)
                })
            })
    }

    /// Get the network class id of the outgoing packets of the control group's tasks.
    pub fn get_class(&self) -> Result<u64> {
        self.open_path("net_cls.classid", false)
            .and_then(read_u64_from)
    }
}
