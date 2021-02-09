// Copyright (c) 2018 Levente Kurusa
// Copyright (c) 2020 Ant Group
//
// SPDX-License-Identifier: Apache-2.0 or MIT
//

//! This module handles cgroup operations. Start here!

use crate::error::ErrorKind::*;
use crate::error::*;

use crate::libc_rmdir;

use crate::{CgroupPid, ControllIdentifier, Controller, Hierarchy, Resources, Subsystem};

use std::collections::HashMap;
use std::convert::From;
use std::fs;
use std::path::{Path, PathBuf};

/// A control group is the central structure to this crate.
///
///
/// # What are control groups?
///
/// Lifting over from the Linux kernel sources:
///
/// > Control Groups provide a mechanism for aggregating/partitioning sets of
/// > tasks, and all their future children, into hierarchical groups with
/// > specialized behaviour.
///
/// This crate is an attempt at providing a Rust-native way of managing these cgroups.
pub struct Cgroup<'b> {
    /// The list of subsystems that control this cgroup
    subsystems: Vec<Subsystem>,

    /// The hierarchy.
    hier: Box<&'b dyn Hierarchy>,
    path: String,
}

impl<'b> Cgroup<'b> {
    /// Create this control group.
    fn create(&self) {
        if self.hier.v2() {
            create_v2_cgroup(self.hier.root().clone(), &self.path);
        } else {
            for subsystem in &self.subsystems {
                subsystem.to_controller().create();
            }
        }
    }

    pub fn v2(&self) -> bool {
        self.hier.v2()
    }

    /// Create a new control group in the hierarchy `hier`, with name `path`.
    ///
    /// Returns a handle to the control group that can be used to manipulate it.
    ///
    /// Note that if the handle goes out of scope and is dropped, the control group is _not_
    /// destroyed.
    pub fn new<P: AsRef<Path>>(hier: Box<&'b dyn Hierarchy>, path: P) -> Cgroup<'b> {
        let relative_paths = get_cgroups_relative_paths().unwrap();
        Cgroup::new_with_relative_paths(hier, path, relative_paths)
    }

    /// Create a handle for a control group in the hierarchy `hier`, with name `path`.
    ///
    /// Returns a handle to the control group (that possibly does not exist until `create()` has
    /// been called on the cgroup.
    ///
    /// Note that if the handle goes out of scope and is dropped, the control group is _not_
    /// destroyed.
    pub fn load<P: AsRef<Path>>(hier: Box<&'b dyn Hierarchy>, path: P) -> Cgroup<'b> {
        let relative_paths = get_cgroups_relative_paths().unwrap();
        Cgroup::load_with_relative_paths(hier, path, relative_paths)
    }

    /// Create a new control group in the hierarchy `hier`, with name `path`.
    /// and relative paths from `/proc/self/cgroup`
    ///
    /// Returns a handle to the control group that can be used to manipulate it.
    ///
    /// Note that if the handle goes out of scope and is dropped, the control group is _not_
    /// destroyed.
    pub fn new_with_relative_paths<P: AsRef<Path>>(
        hier: Box<&'b dyn Hierarchy>,
        path: P,
        relative_paths: HashMap<String, String>,
    ) -> Cgroup<'b> {
        let cg = Cgroup::load_with_relative_paths(hier, path, relative_paths);
        cg.create();
        cg
    }

    /// Create a handle for a control group in the hierarchy `hier`, with name `path`,
    /// and relative paths from `/proc/self/cgroup`
    ///
    /// Returns a handle to the control group (that possibly does not exist until `create()` has
    /// been called on the cgroup.
    ///
    /// Note that if the handle goes out of scope and is dropped, the control group is _not_
    /// destroyed.
    pub fn load_with_relative_paths<P: AsRef<Path>>(
        hier: Box<&'b dyn Hierarchy>,
        path: P,
        relative_paths: HashMap<String, String>,
    ) -> Cgroup<'b> {
        let path = path.as_ref();
        let mut subsystems = hier.subsystems();
        if path.as_os_str() != "" {
            subsystems = subsystems
                .into_iter()
                .map(|x| {
                    let cn = x.controller_name();
                    if relative_paths.contains_key(&cn) {
                        let rp = relative_paths.get(&cn).unwrap();
                        let valid_path = rp.trim_start_matches("/").to_string();
                        let mut p = PathBuf::from(valid_path);
                        p.push(path);
                        x.enter(p.as_ref())
                    } else {
                        x.enter(path)
                    }
                })
                .collect::<Vec<_>>();
        }

        let cg = Cgroup {
            subsystems: subsystems,
            hier: hier,
            path: path.to_str().unwrap().to_string(),
        };

        cg
    }

    /// The list of subsystems that this control group supports.
    pub fn subsystems(&self) -> &Vec<Subsystem> {
        &self.subsystems
    }

    /// Deletes the control group.
    ///
    /// Note that this function makes no effort in cleaning up the descendant and the underlying
    /// system call will fail if there are any descendants. Thus, one should check whether it was
    /// actually removed, and remove the descendants first if not. In the future, this behavior
    /// will change.
    pub fn delete(self) {
        if self.v2() {
            if self.path != "" {
                let mut p = self.hier.root().clone();
                p.push(self.path);
                libc_rmdir(p.to_str().unwrap());
            }
            return;
        }

        self.subsystems.into_iter().for_each(|sub| match sub {
            Subsystem::Pid(pidc) => pidc.delete(),
            Subsystem::Mem(c) => c.delete(),
            Subsystem::CpuSet(c) => c.delete(),
            Subsystem::CpuAcct(c) => c.delete(),
            Subsystem::Cpu(c) => c.delete(),
            Subsystem::Devices(c) => c.delete(),
            Subsystem::Freezer(c) => c.delete(),
            Subsystem::NetCls(c) => c.delete(),
            Subsystem::BlkIo(c) => c.delete(),
            Subsystem::PerfEvent(c) => c.delete(),
            Subsystem::NetPrio(c) => c.delete(),
            Subsystem::HugeTlb(c) => c.delete(),
            Subsystem::Rdma(c) => c.delete(),
            Subsystem::Systemd(c) => c.delete(),
        });
    }

