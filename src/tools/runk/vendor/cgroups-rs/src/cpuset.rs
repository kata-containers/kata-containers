// Copyright (c) 2018 Levente Kurusa
// Copyright (c) 2020 Ant Group
//
// SPDX-License-Identifier: Apache-2.0 or MIT
//

//! This module contains the implementation of the `cpuset` cgroup subsystem.
//!
//! See the Kernel's documentation for more information about this subsystem, found at:
//!  [Documentation/cgroup-v1/cpusets.txt](https://www.kernel.org/doc/Documentation/cgroup-v1/cpusets.txt)

use log::*;
use std::io::Write;
use std::path::PathBuf;

use crate::error::ErrorKind::*;
use crate::error::*;

use crate::{read_string_from, read_u64_from};
use crate::{
    ControllIdentifier, ControllerInternal, Controllers, CpuResources, Resources, Subsystem,
};

/// A controller that allows controlling the `cpuset` subsystem of a Cgroup.
///
/// In essence, this controller is responsible for restricting the tasks in the control group to a
/// set of CPUs and/or memory nodes.
#[derive(Debug, Clone)]
pub struct CpuSetController {
    base: PathBuf,
    path: PathBuf,
    v2: bool,
}

/// The current state of the `cpuset` controller for this control group.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CpuSet {
    /// If true, no other control groups can share the CPUs listed in the `cpus` field.
    pub cpu_exclusive: bool,
    /// The list of CPUs the tasks of the control group can run on.
    ///
    /// This is a vector of `(start, end)` tuples, where each tuple is a range of CPUs where the
    /// control group is allowed to run on. Both sides of the range are inclusive.
    pub cpus: Vec<(u64, u64)>,
    /// The list of CPUs that the tasks can effectively run on. This removes the list of CPUs that
    /// the parent (and all of its parents) cannot run on from the `cpus` field of this control
    /// group.
    pub effective_cpus: Vec<(u64, u64)>,
    /// The list of memory nodes that the tasks can effectively use. This removes the list of nodes that
    /// the parent (and all of its parents) cannot use from the `mems` field of this control
    /// group.
    pub effective_mems: Vec<(u64, u64)>,
    /// If true, no other control groups can share the memory nodes listed in the `mems` field.
    pub mem_exclusive: bool,
    /// If true, the control group is 'hardwalled'. Kernel memory allocations (except for a few
    /// minor exceptions) are made from the memory nodes designated in the `mems` field.
    pub mem_hardwall: bool,
    /// If true, whenever `mems` is changed via `set_mems()`, the memory stored on the previous
    /// nodes are migrated to the new nodes selected by the new `mems`.
    pub memory_migrate: bool,
    /// Running average of the memory pressured faced by the tasks in the control group.
    pub memory_pressure: u64,
    /// This field is only at the root control group and controls whether the kernel will compute
    /// the memory pressure for control groups or not.
    pub memory_pressure_enabled: Option<bool>,
    /// If true, filesystem buffers are spread across evenly between the nodes specified in `mems`.
    pub memory_spread_page: bool,
    /// If true, kernel slab caches for file I/O are spread across evenly between the nodes
    /// specified in `mems`.
    pub memory_spread_slab: bool,
    /// The list of memory nodes the tasks of the control group can use.
    ///
    /// The format is the same as the `cpus`, `effective_cpus` and `effective_mems` fields.
    pub mems: Vec<(u64, u64)>,
    /// If true, the kernel will attempt to rebalance the load between the CPUs specified in the
    /// `cpus` field of this control group.
    pub sched_load_balance: bool,
    /// Represents how much work the kernel should do to rebalance this cpuset.
    ///
    /// | `sched_load_balance` | Effect |
    /// | -------------------- | ------ |
    /// |          -1          | Use the system default value |
    /// |           0          | Only balance loads periodically |
    /// |           1          | Immediately balance the load across tasks on the same core |
    /// |           2          | Immediately balance the load across cores in the same CPU package |
    /// |           4          | Immediately balance the load across CPUs on the same node |
    /// |           5          | Immediately balance the load between CPUs even if the system is NUMA |
    /// |           6          | Immediately balance the load between all CPUs |
    pub sched_relax_domain_level: u64,
}

