// Copyright (c) 2018 Levente Kurusa
// Copyright (c) 2020 And Group
//
// SPDX-License-Identifier: Apache-2.0 or MIT
//

//! Integration tests about the devices subsystem

use cgroups_rs::devices::{DevicePermissions, DeviceType, DevicesController};
use cgroups_rs::{Cgroup, DeviceResource};

#[test]
fn test_devices_parsing() {
    // now only v2
    if cgroups_rs::hierarchies::is_cgroup2_unified_mode() {
        return;
    }

    let h = cgroups_rs::hierarchies::auto();
    let cg = Cgroup::new(h, String::from("test_devices_parsing")).unwrap();
    {
        let devices: &DevicesController = cg.controller_of().unwrap();

        // Deny access to all devices first
        devices
            .deny_device(
                DeviceType::All,
                -1,
                -1,
                &[
                    DevicePermissions::Read,
                    DevicePermissions::Write,
                    DevicePermissions::MkNod,
                ],
            )
            .unwrap();
        // Acquire the list of allowed devices after we denied all
        let allowed_devices = devices.allowed_devices();
        // Verify that there are no devices that we can access.
        assert!(allowed_devices.is_ok());
        assert_eq!(allowed_devices.unwrap(), Vec::new());

        // Now add mknod access to /dev/null device
        devices
            .allow_device(DeviceType::Char, 1, 3, &[DevicePermissions::MkNod])
            .unwrap();
        let allowed_devices = devices.allowed_devices();
        assert!(allowed_devices.is_ok());
        let allowed_devices = allowed_devices.unwrap();
        assert_eq!(allowed_devices.len(), 1);
        assert_eq!(
            allowed_devices[0],
            DeviceResource {
                allow: true,
                devtype: DeviceType::Char,
                major: 1,
                minor: 3,
                access: vec![DevicePermissions::MkNod],
            }
        );

        // Now deny, this device explicitly.
        devices
            .deny_device(DeviceType::Char, 1, 3, &DevicePermissions::all())
            .unwrap();
        // Finally, check that.
        let allowed_devices = devices.allowed_devices();
        // Verify that there are no devices that we can access.
        assert!(allowed_devices.is_ok());
        assert_eq!(allowed_devices.unwrap(), Vec::new());
    }
    cg.delete().unwrap();
}
