// Copyright (c) 2019-2021 Alibaba Cloud
// Copyright (c) 2019-2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::ops::Deref;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

use cgroups::{Cgroup, CgroupPid, Controllers, Hierarchy, Subsystem};
use lazy_static::lazy_static;
use once_cell::sync::Lazy;

use crate::sl;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Can not add tgid {0} to cgroup, {1:?}")]
    AddTgid(u64, #[source] cgroups::error::Error),
    #[error("failed to apply resources to cgroup: {0:?}")]
    ApplyResource(#[source] cgroups::error::Error),
    #[error("failed to delete cgroup after {0} retries")]
    DeleteCgroup(u64),
    #[error("Invalid cgroup path {0}")]
    InvalidCgroupPath(String),
}

pub type Result<T> = std::result::Result<T, Error>;

lazy_static! {
    /// Disable cgroup v1 subsystems.
    pub static ref DISABLED_HIERARCHIES: Mutex<Vec<cgroups::Controllers>> = Mutex::new(Vec::new());
}

/// Update the disabled cgroup subsystems.
///
/// Some cgroup controllers may be disabled by runtime configuration file. The sandbox may call
/// this method to disable those cgroup controllers once.
pub fn update_disabled_cgroup_list(hierarchies: &[String]) {
    let mut disabled_hierarchies = DISABLED_HIERARCHIES.lock().unwrap();
    disabled_hierarchies.clear();
    for hierarchy in hierarchies {
        //disabled_hierarchies.push(hie.clone());
        match hierarchy.as_str() {
            "blkio" => disabled_hierarchies.push(cgroups::Controllers::BlkIo),
            "cpu" => disabled_hierarchies.push(cgroups::Controllers::Cpu),
            "cpuset" => disabled_hierarchies.push(cgroups::Controllers::CpuSet),
            "cpuacct" => disabled_hierarchies.push(cgroups::Controllers::CpuAcct),
            "devices" => disabled_hierarchies.push(cgroups::Controllers::Devices),
            "freezer" => disabled_hierarchies.push(cgroups::Controllers::Freezer),
            "hugetlb" => disabled_hierarchies.push(cgroups::Controllers::HugeTlb),
            "memory" => disabled_hierarchies.push(cgroups::Controllers::Mem),
            "net_cls" => disabled_hierarchies.push(cgroups::Controllers::NetCls),
            "net_prio" => disabled_hierarchies.push(cgroups::Controllers::NetPrio),
            "perf_event" => disabled_hierarchies.push(cgroups::Controllers::PerfEvent),
            "pids" => disabled_hierarchies.push(cgroups::Controllers::Pids),
            "systemd" => disabled_hierarchies.push(cgroups::Controllers::Systemd),
            _ => warn!(sl!(), "unknown cgroup controller {}", hierarchy),
        }
    }
    debug!(
        sl!(),
        "disable cgroup list {:?} from {:?}", disabled_hierarchies, hierarchies
    );
}

/// Filter out disabled cgroup subsystems.
pub fn filter_disabled_cgroup(controllers: &mut Vec<Controllers>) {
    let disabled_hierarchies = DISABLED_HIERARCHIES.lock().unwrap();
    controllers.retain(|x| !disabled_hierarchies.contains(x));
}

#[derive(Copy, Clone, Debug)]
pub enum PidType {
    /// Add pid to `tasks`
    Tasks,
    /// Add pid to `cgroup.procs`
    CgroupProcs,
}

/// Get the singleton instance for cgroup v1 hierarchy object.
pub fn get_cgroup_hierarchies() -> &'static cgroups::hierarchies::V1 {
    static GLOBAL: Lazy<cgroups::hierarchies::V1> = Lazy::new(cgroups::hierarchies::V1::new);
    GLOBAL.deref()
}