impl ControllerInternal for CpuSetController {
    fn control_type(&self) -> Controllers {
        Controllers::CpuSet
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

        update!(self, set_cpus, res.cpus.as_ref());
        update!(self, set_mems, res.mems.as_ref());

        Ok(())
    }

    fn post_create(&self) {
        if self.is_v2() {
            return;
        }
        let current = self.get_path();

        if current != self.get_base() {
            match copy_from_parent(current.to_str().unwrap(), "cpuset.cpus") {
                Ok(_) => (),
                Err(err) => error!("error create_dir for cpuset.cpus {:?}", err),
            }
            match copy_from_parent(current.to_str().unwrap(), "cpuset.mems") {
                Ok(_) => (),
                Err(err) => error!("error create_dir for cpuset.mems {:?}", err),
            }
        }
    }
}

fn find_no_empty_parent(from: &str, file: &str) -> Result<(String, Vec<PathBuf>)> {
    let mut current_path = ::std::path::Path::new(from).to_path_buf();
    let mut v = vec![];

    loop {
        let current_value =
            match ::std::fs::read_to_string(current_path.clone().join(file).to_str().unwrap()) {
                Ok(cpus) => String::from(cpus.trim()),
                Err(e) => {
                    return Err(Error::with_cause(
                        ReadFailed(current_path.display().to_string()),
                        e,
                    ))
                }
            };

        if !current_value.is_empty() {
            return Ok((current_value, v));
        }
        v.push(current_path.clone());

        let parent = match current_path.parent() {
            Some(p) => p,
            None => return Ok(("".to_string(), v)),
        };

        // next loop, find parent
        current_path = parent.to_path_buf();
    }
}

/// copy_from_parent copy the cpuset.cpus and cpuset.mems from the parent
/// directory to the current directory if the file's contents are 0
fn copy_from_parent(current: &str, file: &str) -> Result<()> {
    // find not empty cpus/memes from current directory.
    let (value, parents) = find_no_empty_parent(current, file)?;

    if value.is_empty() || parents.is_empty() {
        return Ok(());
    }

    for p in parents.iter().rev() {
        let mut pb = p.clone();
        pb.push(file);
        match ::std::fs::write(pb.to_str().unwrap(), value.as_bytes()) {
            Ok(_) => (),
            Err(e) => {
                return Err(Error::with_cause(
                    WriteFailed(pb.display().to_string(), pb.display().to_string()),
                    e,
                ))
            }
        }
    }

    Ok(())
}

impl ControllIdentifier for CpuSetController {
    fn controller_type() -> Controllers {
        Controllers::CpuSet
    }
}

impl<'a> From<&'a Subsystem> for &'a CpuSetController {
    fn from(sub: &'a Subsystem) -> &'a CpuSetController {
        unsafe {
            match sub {
                Subsystem::CpuSet(c) => c,
                _ => {
                    assert_eq!(1, 0);
                    let v = std::mem::MaybeUninit::uninit();
                    v.assume_init()
                }
            }
        }
    }
}

/// Parse a string like "1,2,4-5,8" into a list of (start, end) tuples.
fn parse_range(s: String) -> Result<Vec<(u64, u64)>> {
    let mut fin = Vec::new();

    if s.is_empty() {
        return Ok(fin);
    }

    // first split by commas
    let comma_split = s.split(',');

    for sp in comma_split {
        if sp.contains('-') {
            // this is a true range
            let dash_split = sp.split('-').collect::<Vec<_>>();
            if dash_split.len() != 2 {
                return Err(Error::new(ParseError));
            }
            let first = dash_split[0].parse::<u64>();
            let second = dash_split[1].parse::<u64>();
            if first.is_err() || second.is_err() {
                return Err(Error::new(ParseError));
            }
            fin.push((first.unwrap(), second.unwrap()));
        } else {
            // this is just a single number
            let num = sp.parse::<u64>();
            if num.is_err() {
                return Err(Error::new(ParseError));
            }
            fin.push((num.clone().unwrap(), num.clone().unwrap()));
        }
    }

    Ok(fin)
}

