// Copyright (c) 2018 Levente Kurusa
//
// SPDX-License-Identifier: Apache-2.0 or MIT
//

//! This module contains the implementation of the `perf_event` cgroup subsystem.
//!
//! See the Kernel's documentation for more information about this subsystem, found at:
//!  [tools/perf/Documentation/perf-record.txt](https://raw.githubusercontent.com/torvalds/linux/master/tools/perf/Documentation/perf-record.txt)
use std::path::PathBuf;

use crate::error::*;

use crate::{ControllIdentifier, ControllerInternal, Controllers, Resources, Subsystem};

/// A controller that allows controlling the `perf_event` subsystem of a Cgroup.
///
/// In essence, when processes belong to the same `perf_event` controller, they can be monitored
/// together using the `perf` performance monitoring and reporting tool.
#[derive(Debug, Clone)]
pub struct PerfEventController {
    base: PathBuf,
    path: PathBuf,
}

impl ControllerInternal for PerfEventController {
    fn control_type(&self) -> Controllers {
        Controllers::PerfEvent
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

impl ControllIdentifier for PerfEventController {
    fn controller_type() -> Controllers {
        Controllers::PerfEvent
    }
}

impl<'a> From<&'a Subsystem> for &'a PerfEventController {
    fn from(sub: &'a Subsystem) -> &'a PerfEventController {
        unsafe {
            match sub {
                Subsystem::PerfEvent(c) => c,
                _ => {
                    assert_eq!(1, 0);
                    let v = std::mem::MaybeUninit::uninit();
                    v.assume_init()
                }
            }
        }
    }
}

impl PerfEventController {
    /// Constructs a new `PerfEventController` with `root` serving as the root of the control group.
    pub fn new(root: PathBuf) -> Self {
        Self {
            base: root.clone(),
            path: root,
        }
    }
}