// Prepend a kata specific string to oci cgroup path to form a different cgroup path, thus cAdvisor
// couldn't find kata containers cgroup path on host to prevent it from grabbing the stats data.
const CGROUP_KATA_PREFIX: &str = "kata";

/// Convert to a Kata specific cgroup path.
pub fn gen_kata_cgroup_path(path: &str) -> PathBuf {
    // Be careful to trim off the possible '/' prefix. Joining an absolute path to a `Path` object
    // will replace the old `Path` instead of concat.
    Path::new(CGROUP_KATA_PREFIX).join(path.trim_start_matches('/'))
}

/// Convert to a cgroup path for K8S sandbox.
pub fn gen_sandbox_cgroup_path(path: &str) -> PathBuf {
    PathBuf::from(path)
}

/// A customized cgroup v1 hierarchy object with configurable filters for supported subsystems.
#[derive(Debug)]
pub struct V1Customized {
    mount_point: PathBuf,
    controllers: Vec<Controllers>,
}

impl V1Customized {
    /// Create a new instance of [`V1Customized`].
    ///
    /// The `controllers` configures the subsystems to enable.
    ///
    /// Note :
    /// 1. When enabling both blkio and memory cgroups, blkio cgroup must be enabled before memory
    ///    cgroup due to a limitation in writeback control of blkio cgroup.
    /// 2. cpu, cpuset, cpuacct should be adjacent to each other.
    pub fn new(controllers: Vec<Controllers>) -> Self {
        let mount_point = get_cgroup_hierarchies().root();

        V1Customized {
            mount_point,
            controllers,
        }
    }
}

impl Hierarchy for V1Customized {
    fn subsystems(&self) -> Vec<Subsystem> {
        let subsystems = get_cgroup_hierarchies().subsystems();

        subsystems
            .into_iter()
            .filter(|sub| {
                self.controllers
                    .contains(&sub.to_controller().control_type())
            })
            .collect::<Vec<_>>()
    }

    fn root(&self) -> PathBuf {
        self.mount_point.clone()
    }

    fn root_control_group(&self) -> Cgroup {
        Cgroup::load(Box::new(V1Customized::new(self.controllers.clone())), "")
    }

    fn v2(&self) -> bool {
        false
    }
}

/// An boxed cgroup hierarchy object.
pub type BoxedHierarchyObject = Box<dyn Hierarchy>;

/// Create a cgroup hierarchy object with all subsystems disabled.
pub fn get_empty_hierarchy() -> BoxedHierarchyObject {
    Box::new(V1Customized::new(vec![]))
}

/// Create a cgroup hierarchy object for pod sandbox.
pub fn get_sandbox_hierarchy(no_mem: bool) -> BoxedHierarchyObject {
    let mut controllers = vec![
        cgroups::Controllers::BlkIo,
        cgroups::Controllers::Cpu,
        cgroups::Controllers::CpuSet,
        cgroups::Controllers::CpuAcct,
        cgroups::Controllers::PerfEvent,
    ];

    if !no_mem {
        controllers.push(cgroups::Controllers::Mem);
    }
    filter_disabled_cgroup(&mut controllers);
    Box::new(V1Customized::new(controllers))
}

/// Create a cgroup hierarchy object with mem subsystem.
///
/// Note: the mem subsystem may have been disabled, so it will get filtered out.
pub fn get_mem_hierarchy() -> BoxedHierarchyObject {
    let mut controllers = vec![cgroups::Controllers::Mem];
    filter_disabled_cgroup(&mut controllers);
    Box::new(V1Customized::new(controllers))
}

/// Create a cgroup hierarchy object with CPU related subsystems.
///
/// Note: the mem subsystem may have been disabled, so it will get filtered out.
pub fn get_cpu_hierarchy() -> BoxedHierarchyObject {
    let mut controllers = vec![
        cgroups::Controllers::Cpu,
        cgroups::Controllers::CpuSet,
        cgroups::Controllers::CpuAcct,
    ];
    filter_disabled_cgroup(&mut controllers);
    Box::new(V1Customized::new(controllers))
}