impl CpuSetController {
    /// Contructs a new `CpuSetController` with `root` serving as the root of the control group.
    pub fn new(root: PathBuf, v2: bool) -> Self {
        Self {
            base: root.clone(),
            path: root,
            v2,
        }
    }

    /// Returns the statistics gathered by the kernel for this control group. See the struct for
    /// more information on what information this entails.
    pub fn cpuset(&self) -> CpuSet {
        CpuSet {
            cpu_exclusive: {
                self.open_path("cpuset.cpu_exclusive", false)
                    .and_then(read_u64_from)
                    .map(|x| x == 1)
                    .unwrap_or(false)
            },
            cpus: {
                self.open_path("cpuset.cpus", false)
                    .and_then(read_string_from)
                    .and_then(parse_range)
                    .unwrap_or_default()
            },
            effective_cpus: {
                self.open_path("cpuset.effective_cpus", false)
                    .and_then(read_string_from)
                    .and_then(parse_range)
                    .unwrap_or_default()
            },
            effective_mems: {
                self.open_path("cpuset.effective_mems", false)
                    .and_then(read_string_from)
                    .and_then(parse_range)
                    .unwrap_or_default()
            },
            mem_exclusive: {
                self.open_path("cpuset.mem_exclusive", false)
                    .and_then(read_u64_from)
                    .map(|x| x == 1)
                    .unwrap_or(false)
            },
            mem_hardwall: {
                self.open_path("cpuset.mem_hardwall", false)
                    .and_then(read_u64_from)
                    .map(|x| x == 1)
                    .unwrap_or(false)
            },
            memory_migrate: {
                self.open_path("cpuset.memory_migrate", false)
                    .and_then(read_u64_from)
                    .map(|x| x == 1)
                    .unwrap_or(false)
            },
            memory_pressure: {
                self.open_path("cpuset.memory_pressure", false)
                    .and_then(read_u64_from)
                    .unwrap_or(0)
            },
            memory_pressure_enabled: {
                self.open_path("cpuset.memory_pressure_enabled", false)
                    .and_then(read_u64_from)
                    .map(|x| x == 1)
                    .ok()
            },
            memory_spread_page: {
                self.open_path("cpuset.memory_spread_page", false)
                    .and_then(read_u64_from)
                    .map(|x| x == 1)
                    .unwrap_or(false)
            },
            memory_spread_slab: {
                self.open_path("cpuset.memory_spread_slab", false)
                    .and_then(read_u64_from)
                    .map(|x| x == 1)
                    .unwrap_or(false)
            },
            mems: {
                self.open_path("cpuset.mems", false)
                    .and_then(read_string_from)
                    .and_then(parse_range)
                    .unwrap_or_default()
            },
            sched_load_balance: {
                self.open_path("cpuset.sched_load_balance", false)
                    .and_then(read_u64_from)
                    .map(|x| x == 1)
                    .unwrap_or(false)
            },
            sched_relax_domain_level: {
                self.open_path("cpuset.sched_relax_domain_level", false)
                    .and_then(read_u64_from)
                    .unwrap_or(0)
            },
        }
    }

    /// Control whether the CPUs selected via `set_cpus()` should be exclusive to this control
    /// group or not.
    pub fn set_cpu_exclusive(&self, b: bool) -> Result<()> {
        self.open_path("cpuset.cpu_exclusive", true)
            .and_then(|mut file| {
                if b {
                    file.write_all(b"1").map_err(|e| {
                        Error::with_cause(
                            WriteFailed("cpuset.cpu_exclusive".to_string(), "1".to_string()),
                            e,
                        )
                    })
                } else {
                    file.write_all(b"0").map_err(|e| {
                        Error::with_cause(
                            WriteFailed("cpuset.cpu_exclusive".to_string(), "0".to_string()),
                            e,
                        )
                    })
                }
            })
    }

