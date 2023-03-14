// Copyright (c) 2018 Levente Kurusa
// Copyright (c) 2020 Ant Group
//
// SPDX-License-Identifier: Apache-2.0 or MIT
//

//! This module handles cgroup operations. Start here!

use crate::error::ErrorKind::*;
use crate::error::*;

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
#[derive(Debug)]
pub struct Cgroup {
    /// The list of subsystems that control this cgroup
    subsystems: Vec<Subsystem>,

    /// The hierarchy.
    hier: Box<dyn Hierarchy>,
    path: String,

    /// List of controllers specifically enabled in the control group.
    specified_controllers: Option<Vec<String>>,
}

impl Clone for Cgroup {
    fn clone(&self) -> Self {
        Cgroup {
            subsystems: self.subsystems.clone(),
            hier: crate::hierarchies::auto(),
            path: self.path.clone(),
            specified_controllers: None,
        }
    }
}

impl Default for Cgroup {
    fn default() -> Self {
        Cgroup {
            subsystems: Vec::new(),
            hier: crate::hierarchies::auto(),
            path: "".to_string(),
            specified_controllers: None,
        }
    }
}

impl Cgroup {
    pub fn v2(&self) -> bool {
        self.hier.v2()
    }

    /// Create this control group.
    fn create(&self) -> Result<()> {
        if self.hier.v2() {
            create_v2_cgroup(self.hier.root(), &self.path, &self.specified_controllers)
        } else {
            for subsystem in &self.subsystems {
                subsystem.to_controller().create();
            }
            Ok(())
        }
    }

    /// Create a new control group in the hierarchy `hier`, with name `path`.
    ///
    /// Returns a handle to the control group that can be used to manipulate it.
    pub fn new<P: AsRef<Path>>(hier: Box<dyn Hierarchy>, path: P) -> Result<Cgroup> {
        let cg = Cgroup::load(hier, path);
        cg.create()?;
        Ok(cg)
    }

    /// Create a new control group in the hierarchy `hier`, with name `path`.
    ///
    /// Returns a handle to the control group that can be used to manipulate it.
    pub fn new_with_specified_controllers<P: AsRef<Path>>(
        hier: Box<dyn Hierarchy>,
        path: P,
        specified_controllers: Option<Vec<String>>,
    ) -> Result<Cgroup> {
        let cg = if let Some(sc) = specified_controllers {
            Cgroup::load_with_specified_controllers(hier, path, sc)
        } else {
            Cgroup::load(hier, path)
        };
        cg.create()?;
        Ok(cg)
    }

    /// Create a new control group in the hierarchy `hier`, with name `path` and `relative_paths`
    ///
    /// Returns a handle to the control group that can be used to manipulate it.
    ///
    /// Note that this method is only meaningful for cgroup v1, call it is equivalent to call `new` in the v2 mode.
    pub fn new_with_relative_paths<P: AsRef<Path>>(
        hier: Box<dyn Hierarchy>,
        path: P,
        relative_paths: HashMap<String, String>,
    ) -> Result<Cgroup> {
        let cg = Cgroup::load_with_relative_paths(hier, path, relative_paths);
        cg.create()?;
        Ok(cg)
    }

    /// Create a handle for a control group in the hierarchy `hier`, with name `path`.
    ///
    /// Returns a handle to the control group (that possibly does not exist until `create()` has
    /// been called on the cgroup.
    pub fn load<P: AsRef<Path>>(hier: Box<dyn Hierarchy>, path: P) -> Cgroup {
        let path = path.as_ref();
        let mut subsystems = hier.subsystems();
        if path.as_os_str() != "" {
            subsystems = subsystems
                .into_iter()
                .map(|x| x.enter(path))
                .collect::<Vec<_>>();
        }

        Cgroup {
            path: path.to_str().unwrap().to_string(),
            subsystems,
            hier,
            specified_controllers: None,
        }
    }

    /// Create a handle for a specified control group in the hierarchy `hier`, with name `path`.
    ///
    /// Returns a handle to the control group (that possibly does not exist until `create()` has
    /// been called on the cgroup.
    pub fn load_with_specified_controllers<P: AsRef<Path>>(
        hier: Box<dyn Hierarchy>,
        path: P,
        specified_controllers: Vec<String>,
    ) -> Cgroup {
        let path = path.as_ref();
        let mut subsystems = hier.subsystems();
        if path.as_os_str() != "" {
            subsystems = subsystems
                .into_iter()
                .filter(|x| specified_controllers.contains(&x.controller_name()))
                .map(|x| x.enter(path))
                .collect::<Vec<_>>();
        }

        Cgroup {
            path: path.to_str().unwrap().to_string(),
            subsystems,
            hier,
            specified_controllers: Some(specified_controllers),
        }
    }