/// Get cgroup hierarchy object from `path`.
pub fn get_hierarchy_by_path(path: &str) -> Result<BoxedHierarchyObject> {
    let v1 = get_cgroup_hierarchies().clone();
    let valid_path = valid_cgroup_path(path)?;
    let cg = cgroups::Cgroup::load(Box::new(v1), valid_path.as_str());

    let mut hierarchy = vec![];
    for subsys in cg.subsystems() {
        let controller = subsys.to_controller();
        if controller.exists() {
            hierarchy.push(controller.control_type());
        }
    }

    Ok(Box::new(V1Customized::new(hierarchy)))
}

/// Create or load a cgroup object from a path.
pub fn create_or_load_cgroup(path: &str) -> Result<Cgroup> {
    let hie = Box::new(get_cgroup_hierarchies().clone());

    create_or_load_cgroup_with_hier(hie, path)
}

/// Create or load a cgroup v1 object from a path, with a given hierarchy object.
pub fn create_or_load_cgroup_with_hier(hie: BoxedHierarchyObject, path: &str) -> Result<Cgroup> {
    let valid_path = valid_cgroup_path(path)?;
    if is_cgroup_exist(valid_path.as_str()) {
        Ok(cgroups::Cgroup::load(hie, valid_path.as_str()))
    } else {
        Ok(cgroups::Cgroup::new(hie, valid_path.as_str()))
    }
}

/// Check whether `path` hosts a cgroup hierarchy directory.
pub fn is_cgroup_exist(path: &str) -> bool {
    let valid_path = match valid_cgroup_path(path) {
        Ok(v) => v,
        Err(e) => {
            warn!(sl!(), "{}", e);
            return false;
        }
    };

    let v1 = get_cgroup_hierarchies().clone();
    let cg = cgroups::Cgroup::load(Box::new(v1), valid_path.as_str());
    for subsys in cg.subsystems() {
        if subsys.to_controller().exists() {
            debug!(sl!(), "cgroup {} exist", path);
            return true;
        }
    }

    false
}

// Validate the cgroup path is a relative path, do not include ".", "..".
fn valid_cgroup_path(path: &str) -> Result<String> {
    let path = path.trim_start_matches('/').to_string();

    for comp in Path::new(&path).components() {
        if !matches!(comp, Component::Normal(_)) {
            return Err(Error::InvalidCgroupPath(path.to_string()));
        }
    }

    Ok(path)
}

/// Remove all task from cgroup and delete the cgroup.
pub fn force_delete_cgroup(cg: cgroups::Cgroup) -> Result<()> {
    delete_cgroup_with_retry(cg, |cg: &Cgroup| {
        // if task exist need to delete first.
        for cg_pid in cg.tasks() {
            warn!(sl!(), "Delete cgroup task pid {}", cg_pid.pid);
            cg.remove_task(cg_pid);
        }
    })
}

/// Try to delete a cgroup, call the `do_process` handler at each iteration.
pub fn delete_cgroup_with_retry<F>(cg: Cgroup, mut do_process: F) -> Result<()>
where
    F: FnMut(&Cgroup),
{
    // sleep DURATION
    const SLEEP_MILLISECS: u64 = 10;
    const RETRY_COUNT: u64 = 200;

    // In case of deletion failure caused by "Resource busy", sleep DURATION and retry RETRY times.
    for index in 0..RETRY_COUNT {
        do_process(&cg);

        if cg.delete().is_ok() {
            if index > 0 {
                info!(
                    sl!(),
                    "cgroup delete cgroup cost {} ms, retry {} times",
                    index * SLEEP_MILLISECS,
                    index,
                );
            }
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(SLEEP_MILLISECS))
    }

    Err(Error::DeleteCgroup(RETRY_COUNT))
}