    /// Control whether the memory nodes selected via `set_memss()` should be exclusive to this control
    /// group or not.
    pub fn set_mem_exclusive(&self, b: bool) -> Result<()> {
        self.open_path("cpuset.mem_exclusive", true)
            .and_then(|mut file| {
                if b {
                    file.write_all(b"1").map_err(|e| {
                        Error::with_cause(
                            WriteFailed("cpuset.mem_exclusive".to_string(), "1".to_string()),
                            e,
                        )
                    })
                } else {
                    file.write_all(b"0").map_err(|e| {
                        Error::with_cause(
                            WriteFailed("cpuset.mem_exclusive".to_string(), "0".to_string()),
                            e,
                        )
                    })
                }
            })
    }

    /// Set the CPUs that the tasks in this control group can run on.
    ///
    /// Syntax is a comma separated list of CPUs, with an additional extension that ranges can
    /// be represented via dashes.
    pub fn set_cpus(&self, cpus: &str) -> Result<()> {
        self.open_path("cpuset.cpus", true).and_then(|mut file| {
            file.write_all(cpus.as_ref()).map_err(|e| {
                Error::with_cause(WriteFailed("cpuset.cpus".to_string(), cpus.to_string()), e)
            })
        })
    }

    /// Set the memory nodes that the tasks in this control group can use.
    ///
    /// Syntax is the same as with `set_cpus()`.
    pub fn set_mems(&self, mems: &str) -> Result<()> {
        self.open_path("cpuset.mems", true).and_then(|mut file| {
            file.write_all(mems.as_ref()).map_err(|e| {
                Error::with_cause(WriteFailed("cpuset.mems".to_string(), mems.to_string()), e)
            })
        })
    }

    /// Controls whether the control group should be "hardwalled", i.e., whether kernel allocations
    /// should exclusively use the memory nodes set via `set_mems()`.
    ///
    /// Note that some kernel allocations, most notably those that are made in interrupt handlers
    /// may disregard this.
    pub fn set_hardwall(&self, b: bool) -> Result<()> {
        self.open_path("cpuset.mem_hardwall", true)
            .and_then(|mut file| {
                if b {
                    file.write_all(b"1").map_err(|e| {
                        Error::with_cause(
                            WriteFailed("cpuset.mem_hardwall".to_string(), "1".to_string()),
                            e,
                        )
                    })
                } else {
                    file.write_all(b"0").map_err(|e| {
                        Error::with_cause(
                            WriteFailed("cpuset.mem_hardwall".to_string(), "0".to_string()),
                            e,
                        )
                    })
                }
            })
    }

    /// Controls whether the kernel should attempt to rebalance the load between the CPUs specified in the
    /// `cpus` field of this control group.
    pub fn set_load_balancing(&self, b: bool) -> Result<()> {
        self.open_path("cpuset.sched_load_balance", true)
            .and_then(|mut file| {
                if b {
                    file.write_all(b"1").map_err(|e| {
                        Error::with_cause(
                            WriteFailed("cpuset.sched_load_balance".to_string(), "1".to_string()),
                            e,
                        )
                    })
                } else {
                    file.write_all(b"0").map_err(|e| {
                        Error::with_cause(
                            WriteFailed("cpuset.sched_load_balance".to_string(), "0".to_string()),
                            e,
                        )
                    })
                }
            })
    }

    /// Contorl how much effort the kernel should invest in rebalacing the control group.
    ///
    /// See @CpuSet 's similar field for more information.
    pub fn set_rebalance_relax_domain_level(&self, i: i64) -> Result<()> {
        self.open_path("cpuset.sched_relax_domain_level", true)
            .and_then(|mut file| {
                file.write_all(i.to_string().as_ref()).map_err(|e| {
                    Error::with_cause(
                        WriteFailed("cpuset.sched_relax_domain_level".to_string(), i.to_string()),
                        e,
                    )
                })
            })
    }

