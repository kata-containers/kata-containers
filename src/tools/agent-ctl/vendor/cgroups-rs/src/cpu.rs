// Copyright (c) 2018 Levente Kurusa
// Copyright (c) 2020 Ant Group
//
// SPDX-License-Identifier: Apache-2.0 or MIT
//

//! This module contains the implementation of the `cpu` cgroup subsystem.
//!
//! See the Kernel's documentation for more information about this subsystem, found at:
//!  [Documentation/scheduler/sched-design-CFS.txt](https://www.kernel.org/doc/Documentation/scheduler/sched-design-CFS.txt)
//!  paragraph 7 ("GROUP SCHEDULER EXTENSIONS TO CFS").
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;

use crate::error::ErrorKind::*;
use crate::error::*;
use crate::{parse_max_value, read_i64_from, read_u64_from};

use crate::{
    ControllIdentifier, ControllerInternal, Controllers, CpuResources, CustomizedAttribute,
    MaxValue, Resources, Subsystem,
};

/// A controller that allows controlling the `cpu` subsystem of a Cgroup.
///
/// In essence, it allows gathering information about how much the tasks inside the control group
/// are using the CPU and creating rules that limit their usage. Note that this crate does not yet
/// support managing realtime tasks.
#[derive(Debug, Clone)]
pub struct CpuController {
    base: PathBuf,
    path: PathBuf,
    v2: bool,
}

/// The current state of the control group and its processes.
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Cpu {
    /// Reports CPU time statistics.
    ///
    /// Corresponds the `cpu.stat` file in `cpu` control group.
    pub stat: String,
}

/// The current state of the control group and its processes.
#[derive(Debug)]
struct CfsQuotaAndPeriod {
    quota: MaxValue,
    period: u64,
}

impl ControllerInternal for CpuController {
    fn control_type(&self) -> Controllers {
        Controllers::Cpu
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
        let res: &CpuResources = &res.cpu;

        update_and_test!(self, set_shares, res.shares, shares);
        update_and_test!(self, set_cfs_period, res.period, cfs_period);
        update_and_test!(self, set_cfs_quota, res.quota, cfs_quota);

        res.attrs.iter().for_each(|(k, v)| {
            let _ = self.set(k, v);
        });

        // TODO: rt properties (CONFIG_RT_GROUP_SCHED) are not yet supported

        Ok(())
    }
}

impl ControllIdentifier for CpuController {
    fn controller_type() -> Controllers {
        Controllers::Cpu
    }
}

impl<'a> From<&'a Subsystem> for &'a CpuController {
    fn from(sub: &'a Subsystem) -> &'a CpuController {
        unsafe {
            match sub {
                Subsystem::Cpu(c) => c,
                _ => {
                    assert_eq!(1, 0);
                    let v = std::mem::MaybeUninit::uninit();
                    v.assume_init()
                }
            }
        }
    }
}

impl CpuController {
    /// Contructs a new `CpuController` with `root` serving as the root of the control group.
    pub fn new(root: PathBuf, v2: bool) -> Self {
        Self {
            base: root.clone(),
            path: root,
            v2,
        }
    }

    /// Returns CPU time statistics based on the processes in the control group.
    pub fn cpu(&self) -> Cpu {
        Cpu {
            stat: self
                .open_path("cpu.stat", false)
                .and_then(|mut file| {
                    let mut s = String::new();
                    let res = file.read_to_string(&mut s);
                    match res {
                        Ok(_) => Ok(s),
                        Err(e) => Err(Error::with_cause(ReadFailed("cpu.stat".to_string()), e)),
                    }
                })
                .unwrap_or_default(),
        }
    }

    /// Configures the CPU bandwidth (in relative relation to other control groups and this control
    /// group's parent).
    ///
    /// For example, setting control group `A`'s `shares` to `100`, and control group `B`'s
    /// `shares` to `200` ensures that control group `B` receives twice as much as CPU bandwidth.
    /// (Assuming both `A` and `B` are of the same parent)
    pub fn set_shares(&self, shares: u64) -> Result<()> {
        let mut file_name = "cpu.shares";
        if self.v2 {
            file_name = "cpu.weight";
        }
        // NOTE: .CpuShares is not used here. Conversion is the caller's responsibility.
        self.open_path(file_name, true).and_then(|mut file| {
            file.write_all(shares.to_string().as_ref()).map_err(|e| {
                Error::with_cause(WriteFailed(file_name.to_string(), shares.to_string()), e)
            })
        })
    }

    /// Retrieve the CPU bandwidth that this control group (relative to other control groups and
    /// this control group's parent) can use.
    pub fn shares(&self) -> Result<u64> {
        let mut file = "cpu.shares";
        if self.v2 {
            file = "cpu.weight";
        }
        self.open_path(file, false).and_then(read_u64_from)
    }