    /// Apply a set of resource limits to the control group.
    pub fn apply(&self, res: &Resources) -> Result<()> {
        self.subsystems
            .iter()
            .try_fold((), |_, e| e.to_controller().apply(res))
    }

    /// Retrieve a container based on type inference.
    ///
    /// ## Example:
    ///
    /// ```text
    /// let pids: &PidController = control_group.controller_of()
    ///                             .expect("No pids controller attached!");
    /// let cpu: &CpuController = control_group.controller_of()
    ///                             .expect("No cpu controller attached!");
    /// ```
    pub fn controller_of<'a, T>(self: &'a Self) -> Option<&'a T>
    where
        &'a T: From<&'a Subsystem>,
        T: Controller + ControllIdentifier,
    {
        for i in &self.subsystems {
            if i.to_controller().control_type() == T::controller_type() {
                // N.B.:
                // https://play.rust-lang.org/?gist=978b2846bacebdaa00be62374f4f4334&version=stable&mode=debug&edition=2015
                return Some(i.into());
            }
        }
        None
    }

    /// Removes a task from the control group.
    ///
    /// Note that this means that the task will be moved back to the root control group in the
    /// hierarchy and any rules applied to that control group will _still_ apply to the task.
    pub fn remove_task(&self, pid: CgroupPid) {
        let _ = self.hier.root_control_group().add_task(pid);
    }

    /// Attach a task to the control group.
    pub fn add_task(&self, pid: CgroupPid) -> Result<()> {
        if self.v2() {
            let subsystems = self.subsystems();
            if subsystems.len() > 0 {
                let c = subsystems[0].to_controller();
                c.add_task(&pid)
            } else {
                Ok(())
            }
        } else {
            self.subsystems()
                .iter()
                .try_for_each(|sub| sub.to_controller().add_task(&pid))
        }
    }

    /// Returns an Iterator that can be used to iterate over the tasks that are currently in the
    /// control group.
    pub fn tasks(&self) -> Vec<CgroupPid> {
        // Collect the tasks from all subsystems
        let mut v = if self.v2() {
            let subsystems = self.subsystems();
            if subsystems.len() > 0 {
                let c = subsystems[0].to_controller();
                c.tasks()
            } else {
                vec![]
            }
        } else {
            self.subsystems()
                .iter()
                .map(|x| x.to_controller().tasks())
                .fold(vec![], |mut acc, mut x| {
                    acc.append(&mut x);
                    acc
                })
        };

        v.sort();
        v.dedup();
        v
    }
}

pub const UNIFIED_MOUNTPOINT: &'static str = "/sys/fs/cgroup";

fn enable_controllers(controllers: &Vec<String>, path: &PathBuf) {
    let mut f = path.clone();
    f.push("cgroup.subtree_control");
    for c in controllers {
        let body = format!("+{}", c);
        let _rest = fs::write(f.as_path(), body.as_bytes());
    }
}

fn supported_controllers(p: &PathBuf) -> Vec<String> {
    let p = format!("{}/{}", UNIFIED_MOUNTPOINT, "cgroup.controllers");
    let ret = fs::read_to_string(p.as_str());
    ret.unwrap_or(String::new())
        .split(" ")
        .map(|x| x.to_string())
        .collect::<Vec<String>>()
}

fn create_v2_cgroup(root: PathBuf, path: &str) -> Result<()> {
    // controler list ["memory", "cpu"]
    let controllers = supported_controllers(&root);
    let mut fp = root;

    // enable for root
    enable_controllers(&controllers, &fp);

    // path: "a/b/c"
    let elements = path.split("/").collect::<Vec<&str>>();
    let last_index = elements.len() - 1;
    for (i, ele) in elements.iter().enumerate() {
        // ROOT/a
        fp.push(ele);
        // create dir, need not check if is a file or directory
        if !fp.exists() {
            match ::std::fs::create_dir(fp.clone()) {
                Err(e) => return Err(Error::with_cause(ErrorKind::FsError, e)),
                Ok(_) => {}
            }
        }

        if i < last_index {
            // enable controllers for substree
            enable_controllers(&controllers, &fp);
        }
    }

    Ok(())
}

pub fn get_cgroups_relative_paths() -> Result<HashMap<String, String>> {
    let mut m = HashMap::new();
    let content =
        fs::read_to_string("/proc/self/cgroup").map_err(|e| Error::with_cause(ReadFailed, e))?;
    for l in content.lines() {
        let fl: Vec<&str> = l.split(':').collect();
        if fl.len() != 3 {
            continue;
        }

        let keys: Vec<&str> = fl[1].split(',').collect();
        for key in &keys {
            // this is a workaround, cgroup file are using `name=systemd`,
            // but if file system the name is `systemd`
            if *key == "name=systemd" {
                m.insert("systemd".to_string(), fl[2].to_string());
            } else {
                m.insert(key.to_string(), fl[2].to_string());
            }
        }
    }
    Ok(m)
}
