// Copyright (c) 2018 Levente Kurusa
// Copyright (c) 2020 And Group
//
// SPDX-License-Identifier: Apache-2.0 or MIT
//

use cgroups_rs::cpuset::CpuSetController;
use cgroups_rs::error::ErrorKind;
use cgroups_rs::{Cgroup, CgroupPid};

use std::fs;

#[test]
fn test_cpuset_memory_pressure_root_cg() {
    let h = cgroups_rs::hierarchies::auto();
    let cg = Cgroup::new(h, String::from("test_cpuset_memory_pressure_root_cg"));
    {
        let cpuset: &CpuSetController = cg.controller_of().unwrap();

        // This is not a root control group, so it should fail via InvalidOperation.
        let res = cpuset.set_enable_memory_pressure(true);
        assert_eq!(res.unwrap_err().kind(), &ErrorKind::InvalidOperation);
    }
    cg.delete().unwrap();
}

#[test]
fn test_cpuset_set_cpus() {
    let h = cgroups_rs::hierarchies::auto();
    let cg = Cgroup::new(h, String::from("test_cpuset_set_cpus"));
    {
        let cpuset: &CpuSetController = cg.controller_of().unwrap();

        let set = cpuset.cpuset();
        if cg.v2() {
            assert_eq!(0, set.cpus.len());
        } else {
            // for cgroup v1, cpuset is copied from parent.
            assert_eq!(true, set.cpus.len() > 0);
        }

        // 0
        let r = cpuset.set_cpus("0");
        assert_eq!(true, r.is_ok());

        let set = cpuset.cpuset();
        assert_eq!(1, set.cpus.len());
        assert_eq!((0, 0), set.cpus[0]);

        // all cpus in system
        let cpus =
            fs::read_to_string("/sys/fs/cgroup/cpuset.cpus.effective").unwrap_or("".to_string());
        let cpus = cpus.trim();
        if cpus != "" {
            let r = cpuset.set_cpus(&cpus);
            assert_eq!(true, r.is_ok());
            let set = cpuset.cpuset();
            assert_eq!(1, set.cpus.len());
            assert_eq!(format!("{}-{}", set.cpus[0].0, set.cpus[0].1), cpus);
        }
    }
    cg.delete().unwrap();
}

#[test]
fn test_cpuset_set_cpus_add_task() {
    let h = cgroups_rs::hierarchies::auto();
    let cg = Cgroup::new(h, String::from("test_cpuset_set_cpus_add_task/sub-dir"));

    let cpuset: &CpuSetController = cg.controller_of().unwrap();
    let set = cpuset.cpuset();
    if cg.v2() {
        assert_eq!(0, set.cpus.len());
    } else {
        // for cgroup v1, cpuset is copied from parent.
        assert_eq!(true, set.cpus.len() > 0);
    }

    // Add a task to the control group.
    let pid_i = libc::pid_t::from(nix::unistd::getpid()) as u64;
    let _ = cg.add_task(CgroupPid::from(pid_i));
    let tasks = cg.tasks();
    assert_eq!(true, tasks.len() > 0);
    println!("tasks after added: {:?}", tasks);

    // remove task
    let _ = cg.remove_task(CgroupPid::from(pid_i));
    let tasks = cg.tasks();
    println!("tasks after deleted: {:?}", tasks);
    assert_eq!(0, tasks.len());

    cg.delete().unwrap();
}