    /// Specify a period (when using the CFS scheduler) of time in microseconds for how often this
    /// control group's access to the CPU should be reallocated.
    pub fn set_cfs_period(&self, us: u64) -> Result<()> {
        if self.v2 {
            return self.set_cfs_quota_and_period(None, Some(us));
        }
        self.open_path("cpu.cfs_period_us", true)
            .and_then(|mut file| {
                file.write_all(us.to_string().as_ref()).map_err(|e| {
                    Error::with_cause(
                        WriteFailed("cpu.cfs_period_us".to_string(), us.to_string()),
                        e,
                    )
                })
            })
    }

    /// Retrieve the period of time of how often this cgroup's access to the CPU should be
    /// reallocated in microseconds.
    pub fn cfs_period(&self) -> Result<u64> {
        if self.v2 {
            let current_value = self
                .open_path("cpu.max", false)
                .and_then(parse_cfs_quota_and_period)?;
            return Ok(current_value.period);
        }
        self.open_path("cpu.cfs_period_us", false)
            .and_then(read_u64_from)
    }

    /// Specify a quota (when using the CFS scheduler) of time in microseconds for which all tasks
    /// in this control group can run during one period (see: `set_cfs_period()`).
    pub fn set_cfs_quota(&self, us: i64) -> Result<()> {
        if self.v2 {
            return self.set_cfs_quota_and_period(Some(us), None);
        }
        self.open_path("cpu.cfs_quota_us", true)
            .and_then(|mut file| {
                file.write_all(us.to_string().as_ref()).map_err(|e| {
                    Error::with_cause(
                        WriteFailed("cpu.cfs_quota_us".to_string(), us.to_string()),
                        e,
                    )
                })
            })
    }

    /// Retrieve the quota of time for which all tasks in this cgroup can run during one period, in
    /// microseconds.
    pub fn cfs_quota(&self) -> Result<i64> {
        if self.v2 {
            let current_value = self
                .open_path("cpu.max", false)
                .and_then(parse_cfs_quota_and_period)?;
            return Ok(current_value.quota.to_i64());
        }

        self.open_path("cpu.cfs_quota_us", false)
            .and_then(read_i64_from)
    }

    pub fn set_cfs_quota_and_period(&self, quota: Option<i64>, period: Option<u64>) -> Result<()> {
        if !self.v2 {
            if let Some(q) = quota {
                self.set_cfs_quota(q)?;
            }
            if let Some(p) = period {
                self.set_cfs_period(p)?;
            }
            return Ok(());
        }

        // https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html

        // cpu.max
        // A read-write two value file which exists on non-root cgroups. The default is “max 100000”.
        // The maximum bandwidth limit. It’s in the following format:
        // $MAX $PERIOD
        // which indicates that the group may consume upto $MAX in each $PERIOD duration.
        // “max” for $MAX indicates no limit. If only one number is written, $MAX is updated.

        let current_value = self
            .open_path("cpu.max", false)
            .and_then(parse_cfs_quota_and_period)?;

        let new_quota = if let Some(q) = quota {
            if q > 0 {
                q.to_string()
            } else {
                "max".to_string()
            }
        } else {
            current_value.quota.to_string()
        };

        let new_period = if let Some(p) = period {
            p.to_string()
        } else {
            current_value.period.to_string()
        };

        let line = format!("{} {}", new_quota, new_period);
        self.open_path("cpu.max", true).and_then(|mut file| {
            file.write_all(line.as_ref())
                .map_err(|e| Error::with_cause(WriteFailed("cpu.max".to_string(), line), e))
        })
    }

    pub fn set_rt_runtime(&self, us: i64) -> Result<()> {
        self.open_path("cpu.rt_runtime_us", true)
            .and_then(|mut file| {
                file.write_all(us.to_string().as_ref()).map_err(|e| {
                    Error::with_cause(
                        WriteFailed("cpu.rt_runtime_us".to_string(), us.to_string()),
                        e,
                    )
                })
            })
    }

    pub fn set_rt_period_us(&self, us: u64) -> Result<()> {
        self.open_path("cpu.rt_period_us", true)
            .and_then(|mut file| {
                file.write_all(us.to_string().as_ref()).map_err(|e| {
                    Error::with_cause(
                        WriteFailed("cpu.rt_period_us".to_string(), us.to_string()),
                        e,
                    )
                })
            })
    }
}

impl CustomizedAttribute for CpuController {}

fn parse_cfs_quota_and_period(mut file: File) -> Result<CfsQuotaAndPeriod> {
    let mut content = String::new();
    file.read_to_string(&mut content)
        .map_err(|e| Error::with_cause(ReadFailed("cpu.max".to_string()), e))?;

    let fields = content.trim().split(' ').collect::<Vec<&str>>();
    if fields.len() != 2 {
        return Err(Error::from_string(format!("invaild format: {}", content)));
    }

    let quota = parse_max_value(fields[0])?;
    let period = fields[1]
        .parse::<u64>()
        .map_err(|e| Error::with_cause(ParseError, e))?;

    Ok(CfsQuotaAndPeriod { quota, period })
}