    /// Create a handle for a control group in the hierarchy `hier`, with name `path` and `relative_paths`
    ///
    /// Returns a handle to the control group (that possibly does not exist until `create()` has
    /// been called on the cgroup.
    ///
    /// Note that this method is only meaningful for cgroup v1, call it is equivalent to call `load` in the v2 mode
    pub fn load_with_relative_paths<P: AsRef<Path>>(
        hier: Box<dyn Hierarchy>,
        path: P,
        relative_paths: HashMap<String, String>,
    ) -> Cgroup {
        // relative_paths only valid for cgroup v1
        if hier.v2() {
            return Self::load(hier, path);
        }

        let path = path.as_ref();
        let mut subsystems = hier.subsystems();
        if path.as_os_str() != "" {
            subsystems = subsystems
                .into_iter()
                .map(|x| {
                    let cn = x.controller_name();
                    if relative_paths.contains_key(&cn) {
                        let rp = relative_paths.get(&cn).unwrap();
                        let valid_path = rp.trim_start_matches('/').to_string();
                        let mut p = PathBuf::from(valid_path);
                        p.push(path);
                        x.enter(p.as_ref())
                    } else {
                        x.enter(path)
                    }
                })
                .collect::<Vec<_>>();
        }

        Cgroup {
            subsystems,
            hier,
            path: path.to_str().unwrap().to_string(),
            specified_controllers: None,
        }
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
    pub fn delete(&self) -> Result<()> {
        if self.v2() {
            if !self.path.is_empty() {
                let mut p = self.hier.root();
                p.push(self.path.clone());
                return fs::remove_dir(p).map_err(|e| Error::with_cause(RemoveFailed, e));
            }
            return Ok(());
        }

        self.subsystems.iter().try_for_each(|sub| match sub {
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
        })
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
    pub fn controller_of<'a, T>(&'a self) -> Option<&'a T>
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

    /// Removes tasks from the control group by thread group id.
    ///
    /// Note that this means that the task will be moved back to the root control group in the
    /// hierarchy and any rules applied to that control group will _still_ apply to the proc.
    pub fn remove_task_by_tgid(&self, tgid: CgroupPid) -> Result<()> {
        self.hier.root_control_group().add_task_by_tgid(tgid)
    }

    /// Removes a task from the control group.
    ///
    /// Note that this means that the task will be moved back to the root control group in the
    /// hierarchy and any rules applied to that control group will _still_ apply to the task.
    pub fn remove_task(&self, tid: CgroupPid) -> Result<()> {
        self.hier.root_control_group().add_task(tid)
    }

    /// Moves tasks to the parent control group by thread group id.
    pub fn move_task_to_parent_by_tgid(&self, tgid: CgroupPid) -> Result<()> {
        self.hier
            .parent_control_group(&self.path)
            .add_task_by_tgid(tgid)
    }

    /// Moves a task to the parent control group.
    pub fn move_task_to_parent(&self, tid: CgroupPid) -> Result<()> {
        self.hier.parent_control_group(&self.path).add_task(tid)
    }

    /// Return a handle to the parent control group in the hierarchy.
    pub fn parent_control_group(&self) -> Cgroup {
        self.hier.parent_control_group(&self.path)
    }

    /// Kill every process in the control group. Only supported for v2 cgroups and on
    /// kernels 5.14+. This will fail with InvalidOperation if the 'cgroup.kill' file does
    /// not exist.
    pub fn kill(&self) -> Result<()> {
        if !self.v2() {
            return Err(Error::new(CgroupVersion));
        }

        let val = "1";
        let file_name = "cgroup.kill";
        let p = self.hier.root().join(self.path.clone()).join(file_name);

        // If cgroup.kill doesn't exist they're not on 5.14+ so lets
        // surface some error the caller can check against.
        if !p.exists() {
            return Err(Error::new(InvalidOperation));
        }

        fs::write(p, val)
            .map_err(|e| Error::with_cause(WriteFailed(file_name.to_string(), val.to_string()), e))
    }

    /// Attach a task to the control group.
    pub fn add_task(&self, tid: CgroupPid) -> Result<()> {
        if self.v2() {
            let subsystems = self.subsystems();
            if !subsystems.is_empty() {
                let c = subsystems[0].to_controller();
                c.add_task(&tid)
            } else {
                Err(Error::new(SubsystemsEmpty))
            }
        } else {
            self.subsystems()
                .iter()
                .try_for_each(|sub| sub.to_controller().add_task(&tid))
        }
    }

    /// Attach tasks to the control group by thread group id.
    pub fn add_task_by_tgid(&self, tgid: CgroupPid) -> Result<()> {
        if self.v2() {
            let subsystems = self.subsystems();
            if !subsystems.is_empty() {
                let c = subsystems[0].to_controller();
                c.add_task_by_tgid(&tgid)
            } else {
                Err(Error::new(SubsystemsEmpty))
            }
        } else {
            self.subsystems()
                .iter()
                .try_for_each(|sub| sub.to_controller().add_task_by_tgid(&tgid))
        }
    }

    /// set cgroup.type
    pub fn set_cgroup_type(&self, cgroup_type: &str) -> Result<()> {
        if self.v2() {
            let subsystems = self.subsystems();
            if !subsystems.is_empty() {
                let c = subsystems[0].to_controller();
                c.set_cgroup_type(cgroup_type)
            } else {
                Err(Error::new(SubsystemsEmpty))
            }
        } else {
            Err(Error::new(CgroupVersion))
        }
    }

    /// get cgroup.type
    pub fn get_cgroup_type(&self) -> Result<String> {
        if self.v2() {
            let subsystems = self.subsystems();
            if !subsystems.is_empty() {
                let c = subsystems[0].to_controller();
                let cgroup_type = c.get_cgroup_type()?;
                Ok(cgroup_type)
            } else {
                Err(Error::new(SubsystemsEmpty))
            }
        } else {
            Err(Error::new(CgroupVersion))
        }
    }

    /// Set notify_on_release to the control group.
    pub fn set_notify_on_release(&self, enable: bool) -> Result<()> {
        self.subsystems()
            .iter()
            .try_for_each(|sub| sub.to_controller().set_notify_on_release(enable))
    }

    /// Set release_agent
    pub fn set_release_agent(&self, path: &str) -> Result<()> {
        self.hier
            .root_control_group()
            .subsystems()
            .iter()
            .try_for_each(|sub| sub.to_controller().set_release_agent(path))
    }

    /// Returns an Iterator that can be used to iterate over the procs that are currently in the
    /// control group.
    pub fn procs(&self) -> Vec<CgroupPid> {
        // Collect the procs from all subsystems
        let mut v = if self.v2() {
            let subsystems = self.subsystems();
            if !subsystems.is_empty() {
                let c = subsystems[0].to_controller();
                c.procs()
            } else {
                vec![]
            }
        } else {
            self.subsystems()
                .iter()
                .map(|x| x.to_controller().procs())
                .fold(vec![], |mut acc, mut x| {
                    acc.append(&mut x);
                    acc
                })
        };

        v.sort();
        v.dedup();
        v
    }

    /// Returns an Iterator that can be used to iterate over the tasks that are currently in the
    /// control group.
    pub fn tasks(&self) -> Vec<CgroupPid> {
        // Collect the tasks from all subsystems
        let mut v = if self.v2() {
            let subsystems = self.subsystems();
            if !subsystems.is_empty() {
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

pub const UNIFIED_MOUNTPOINT: &str = "/sys/fs/cgroup";

fn enable_controllers(controllers: &[String], path: &Path) {
    let f = path.join("cgroup.subtree_control");
    for c in controllers {
        let body = format!("+{}", c);
        let _rest = fs::write(f.as_path(), body.as_bytes());
    }
}

fn supported_controllers() -> Vec<String> {
    let p = format!("{}/{}", UNIFIED_MOUNTPOINT, "cgroup.controllers");
    let ret = fs::read_to_string(p.as_str());
    ret.unwrap_or_default()
        .split(' ')
        .map(|x| x.to_string())
        .collect::<Vec<String>>()
}

fn create_v2_cgroup(
    root: PathBuf,
    path: &str,
    specified_controllers: &Option<Vec<String>>,
) -> Result<()> {
    // controler list ["memory", "cpu"]
    let controllers = if let Some(s_controllers) = specified_controllers.clone() {
        if verify_supported_controllers(s_controllers.as_ref()) {
            s_controllers
        } else {
            return Err(Error::new(ErrorKind::SpecifiedControllers));
        }
    } else {
        supported_controllers()
    };

    let mut fp = root;

    // enable for root
    enable_controllers(&controllers, &fp);

    // path: "a/b/c"
    let elements = path.split('/').collect::<Vec<&str>>();
    let last_index = elements.len() - 1;
    for (i, ele) in elements.iter().enumerate() {
        // ROOT/a
        fp.push(ele);
        // create dir, need not check if is a file or directory
        if !fp.exists() {
            if let Err(e) = std::fs::create_dir(fp.clone()) {
                return Err(Error::with_cause(ErrorKind::FsError, e));
            }
        }

        if i < last_index {
            // enable controllers for substree
            enable_controllers(&controllers, &fp);
        }
    }

    Ok(())
}

pub fn verify_supported_controllers(controllers: &[String]) -> bool {
    let sc = supported_controllers();
    for controller in controllers.iter() {
        if !sc.contains(controller) {
            return false;
        }
    }
    true
}

pub fn get_cgroups_relative_paths() -> Result<HashMap<String, String>> {
    let path = "/proc/self/cgroup".to_string();
    get_cgroups_relative_paths_by_path(path)
}

pub fn get_cgroups_relative_paths_by_pid(pid: u32) -> Result<HashMap<String, String>> {
    let path = format!("/proc/{}/cgroup", pid);
    get_cgroups_relative_paths_by_path(path)
}

fn get_cgroups_relative_paths_by_path(path: String) -> Result<HashMap<String, String>> {
    let mut m = HashMap::new();
    let content =
        fs::read_to_string(path.clone()).map_err(|e| Error::with_cause(ReadFailed(path), e))?;
    for l in content.lines() {
        let fl: Vec<&str> = l.split(':').collect();
        if fl.len() != 3 {
            continue;
        }

        let keys: Vec<&str> = fl[1].split(',').collect();
        for key in &keys {
            m.insert(key.to_string(), fl[2].to_string());
        }
    }
    Ok(m)
}