    /// Control whether when using `set_mems()` the existing memory used by the tasks should be
    /// migrated over to the now-selected nodes.
    pub fn set_memory_migration(&self, b: bool) -> Result<()> {
        self.open_path("cpuset.memory_migrate", true)
            .and_then(|mut file| {
                if b {
                    file.write_all(b"1").map_err(|e| {
                        Error::with_cause(
                            WriteFailed("cpuset.memory_migrate".to_string(), "1".to_string()),
                            e,
                        )
                    })
                } else {
                    file.write_all(b"0").map_err(|e| {
                        Error::with_cause(
                            WriteFailed("cpuset.memory_migrate".to_string(), "0".to_string()),
                            e,
                        )
                    })
                }
            })
    }

    /// Control whether filesystem buffers should be evenly split across the nodes selected via
    /// `set_mems()`.
    pub fn set_memory_spread_page(&self, b: bool) -> Result<()> {
        self.open_path("cpuset.memory_spread_page", true)
            .and_then(|mut file| {
                if b {
                    file.write_all(b"1").map_err(|e| {
                        Error::with_cause(
                            WriteFailed("cpuset.memory_spread_page".to_string(), "1".to_string()),
                            e,
                        )
                    })
                } else {
                    file.write_all(b"0").map_err(|e| {
                        Error::with_cause(
                            WriteFailed("cpuset.memory_spread_page".to_string(), "0".to_string()),
                            e,
                        )
                    })
                }
            })
    }

    /// Control whether the kernel's slab cache for file I/O should be evenly split across the
    /// nodes selected via `set_mems()`.
    pub fn set_memory_spread_slab(&self, b: bool) -> Result<()> {
        self.open_path("cpuset.memory_spread_slab", true)
            .and_then(|mut file| {
                if b {
                    file.write_all(b"1").map_err(|e| {
                        Error::with_cause(
                            WriteFailed("cpuset.memory_spread_slab".to_string(), "1".to_string()),
                            e,
                        )
                    })
                } else {
                    file.write_all(b"0").map_err(|e| {
                        Error::with_cause(
                            WriteFailed("cpuset.memory_spread_slab".to_string(), "0".to_string()),
                            e,
                        )
                    })
                }
            })
    }

    /// Control whether the kernel should collect information to calculate memory pressure for
    /// control groups.
    ///
    /// Note: This will fail with `InvalidOperation` if the current congrol group is not the root
    /// control group.
    pub fn set_enable_memory_pressure(&self, b: bool) -> Result<()> {
        if !self.path_exists("cpuset.memory_pressure_enabled") {
            return Err(Error::new(InvalidOperation));
        }
        self.open_path("cpuset.memory_pressure_enabled", true)
            .and_then(|mut file| {
                if b {
                    file.write_all(b"1").map_err(|e| {
                        Error::with_cause(
                            WriteFailed(
                                "cpuset.memory_pressure_enabled".to_string(),
                                "1".to_string(),
                            ),
                            e,
                        )
                    })
                } else {
                    file.write_all(b"0").map_err(|e| {
                        Error::with_cause(
                            WriteFailed(
                                "cpuset.memory_pressure_enabled".to_string(),
                                "0".to_string(),
                            ),
                            e,
                        )
                    })
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use crate::cpuset;
    #[test]
    fn test_parse_range() {
        let test_cases = vec![
            "1,2,4-6,9".to_string(),
            "".to_string(),
            "1".to_string(),
            "1-111".to_string(),
            "1,2,3,4".to_string(),
            "1-5,6-7,8-9".to_string(),
        ];
        let expecteds = vec![
            vec![(1, 1), (2, 2), (4, 6), (9, 9)],
            vec![],
            vec![(1, 1)],
            vec![(1, 111)],
            vec![(1, 1), (2, 2), (3, 3), (4, 4)],
            vec![(1, 5), (6, 7), (8, 9)],
        ];

        for (i, case) in test_cases.into_iter().enumerate() {
            let range = cpuset::parse_range(case.clone());
            println!("{:?} => {:?}", case, range);
            assert!(range.is_ok());
            assert_eq!(range.unwrap(), expecteds[i]);
        }
    }
}
