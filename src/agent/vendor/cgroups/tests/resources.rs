// Copyright (c) 2018 Levente Kurusa
// Copyright (c) 2020 And Group
//
// SPDX-License-Identifier: Apache-2.0 or MIT
//

//! Integration test about setting resources using `apply()`
use cgroups::pid::PidController;
use cgroups::{Cgroup, Hierarchy, MaxValue, PidResources, Resources};

#[test]
fn pid_resources() {
    let h = cgroups::hierarchies::auto();
    let h = Box::new(&*h);
    let cg = Cgroup::new(h, String::from("pid_resources"));
    {
        let res = Resources {
            pid: PidResources {
                update_values: true,
                maximum_number_of_processes: MaxValue::Value(512),
            },
            ..Default::default()
        };
        cg.apply(&res);

        // verify
        let pidcontroller: &PidController = cg.controller_of().unwrap();
        let pid_max = pidcontroller.get_pid_max();
        assert_eq!(pid_max.is_ok(), true);
        assert_eq!(pid_max.unwrap(), MaxValue::Value(512));
    }
    cg.delete();
}
