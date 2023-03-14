// Copyright (c) 2020 And Group
//
// SPDX-License-Identifier: Apache-2.0 or MIT
//

//! Integration tests about the hugetlb subsystem
use cgroups_rs::memory::{MemController, SetMemory};
use cgroups_rs::Controller;
use cgroups_rs::{Cgroup, MaxValue};

#[test]
fn test_disable_oom_killer() {
    let h = cgroups_rs::hierarchies::auto();
    let cg = Cgroup::new(h, String::from("test_disable_oom_killer")).unwrap();
    {
        let mem_controller: &MemController = cg.controller_of().unwrap();

        // before disable
        let m = mem_controller.memory_stat();
        assert!(!m.oom_control.oom_kill_disable);

        // now only v1
        if !mem_controller.v2() {
            // disable oom killer
            let r = mem_controller.disable_oom_killer();
            assert!(r.is_ok());

            // after disable
            let m = mem_controller.memory_stat();
            assert!(m.oom_control.oom_kill_disable);
        }
    }
    cg.delete().unwrap();
}

#[test]
fn set_kmem_limit_v1() {
    let h = cgroups_rs::hierarchies::auto();
    if h.v2() {
        return;
    }

    let cg = Cgroup::new(h, String::from("set_kmem_limit_v1")).unwrap();
    {
        let mem_controller: &MemController = cg.controller_of().unwrap();
        mem_controller.set_kmem_limit(1).unwrap();
    }
    cg.delete().unwrap();
}

#[test]
fn set_mem_v2() {
    let h = cgroups_rs::hierarchies::auto();
    if !h.v2() {
        return;
    }

    let cg = Cgroup::new(h, String::from("set_mem_v2")).unwrap();
    {
        let mem_controller: &MemController = cg.controller_of().unwrap();

        // before disable
        let m = mem_controller.get_mem().unwrap();
        // case 1: get default value
        assert_eq!(m.low, Some(MaxValue::Value(0)));
        assert_eq!(m.min, Some(MaxValue::Value(0)));
        assert_eq!(m.high, Some(MaxValue::Max));
        assert_eq!(m.max, Some(MaxValue::Max));

        // case 2: set parts
        let m = SetMemory {
            low: Some(MaxValue::Value(1024 * 1024 * 2)),
            high: Some(MaxValue::Value(1024 * 1024 * 1024 * 2)),
            min: Some(MaxValue::Value(1024 * 1024 * 3)),
            max: None,
        };
        let r = mem_controller.set_mem(m);
        assert!(r.is_ok());

        let m = mem_controller.get_mem().unwrap();
        // get
        assert_eq!(m.low, Some(MaxValue::Value(1024 * 1024 * 2)));
        assert_eq!(m.min, Some(MaxValue::Value(1024 * 1024 * 3)));
        assert_eq!(m.high, Some(MaxValue::Value(1024 * 1024 * 1024 * 2)));
        assert_eq!(m.max, Some(MaxValue::Max));

        // case 3: set parts
        let m = SetMemory {
            max: Some(MaxValue::Value(1024 * 1024 * 1024 * 2)),
            min: Some(MaxValue::Value(1024 * 1024 * 4)),
            high: Some(MaxValue::Max),
            low: None,
        };
        let r = mem_controller.set_mem(m);
        assert!(r.is_ok());

        let m = mem_controller.get_mem().unwrap();
        // get
        assert_eq!(m.low, Some(MaxValue::Value(1024 * 1024 * 2)));
        assert_eq!(m.min, Some(MaxValue::Value(1024 * 1024 * 4)));
        assert_eq!(m.max, Some(MaxValue::Value(1024 * 1024 * 1024 * 2)));
        assert_eq!(m.high, Some(MaxValue::Max));
    }

    cg.delete().unwrap();
}
