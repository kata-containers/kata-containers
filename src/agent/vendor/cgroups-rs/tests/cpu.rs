// Copyright (c) 2020 And Group
//
// SPDX-License-Identifier: Apache-2.0 or MIT
//

//! Simple unit tests about the CPU control groups system.
use cgroups_rs::cpu::CpuController;
use cgroups_rs::Cgroup;

#[test]
fn test_cfs_quota_and_periods() {
    let h = cgroups_rs::hierarchies::auto();
    let cg = Cgroup::new(h, String::from("test_cfs_quota_and_periods"));

    let cpu_controller: &CpuController = cg.controller_of().unwrap();

    let current_quota = cpu_controller.cfs_quota().unwrap();
    let current_peroid = cpu_controller.cfs_period().unwrap();

    // verify default value
    // The default is “max 100000”.
    assert_eq!(-1, current_quota);
    assert_eq!(100000, current_peroid);

    // case 1 set quota
    let _ = cpu_controller.set_cfs_quota(2000);

    let current_quota = cpu_controller.cfs_quota().unwrap();
    let current_peroid = cpu_controller.cfs_period().unwrap();
    assert_eq!(2000, current_quota);
    assert_eq!(100000, current_peroid);

    // case 2 set period
    cpu_controller.set_cfs_period(1000000).unwrap();
    let current_quota = cpu_controller.cfs_quota().unwrap();
    let current_peroid = cpu_controller.cfs_period().unwrap();
    assert_eq!(2000, current_quota);
    assert_eq!(1000000, current_peroid);

    // case 3 set both quota and period
    cpu_controller
        .set_cfs_quota_and_period(Some(5000), Some(100000))
        .unwrap();

    let current_quota = cpu_controller.cfs_quota().unwrap();
    let current_peroid = cpu_controller.cfs_period().unwrap();
    assert_eq!(5000, current_quota);
    assert_eq!(100000, current_peroid);

    // case 4 set both quota and period, set quota to -1
    cpu_controller
        .set_cfs_quota_and_period(Some(-1), None)
        .unwrap();

    let current_quota = cpu_controller.cfs_quota().unwrap();
    let current_peroid = cpu_controller.cfs_period().unwrap();
    assert_eq!(-1, current_quota);
    assert_eq!(100000, current_peroid);

    cg.delete().unwrap();
}