/// Move the process `pid` into the cgroup `to`.
pub fn move_tgid(pid: u64, to: &Cgroup) -> Result<()> {
    info!(sl!(), "try to move tid {:?}", pid);
    to.add_task_by_tgid(CgroupPid::from(pid))
        .map_err(|e| Error::AddTgid(pid, e))
}

/// Move all processes tasks from `from` to `to`.
pub fn move_cgroup_task(from: &Cgroup, to: &Cgroup) -> Result<()> {
    info!(sl!(), "try to move tasks {:?}", from.tasks());
    for cg_pid in from.tasks() {
        from.remove_task(CgroupPid::from(cg_pid.pid));
        // TODO: enhance cgroups to implement Copy for CgroupPid
        // https://github.com/kata-containers/cgroups-rs/issues/70
        let pid = cg_pid.pid;
        to.add_task(cg_pid).map_err(|e| Error::AddTgid(pid, e))?;
    }

    Ok(())
}

/// Associate a group of tasks with a cgroup, and optionally configure resources for the cgroup.
pub fn update_cgroup_task_resources(
    hierarchy: BoxedHierarchyObject,
    path: &str,
    pids: &[u64],
    pid_type: PidType,
    resources: Option<&cgroups::Resources>,
) -> Result<()> {
    if hierarchy.subsystems().is_empty() {
        return Ok(());
    }
    fail::fail_point!("update_cgroup_task_resources", |_| { () });

    let cg = create_or_load_cgroup_with_hier(hierarchy, path)?;
    for pid in pids {
        let result = match pid_type {
            PidType::Tasks => cg.add_task(CgroupPid { pid: *pid }),
            PidType::CgroupProcs => cg.add_task_by_tgid(CgroupPid { pid: *pid }),
        };
        if let Err(err) = result {
            return Err(Error::AddTgid(*pid, err));
        }
    }

    if let Some(res) = resources {
        cg.apply(res).map_err(Error::ApplyResource)?;
    }

    debug!(
        sl!(),
        "update {:?} {:?} resources {:?} for cgroup {}", pid_type, pids, resources, path
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cgroups::Controllers;
    use serial_test::serial;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static GLOBAL_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn gen_test_path() -> String {
        let pid = nix::unistd::getpid().as_raw();
        let index = GLOBAL_COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = format!("kata-tests-{}-{}", pid, index);
        println!("test path {}", path);
        path
    }

    fn get_hierarchy(controllers: Vec<Controllers>) -> Box<V1Customized> {
        Box::new(V1Customized::new(controllers))
    }

    #[test]
    fn test_v1_customized_cgroup() {
        update_disabled_cgroup_list(&[]);

        let c = V1Customized::new(vec![]);
        assert_eq!(c.subsystems().len(), 0);
        assert!(!c.v2());

        let c = V1Customized::new(vec![Controllers::Cpu, Controllers::CpuSet]);
        assert_eq!(c.subsystems().len(), 2);
        assert!(!c.v2());
    }

    #[test]
    #[serial]
    fn test_filter_disabled_cgroup() {
        update_disabled_cgroup_list(&[]);
        assert_eq!(DISABLED_HIERARCHIES.lock().unwrap().len(), 0);

        let disabeld = ["perf_event".to_string()];
        update_disabled_cgroup_list(&disabeld);
        assert_eq!(DISABLED_HIERARCHIES.lock().unwrap().len(), 1);
        assert_eq!(
            DISABLED_HIERARCHIES.lock().unwrap()[0],
            Controllers::PerfEvent
        );

        let mut subsystems = vec![Controllers::BlkIo, Controllers::PerfEvent, Controllers::Cpu];
        filter_disabled_cgroup(&mut subsystems);
        assert_eq!(subsystems.len(), 2);
        assert_eq!(subsystems[0], Controllers::BlkIo);
        assert_eq!(subsystems[1], Controllers::Cpu);

        let disabeld = ["cpu".to_string(), "cpuset".to_string()];
        update_disabled_cgroup_list(&disabeld);
        assert_eq!(DISABLED_HIERARCHIES.lock().unwrap().len(), 2);

        let mut subsystems = vec![Controllers::BlkIo, Controllers::PerfEvent, Controllers::Cpu];
        filter_disabled_cgroup(&mut subsystems);
        assert_eq!(subsystems.len(), 2);
        assert_eq!(subsystems[0], Controllers::BlkIo);
        assert_eq!(subsystems[1], Controllers::PerfEvent);

        update_disabled_cgroup_list(&[]);
    }

    #[test]
    fn test_create_empty_hierarchy() {
        update_disabled_cgroup_list(&[]);

        let controller = get_empty_hierarchy();
        assert_eq!(controller.subsystems().len(), 0);
        assert!(!controller.root_control_group().v2());
    }

    #[test]
    #[serial]
    fn test_create_sandbox_hierarchy() {
        update_disabled_cgroup_list(&[]);

        let controller = get_sandbox_hierarchy(true);
        assert_eq!(controller.subsystems().len(), 5);
        assert!(!controller.root_control_group().v2());

        let controller = get_sandbox_hierarchy(false);
        assert_eq!(controller.subsystems().len(), 6);
        assert!(!controller.root_control_group().v2());
    }

    #[test]
    #[serial]
    fn test_get_hierarchy() {
        update_disabled_cgroup_list(&[]);

        let controller = get_mem_hierarchy();
        assert!(!controller.v2());
        assert_eq!(controller.subsystems().len(), 1);

        let controller = get_cpu_hierarchy();
        assert!(!controller.v2());
        assert_eq!(controller.subsystems().len(), 3);
    }

    #[test]
    #[serial]
    fn test_create_cgroup_default() {
        update_disabled_cgroup_list(&[]);
        // test need root permission
        if !nix::unistd::getuid().is_root() {
            println!("test need root permission");
            return;
        }

        let v1 = Box::new(cgroups::hierarchies::V1::new());
        let test_path = gen_test_path();
        let cg_path = test_path.as_str();
        assert!(!is_cgroup_exist(cg_path));

        // new cgroup
        let cg = cgroups::Cgroup::new(v1, cg_path);
        assert!(is_cgroup_exist(cg_path));

        // add task
        let _ = cg.add_task(cgroups::CgroupPid {
            pid: nix::unistd::getpid().as_raw() as u64,
        });

        // delete cgroup
        force_delete_cgroup(cg).unwrap();
        assert!(!is_cgroup_exist(cg_path));
    }

    #[test]
    #[serial]
    fn test_create_cgroup_cpus() {
        update_disabled_cgroup_list(&[]);
        // test need root permission
        if !nix::unistd::getuid().is_root() {
            println!("test need root permission");
            return;
        }
        if num_cpus::get() <= 1 {
            println!("The unit test is only supported on SMP systems.");
            return;
        }

        let test_path = gen_test_path();
        let cg_path = test_path.as_str();
        assert!(!is_cgroup_exist(cg_path));

        // new cgroup
        let cgroup = create_or_load_cgroup(cg_path).unwrap();
        let cpus: &cgroups::cpuset::CpuSetController = cgroup.controller_of().unwrap();
        cpus.set_cpus("0-1").unwrap();
        assert!(is_cgroup_exist(cg_path));

        // current cgroup
        let current_cgroup = create_or_load_cgroup(cg_path).unwrap();
        let current_cpus: &cgroups::cpuset::CpuSetController =
            current_cgroup.controller_of().unwrap();
        // check value
        assert_eq!(cpus.cpuset().cpus, current_cpus.cpuset().cpus);

        // delete cgroup
        force_delete_cgroup(cgroup).unwrap();
        assert!(!is_cgroup_exist(cg_path));
    }

    #[test]
    #[serial]
    fn test_create_cgroup_with_parent() {
        update_disabled_cgroup_list(&[]);
        // test need root permission
        if !nix::unistd::getuid().is_root() {
            println!("test need root permission");
            return;
        }
        if num_cpus::get() <= 1 {
            println!("The unit test is only supported on SMP systems.");
            return;
        }

        let test_path = gen_test_path();
        let cg_path = test_path.as_str();
        assert!(!is_cgroup_exist(cg_path));

        // new cgroup
        let cg = create_or_load_cgroup(cg_path).unwrap();
        let cpus: &cgroups::cpuset::CpuSetController = cg.controller_of().unwrap();
        cpus.set_cpus("0-1").unwrap();
        assert!(is_cgroup_exist(cg_path));

        // new cgroup 1
        let cg_test_path_1 = format!("{}/vcpu0", test_path);
        let cg_path_1 = cg_test_path_1.as_str();
        let cg1 = create_or_load_cgroup(cg_path_1).unwrap();
        let cpus1: &cgroups::cpuset::CpuSetController = cg1.controller_of().unwrap();
        cpus1.set_cpus("0").unwrap();
        assert!(is_cgroup_exist(cg_path_1));

        // new cgroup 2
        let cg_test_path_2 = format!("{}/vcpu1", test_path);
        let cg_path_2 = cg_test_path_2.as_str();
        // new cgroup
        let cg2 = create_or_load_cgroup(cg_path_2).unwrap();
        let cpus2: &cgroups::cpuset::CpuSetController = cg2.controller_of().unwrap();
        cpus2.set_cpus("1").unwrap();
        assert!(is_cgroup_exist(cg_path_2));

        // must delete sub dir first
        force_delete_cgroup(cg1).unwrap();
        assert!(!is_cgroup_exist(cg_path_1));
        force_delete_cgroup(cg2).unwrap();
        assert!(!is_cgroup_exist(cg_path_2));
        force_delete_cgroup(cg).unwrap();
        assert!(!is_cgroup_exist(cg_path));
    }

    fn assert_customize_path_exist(path: &str, current_subsystems: &[Subsystem], expect: bool) {
        println!("assert customize path {} exist expect {}", path, expect);
        let v1 = Box::new(cgroups::hierarchies::V1::new());
        let v1_cg = Cgroup::load(v1, path);
        let v1_subsystems = v1_cg.subsystems();

        for v1_sub in v1_subsystems {
            let check_expect = || -> bool {
                for current_sub in current_subsystems {
                    if v1_sub.to_controller().control_type()
                        == current_sub.to_controller().control_type()
                    {
                        return expect;
                    }
                }
                false
            }();
            assert_eq!(
                check_expect,
                v1_sub.to_controller().exists(),
                "failed to check path {:?} subsystem {:?}",
                path,
                v1_sub
            )
        }
    }

    fn clean_cgroup_v1(path: &str) {
        let v1 = Box::new(cgroups::hierarchies::V1::new());
        let cg = Cgroup::load(v1.clone(), path);
        delete_cgroup_with_retry(cg, |_: &Cgroup| {}).unwrap();

        let check_cg = Cgroup::load(v1, path);
        assert_customize_path_exist(path, check_cg.subsystems(), false);
    }

    #[test]
    #[serial]
    fn test_customize_hierarchies() {
        update_disabled_cgroup_list(&[]);
        // test need root permission
        if !nix::unistd::getuid().is_root() {
            println!("test need root permission");
            return;
        }

        let cg_path_1 = "test_customize_hierarchies1";
        let cg_path_2 = "test_customize_hierarchies2";

        // clean
        clean_cgroup_v1(cg_path_1);
        clean_cgroup_v1(cg_path_2);

        // check customized cgroup
        // With some kernels, Cpu and CpuAcct are combined into one directory, so enable both
        // to ease test code.
        let controllers_1 = vec![Controllers::Cpu, Controllers::CpuAcct];
        let controllers_2 = vec![Controllers::Cpu, Controllers::CpuSet, Controllers::CpuAcct];
        let cg_1 = Cgroup::new(get_hierarchy(controllers_1.clone()), cg_path_1);
        let cg_2 = Cgroup::new(get_hierarchy(controllers_2.clone()), cg_path_2);

        assert_customize_path_exist(cg_path_1, cg_1.subsystems(), true);
        assert_customize_path_exist(cg_path_2, cg_2.subsystems(), true);

        // delete
        let _ = cg_1.delete();
        let _ = cg_2.delete();

        // check after delete
        let check_cg_1 = Cgroup::load(get_hierarchy(controllers_1), cg_path_1);
        let check_cg_2 = Cgroup::load(get_hierarchy(controllers_2), cg_path_2);
        assert_customize_path_exist(cg_path_1, check_cg_1.subsystems(), false);
        assert_customize_path_exist(cg_path_2, check_cg_2.subsystems(), false);
    }

    #[test]
    #[serial]
    fn test_task_move() {
        update_disabled_cgroup_list(&[]);
        // test need root permission
        if !nix::unistd::getuid().is_root() {
            println!("test need root permission");
            return;
        }

        let cg_path_1 = "test_task_move_before";
        let cg_path_2 = "test_task_move_after";

        // clean
        clean_cgroup_v1(cg_path_1);
        clean_cgroup_v1(cg_path_2);

        // With some kernels, Cpu and CpuAcct are combined into one directory, so enable both
        // to ease test code.
        let controllers = vec![Controllers::Cpu, Controllers::CpuAcct];
        let cg_1 = Cgroup::new(get_hierarchy(controllers.clone()), cg_path_1);
        let cg_2 = Cgroup::new(get_hierarchy(controllers.clone()), cg_path_2);

        assert_customize_path_exist(cg_path_1, cg_1.subsystems(), true);
        assert_customize_path_exist(cg_path_2, cg_2.subsystems(), true);

        // add task
        let pid = libc::pid_t::from(nix::unistd::getpid()) as u64;
        let _ = cg_1.add_task(CgroupPid::from(pid)).unwrap();
        let mut cg_task_1 = cg_1.tasks();
        let mut cg_task_2 = cg_2.tasks();
        assert_eq!(1, cg_task_1.len());
        assert_eq!(0, cg_task_2.len());

        // move task
        let _ = cg_2.add_task(CgroupPid::from(pid)).unwrap();
        cg_task_1 = cg_1.tasks();
        cg_task_2 = cg_2.tasks();
        assert_eq!(0, cg_task_1.len());
        assert_eq!(1, cg_task_2.len());

        cg_2.remove_task(CgroupPid::from(pid));

        // delete
        cg_1.delete().unwrap();
        // delete cg_2 with retry because of possible unknown failed
        // caused by "Resource busy", we do the same in the production
        // code, so it makes sense in the test.
        delete_cgroup_with_retry(cg_2, |_| {}).unwrap();

        // check after delete
        let check_cg_1 = Cgroup::load(get_hierarchy(controllers.clone()), cg_path_1);
        let check_cg_2 = Cgroup::load(get_hierarchy(controllers), cg_path_2);
        assert_customize_path_exist(cg_path_1, check_cg_1.subsystems(), false);
        assert_customize_path_exist(cg_path_2, check_cg_2.subsystems(), false);
    }

    #[test]
    fn test_gen_kata_cgroup_path() {
        assert_eq!(
            &gen_kata_cgroup_path("sandbox1/container2"),
            Path::new("kata/sandbox1/container2")
        );
        assert_eq!(
            &gen_kata_cgroup_path("/sandbox1/container2"),
            Path::new("kata/sandbox1/container2")
        );
        assert_eq!(
            &gen_kata_cgroup_path("/sandbox1:container2"),
            Path::new("kata/sandbox1:container2")
        );
    }
}
