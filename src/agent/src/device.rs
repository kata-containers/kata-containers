// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

// Convenience function to obtain the scope logger.
fn sl() -> slog::Logger {
    slog_scope::logger().new(o!("subsystem" => "device"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::uevent::spawn_test_watcher;
    use oci::{
        Linux, LinuxBuilder, LinuxDeviceBuilder, LinuxDeviceCgroupBuilder, LinuxDeviceType,
        LinuxResources, LinuxResourcesBuilder, SpecBuilder,
    };
    use oci_spec::runtime as oci;
    use std::iter::FromIterator;
    use tempfile::tempdir;

    const VM_ROOTFS: &str = "/";

    #[test]
    fn test_update_device_cgroup() {
        let mut linux = Linux::default();
        linux.set_resources(Some(LinuxResources::default()));
        let mut spec = SpecBuilder::default().linux(linux).build().unwrap();

        let dev_info = DeviceInfo::new(VM_ROOTFS, false).unwrap();
        insert_devices_cgroup_rule(&mut spec, &dev_info, false, "rw").unwrap();

        let devices = spec
            .linux()
            .as_ref()
            .unwrap()
            .resources()
            .as_ref()
            .unwrap()
            .devices()
            .clone()
            .unwrap();
        assert_eq!(devices.len(), 1);

        let meta = fs::metadata(VM_ROOTFS).unwrap();
        let rdev = meta.dev();
        let major = stat::major(rdev) as i64;
        let minor = stat::minor(rdev) as i64;

        assert_eq!(devices[0].major(), Some(major));
        assert_eq!(devices[0].minor(), Some(minor));
    }

    #[test]
    fn test_update_spec_devices() {
        let (major, minor) = (7, 2);
        let mut spec = Spec::default();

        // vm_path empty
        let update = DeviceInfo::new("", true);
        assert!(update.is_err());

        // linux is empty
        let container_path = "/dev/null";
        let vm_path = "/dev/null";
        let res = update_spec_devices(
            &mut spec,
            HashMap::from_iter(vec![(
                container_path,
                DeviceInfo::new(vm_path, true).unwrap().into(),
            )]),
        );
        assert!(res.is_err());

        spec.set_linux(Some(Linux::default()));

        // linux.devices doesn't contain the updated device
        let res = update_spec_devices(
            &mut spec,
            HashMap::from_iter(vec![(
                container_path,
                DeviceInfo::new(vm_path, true).unwrap().into(),
            )]),
        );
        assert!(res.is_err());

        spec.linux_mut()
            .as_mut()
            .unwrap()
            .set_devices(Some(vec![LinuxDeviceBuilder::default()
                .path(PathBuf::from("/dev/null2"))
                .major(major)
                .minor(minor)
                .build()
                .unwrap()]));

        // guest and host path are not the same
        let res = update_spec_devices(
            &mut spec,
            HashMap::from_iter(vec![(
                container_path,
                DeviceInfo::new(vm_path, true).unwrap().into(),
            )]),
        );
        assert!(
            res.is_err(),
            "container_path={:?} vm_path={:?} spec={:?}",
            container_path,
            vm_path,
            spec
        );

        spec.linux_mut()
            .as_mut()
            .unwrap()
            .devices_mut()
            .as_mut()
            .unwrap()[0]
            .set_path(PathBuf::from(container_path));

        // spec.linux.resources is empty
        let res = update_spec_devices(
            &mut spec,
            HashMap::from_iter(vec![(
                container_path,
                DeviceInfo::new(vm_path, true).unwrap().into(),
            )]),
        );
        assert!(res.is_ok());

        // update both devices and cgroup lists
        spec.linux_mut()
            .as_mut()
            .unwrap()
            .set_devices(Some(vec![LinuxDeviceBuilder::default()
                .path(PathBuf::from(container_path))
                .major(major)
                .minor(minor)
                .build()
                .unwrap()]));

        spec.linux_mut().as_mut().unwrap().set_resources(Some(
            oci::LinuxResourcesBuilder::default()
                .devices(vec![LinuxDeviceCgroupBuilder::default()
                    .major(major)
                    .minor(minor)
                    .build()
                    .unwrap()])
                .build()
                .unwrap(),
        ));

        let res = update_spec_devices(
            &mut spec,
            HashMap::from_iter(vec![(
                container_path,
                DeviceInfo::new(vm_path, true).unwrap().into(),
            )]),
        );
        assert!(res.is_ok());
    }

    #[test]
    fn test_update_spec_devices_guest_host_conflict() {
        let null_rdev = fs::metadata("/dev/null").unwrap().rdev();
        let zero_rdev = fs::metadata("/dev/zero").unwrap().rdev();
        let full_rdev = fs::metadata("/dev/full").unwrap().rdev();

        let host_major_a = stat::major(null_rdev) as i64;
        let host_minor_a = stat::minor(null_rdev) as i64;
        let host_major_b = stat::major(zero_rdev) as i64;
        let host_minor_b = stat::minor(zero_rdev) as i64;

        let mut spec = SpecBuilder::default()
            .linux(
                LinuxBuilder::default()
                    .devices(vec![
                        LinuxDeviceBuilder::default()
                            .path(PathBuf::from("/dev/a"))
                            .typ(LinuxDeviceType::C)
                            .major(host_major_a)
                            .minor(host_minor_a)
                            .build()
                            .unwrap(),
                        LinuxDeviceBuilder::default()
                            .path(PathBuf::from("/dev/b"))
                            .typ(LinuxDeviceType::C)
                            .major(host_major_b)
                            .minor(host_minor_b)
                            .build()
                            .unwrap(),
                    ])
                    .resources(
                        LinuxResourcesBuilder::default()
                            .devices(vec![
                                LinuxDeviceCgroupBuilder::default()
                                    .typ(LinuxDeviceType::C)
                                    .major(host_major_a)
                                    .minor(host_minor_a)
                                    .build()
                                    .unwrap(),
                                LinuxDeviceCgroupBuilder::default()
                                    .typ(LinuxDeviceType::C)
                                    .major(host_major_b)
                                    .minor(host_minor_b)
                                    .build()
                                    .unwrap(),
                            ])
                            .build()
                            .unwrap(),
                    )
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();

        let container_path_a = "/dev/a";
        let vm_path_a = "/dev/zero";

        let guest_major_a = stat::major(zero_rdev) as i64;
        let guest_minor_a = stat::minor(zero_rdev) as i64;

        let container_path_b = "/dev/b";
        let vm_path_b = "/dev/full";

        let guest_major_b = stat::major(full_rdev) as i64;
        let guest_minor_b = stat::minor(full_rdev) as i64;

        let specdevices = &spec.linux().as_ref().unwrap().devices().clone().unwrap();
        assert_eq!(host_major_a, specdevices[0].major());
        assert_eq!(host_minor_a, specdevices[0].minor());
        assert_eq!(host_major_b, specdevices[1].major());
        assert_eq!(host_minor_b, specdevices[1].minor());

        let specresources_devices = spec
            .linux()
            .as_ref()
            .unwrap()
            .resources()
            .as_ref()
            .unwrap()
            .devices()
            .clone()
            .unwrap();
        assert_eq!(Some(host_major_a), specresources_devices[0].major());
        assert_eq!(Some(host_minor_a), specresources_devices[0].minor());
        assert_eq!(Some(host_major_b), specresources_devices[1].major());
        assert_eq!(Some(host_minor_b), specresources_devices[1].minor());

        let updates = HashMap::from_iter(vec![
            (
                container_path_a,
                DeviceInfo::new(vm_path_a, true).unwrap().into(),
            ),
            (
                container_path_b,
                DeviceInfo::new(vm_path_b, true).unwrap().into(),
            ),
        ]);
        let res = update_spec_devices(&mut spec, updates);
        assert!(res.is_ok());

        let specdevices = &spec.linux().as_ref().unwrap().devices().clone().unwrap();
        assert_eq!(guest_major_a, specdevices[0].major());
        assert_eq!(guest_minor_a, specdevices[0].minor());
        assert_eq!(guest_major_b, specdevices[1].major());
        assert_eq!(guest_minor_b, specdevices[1].minor());

        let specresources_devices = spec
            .linux()
            .as_ref()
            .unwrap()
            .resources()
            .as_ref()
            .unwrap()
            .devices()
            .clone()
            .unwrap();
        assert_eq!(Some(guest_major_a), specresources_devices[0].major());
        assert_eq!(Some(guest_minor_a), specresources_devices[0].minor());
        assert_eq!(Some(guest_major_b), specresources_devices[1].major());
        assert_eq!(Some(guest_minor_b), specresources_devices[1].minor());
    }

    #[test]
    fn test_update_spec_devices_char_block_conflict() {
        let null_rdev = fs::metadata("/dev/null").unwrap().rdev();

        let guest_major = stat::major(null_rdev) as i64;
        let guest_minor = stat::minor(null_rdev) as i64;
        let host_major: i64 = 99;
        let host_minor: i64 = 99;

        let mut spec = SpecBuilder::default()
            .linux(
                LinuxBuilder::default()
                    .devices(vec![
                        LinuxDeviceBuilder::default()
                            .path(PathBuf::from("/dev/char"))
                            .typ(LinuxDeviceType::C)
                            .major(host_major)
                            .minor(host_minor)
                            .build()
                            .unwrap(),
                        LinuxDeviceBuilder::default()
                            .path(PathBuf::from("/dev/block"))
                            .typ(LinuxDeviceType::B)
                            .major(host_major)
                            .minor(host_minor)
                            .build()
                            .unwrap(),
                    ])
                    .resources(
                        LinuxResourcesBuilder::default()
                            .devices(vec![
                                LinuxDeviceCgroupBuilder::default()
                                    .typ(LinuxDeviceType::C)
                                    .major(host_major)
                                    .minor(host_minor)
                                    .build()
                                    .unwrap(),
                                LinuxDeviceCgroupBuilder::default()
                                    .typ(LinuxDeviceType::B)
                                    .major(host_major)
                                    .minor(host_minor)
                                    .build()
                                    .unwrap(),
                            ])
                            .build()
                            .unwrap(),
                    )
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();

        let container_path = "/dev/char";
        let vm_path = "/dev/null";

        let specresources_devices = spec
            .linux()
            .as_ref()
            .unwrap()
            .resources()
            .as_ref()
            .unwrap()
            .devices()
            .clone()
            .unwrap();
        assert_eq!(Some(host_major), specresources_devices[0].major());
        assert_eq!(Some(host_minor), specresources_devices[0].minor());
        assert_eq!(Some(host_major), specresources_devices[1].major());
        assert_eq!(Some(host_minor), specresources_devices[1].minor());

        let res = update_spec_devices(
            &mut spec,
            HashMap::from_iter(vec![(
                container_path,
                DeviceInfo::new(vm_path, true).unwrap().into(),
            )]),
        );
        assert!(res.is_ok());

        // Only the char device, not the block device should be updated
        let specresources_devices = spec
            .linux()
            .as_ref()
            .unwrap()
            .resources()
            .as_ref()
            .unwrap()
            .devices()
            .clone()
            .unwrap();
        assert_eq!(Some(guest_major), specresources_devices[0].major());
        assert_eq!(Some(guest_minor), specresources_devices[0].minor());
        assert_eq!(Some(host_major), specresources_devices[1].major());
        assert_eq!(Some(host_minor), specresources_devices[1].minor());
    }

    #[test]
    fn test_update_spec_devices_final_path() {
        let null_rdev = fs::metadata("/dev/null").unwrap().rdev();
        let guest_major = stat::major(null_rdev) as i64;
        let guest_minor = stat::minor(null_rdev) as i64;

        let container_path = "/dev/original";
        let host_major: i64 = 99;
        let host_minor: i64 = 99;

        let mut spec = SpecBuilder::default()
            .linux(
                LinuxBuilder::default()
                    .devices(vec![LinuxDeviceBuilder::default()
                        .path(PathBuf::from(container_path))
                        .typ(LinuxDeviceType::C)
                        .major(host_major)
                        .minor(host_minor)
                        .build()
                        .unwrap()])
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();

        let vm_path = "/dev/null";
        let final_path = "/dev/new";

        let res = update_spec_devices(
            &mut spec,
            HashMap::from_iter(vec![(
                container_path,
                DevUpdate::new(vm_path, final_path).unwrap(),
            )]),
        );
        assert!(res.is_ok());

        let specdevices = &spec.linux().as_ref().unwrap().devices().clone().unwrap();
        assert_eq!(guest_major, specdevices[0].major());
        assert_eq!(guest_minor, specdevices[0].minor());
        assert_eq!(&PathBuf::from(final_path), specdevices[0].path());
    }

    #[test]
    fn test_update_env_pci() {
        let example_map = [
            // Each is a host,guest pair of pci addresses
            ("0000:1a:01.0", "0000:01:01.0"),
            ("0000:1b:02.0", "0000:01:02.0"),
            // This one has the same host address as guest address
            // above, to test that we're not double-translating
            ("0000:01:01.0", "ffff:02:1f.7"),
        ];

        let pci_dev_info_original = r#"PCIDEVICE_x_INFO={"0000:1a:01.0":{"generic":{"deviceID":"0000:1a:01.0"}},"0000:1b:02.0":{"generic":{"deviceID":"0000:1b:02.0"}}}"#;
        let pci_dev_info_expected = r#"PCIDEVICE_x_INFO={"0000:01:01.0":{"generic":{"deviceID":"0000:01:01.0"}},"0000:01:02.0":{"generic":{"deviceID":"0000:01:02.0"}}}"#;
        let mut env = vec![
            "PCIDEVICE_x=0000:1a:01.0,0000:1b:02.0".to_string(),
            pci_dev_info_original.to_string(),
            "PCIDEVICE_y=0000:01:01.0".to_string(),
            "NOTAPCIDEVICE_blah=abcd:ef:01.0".to_string(),
        ];

        let pci_fixups = example_map
            .iter()
            .map(|(h, g)| {
                (
                    pci::Address::from_str(h).unwrap(),
                    pci::Address::from_str(g).unwrap(),
                )
            })
            .collect();

        let res = update_env_pci(&mut env, &pci_fixups);
        assert!(res.is_ok(), "error: {}", res.err().unwrap());

        assert_eq!(env[0], "PCIDEVICE_x=0000:01:01.0,0000:01:02.0");
        assert_eq!(env[1], pci_dev_info_expected);
        assert_eq!(env[2], "PCIDEVICE_y=ffff:02:1f.7");
        assert_eq!(env[3], "NOTAPCIDEVICE_blah=abcd:ef:01.0");
    }

    #[test]
    fn test_pcipath_to_sysfs() {
        let testdir = tempdir().expect("failed to create tmpdir");
        let rootbuspath = testdir.path().to_str().unwrap();

        let path2 = pci::Path::from_str("02").unwrap();
        let path23 = pci::Path::from_str("02/03").unwrap();
        let path234 = pci::Path::from_str("02/03/04").unwrap();

        let relpath = pcipath_to_sysfs(rootbuspath, &path2);
        assert_eq!(relpath.unwrap(), "/0000:00:02.0");

        let relpath = pcipath_to_sysfs(rootbuspath, &path23);
        assert!(relpath.is_err());

        let relpath = pcipath_to_sysfs(rootbuspath, &path234);
        assert!(relpath.is_err());

        // Create mock sysfs files for the device at 0000:00:02.0
        let bridge2path = format!("{}{}", rootbuspath, "/0000:00:02.0");

        fs::create_dir_all(&bridge2path).unwrap();

        let relpath = pcipath_to_sysfs(rootbuspath, &path2);
        assert_eq!(relpath.unwrap(), "/0000:00:02.0");

        let relpath = pcipath_to_sysfs(rootbuspath, &path23);
        assert!(relpath.is_err());

        let relpath = pcipath_to_sysfs(rootbuspath, &path234);
        assert!(relpath.is_err());

        // Create mock sysfs files to indicate that 0000:00:02.0 is a bridge to bus 01
        let bridge2bus = "0000:01";
        let bus2path = format!("{}/pci_bus/{}", bridge2path, bridge2bus);

        fs::create_dir_all(bus2path).unwrap();

        let relpath = pcipath_to_sysfs(rootbuspath, &path2);
        assert_eq!(relpath.unwrap(), "/0000:00:02.0");

        let relpath = pcipath_to_sysfs(rootbuspath, &path23);
        assert_eq!(relpath.unwrap(), "/0000:00:02.0/0000:01:03.0");

        let relpath = pcipath_to_sysfs(rootbuspath, &path234);
        assert!(relpath.is_err());

        // Create mock sysfs files for a bridge at 0000:01:03.0 to bus 02
        let bridge3path = format!("{}/0000:01:03.0", bridge2path);
        let bridge3bus = "0000:02";
        let bus3path = format!("{}/pci_bus/{}", bridge3path, bridge3bus);

        fs::create_dir_all(bus3path).unwrap();

        let relpath = pcipath_to_sysfs(rootbuspath, &path2);
        assert_eq!(relpath.unwrap(), "/0000:00:02.0");

        let relpath = pcipath_to_sysfs(rootbuspath, &path23);
        assert_eq!(relpath.unwrap(), "/0000:00:02.0/0000:01:03.0");

        let relpath = pcipath_to_sysfs(rootbuspath, &path234);
        assert_eq!(relpath.unwrap(), "/0000:00:02.0/0000:01:03.0/0000:02:04.0");
    }

    // We use device specific variants of this for real cases, but
    // they have some complications that make them troublesome to unit
    // test
    async fn example_get_device_name(
        sandbox: &Arc<Mutex<Sandbox>>,
        relpath: &str,
    ) -> Result<String> {
        let matcher = VirtioBlkPciMatcher::new(relpath);

        let uev = wait_for_uevent(sandbox, matcher).await?;

        Ok(uev.devname)
    }

    #[tokio::test]
    async fn test_get_device_name() {
        let devname = "vda";
        let root_bus = create_pci_root_bus_path();
        let relpath = "/0000:00:0a.0/0000:03:0b.0";
        let devpath = format!("{}{}/virtio4/block/{}", root_bus, relpath, devname);

        let mut uev = crate::uevent::Uevent::default();
        uev.action = crate::linux_abi::U_EVENT_ACTION_ADD.to_string();
        uev.subsystem = BLOCK.to_string();
        uev.devpath = devpath.clone();
        uev.devname = devname.to_string();

        let logger = slog::Logger::root(slog::Discard, o!());
        let sandbox = Arc::new(Mutex::new(Sandbox::new(&logger).unwrap()));

        let mut sb = sandbox.lock().await;
        sb.uevent_map.insert(devpath.clone(), uev);
        drop(sb); // unlock

        let name = example_get_device_name(&sandbox, relpath).await;
        assert!(name.is_ok(), "{}", name.unwrap_err());
        assert_eq!(name.unwrap(), devname);

        let mut sb = sandbox.lock().await;
        let uev = sb.uevent_map.remove(&devpath).unwrap();
        drop(sb); // unlock

        spawn_test_watcher(sandbox.clone(), uev);

        let name = example_get_device_name(&sandbox, relpath).await;
        assert!(name.is_ok(), "{}", name.unwrap_err());
        assert_eq!(name.unwrap(), devname);
    }

    #[tokio::test]
    #[allow(clippy::redundant_clone)]
    async fn test_virtio_blk_matcher() {
        let root_bus = create_pci_root_bus_path();
        let devname = "vda";

        let mut uev_a = crate::uevent::Uevent::default();
        let relpath_a = "/0000:00:0a.0";
        uev_a.action = crate::linux_abi::U_EVENT_ACTION_ADD.to_string();
        uev_a.subsystem = BLOCK.to_string();
        uev_a.devname = devname.to_string();
        uev_a.devpath = format!("{}{}/virtio4/block/{}", root_bus, relpath_a, devname);
        let matcher_a = VirtioBlkPciMatcher::new(relpath_a);

        let mut uev_b = uev_a.clone();
        let relpath_b = "/0000:00:0a.0/0000:00:0b.0";
        uev_b.devpath = format!("{}{}/virtio0/block/{}", root_bus, relpath_b, devname);
        let matcher_b = VirtioBlkPciMatcher::new(relpath_b);

        assert!(matcher_a.is_match(&uev_a));
        assert!(matcher_b.is_match(&uev_b));
        assert!(!matcher_b.is_match(&uev_a));
        assert!(!matcher_a.is_match(&uev_b));
    }

    #[cfg(target_arch = "s390x")]
    #[tokio::test]
    async fn test_virtio_blk_ccw_matcher() {
        let root_bus = CCW_ROOT_BUS_PATH;
        let subsystem = "block";
        let devname = "vda";
        let relpath = "0.0.0002";

        let mut uev = crate::uevent::Uevent::default();
        uev.action = crate::linux_abi::U_EVENT_ACTION_ADD.to_string();
        uev.subsystem = subsystem.to_string();
        uev.devname = devname.to_string();
        uev.devpath = format!(
            "{}/0.0.0001/{}/virtio1/{}/{}",
            root_bus, relpath, subsystem, devname
        );

        // Valid path
        let device = ccw::Device::from_str(relpath).unwrap();
        let matcher = VirtioBlkCCWMatcher::new(root_bus, &device);
        assert!(matcher.is_match(&uev));

        // Invalid paths
        uev.devpath = format!(
            "{}/0.0.0001/0.0.0003/virtio1/{}/{}",
            root_bus, subsystem, devname
        );
        assert!(!matcher.is_match(&uev));

        uev.devpath = format!("0.0.0001/{}/virtio1/{}/{}", relpath, subsystem, devname);
        assert!(!matcher.is_match(&uev));

        uev.devpath = format!(
            "{}/0.0.0001/{}/virtio/{}/{}",
            root_bus, relpath, subsystem, devname
        );
        assert!(!matcher.is_match(&uev));

        uev.devpath = format!("{}/0.0.0001/{}/virtio1", root_bus, relpath);
        assert!(!matcher.is_match(&uev));

        uev.devpath = format!(
            "{}/1.0.0001/{}/virtio1/{}/{}",
            root_bus, relpath, subsystem, devname
        );
        assert!(!matcher.is_match(&uev));

        uev.devpath = format!(
            "{}/0.4.0001/{}/virtio1/{}/{}",
            root_bus, relpath, subsystem, devname
        );
        assert!(!matcher.is_match(&uev));

        uev.devpath = format!(
            "{}/0.0.10000/{}/virtio1/{}/{}",
            root_bus, relpath, subsystem, devname
        );
        assert!(!matcher.is_match(&uev));
    }

    #[tokio::test]
    #[allow(clippy::redundant_clone)]
    async fn test_scsi_block_matcher() {
        let root_bus = create_pci_root_bus_path();
        let devname = "sda";

        let mut uev_a = crate::uevent::Uevent::default();
        let addr_a = "0:0";
        uev_a.action = crate::linux_abi::U_EVENT_ACTION_ADD.to_string();
        uev_a.subsystem = BLOCK.to_string();
        uev_a.devname = devname.to_string();
        uev_a.devpath = format!(
            "{}/0000:00:00.0/virtio0/host0/target0:0:0/0:0:{}/block/sda",
            root_bus, addr_a
        );
        let matcher_a = ScsiBlockMatcher::new(addr_a);

        let mut uev_b = uev_a.clone();
        let addr_b = "2:0";
        uev_b.devpath = format!(
            "{}/0000:00:00.0/virtio0/host0/target0:0:2/0:0:{}/block/sdb",
            root_bus, addr_b
        );
        let matcher_b = ScsiBlockMatcher::new(addr_b);

        assert!(matcher_a.is_match(&uev_a));
        assert!(matcher_b.is_match(&uev_b));
        assert!(!matcher_b.is_match(&uev_a));
        assert!(!matcher_a.is_match(&uev_b));
    }

    #[tokio::test]
    #[allow(clippy::redundant_clone)]
    async fn test_vfio_matcher() {
        let grpa = IommuGroup(1);
        let grpb = IommuGroup(22);

        let mut uev_a = crate::uevent::Uevent::default();
        uev_a.action = crate::linux_abi::U_EVENT_ACTION_ADD.to_string();
        uev_a.devname = format!("vfio/{}", grpa);
        uev_a.devpath = format!("/devices/virtual/vfio/{}", grpa);
        let matcher_a = VfioMatcher::new(grpa);

        let mut uev_b = uev_a.clone();
        uev_b.devpath = format!("/devices/virtual/vfio/{}", grpb);
        let matcher_b = VfioMatcher::new(grpb);

        assert!(matcher_a.is_match(&uev_a));
        assert!(matcher_b.is_match(&uev_b));
        assert!(!matcher_b.is_match(&uev_a));
        assert!(!matcher_a.is_match(&uev_b));
    }

    #[tokio::test]
    #[allow(clippy::redundant_clone)]
    async fn test_net_pci_matcher() {
        let root_bus = create_pci_root_bus_path();
        let relpath_a = "/0000:00:02.0/0000:01:01.0";

        let mut uev_a = crate::uevent::Uevent::default();
        uev_a.action = crate::linux_abi::U_EVENT_ACTION_ADD.to_string();
        uev_a.devpath = format!("{}{}", root_bus, relpath_a);
        uev_a.subsystem = String::from("net");
        uev_a.interface = String::from("eth0");
        let matcher_a = NetPciMatcher::new(relpath_a);
        println!("Matcher a : {}", matcher_a.devpath);

        let relpath_b = "/0000:00:02.0/0000:01:02.0";
        let mut uev_b = uev_a.clone();
        uev_b.devpath = format!("{}{}", root_bus, relpath_b);
        let matcher_b = NetPciMatcher::new(relpath_b);

        assert!(matcher_a.is_match(&uev_a));
        assert!(matcher_b.is_match(&uev_b));
        assert!(!matcher_b.is_match(&uev_a));
        assert!(!matcher_a.is_match(&uev_b));

        let relpath_c = "/0000:00:02.0/0000:01:03.0";
        let net_substr = "/net/eth0";
        let mut uev_c = uev_a.clone();
        uev_c.devpath = format!("{}{}{}", root_bus, relpath_c, net_substr);
        let matcher_c = NetPciMatcher::new(relpath_c);

        assert!(matcher_c.is_match(&uev_c));
        assert!(!matcher_a.is_match(&uev_c));
        assert!(!matcher_b.is_match(&uev_c));
    }

    #[tokio::test]
    #[allow(clippy::redundant_clone)]
    async fn test_mmio_block_matcher() {
        let devname_a = "vda";
        let devname_b = "vdb";
        let mut uev_a = crate::uevent::Uevent::default();
        uev_a.action = crate::linux_abi::U_EVENT_ACTION_ADD.to_string();
        uev_a.subsystem = BLOCK.to_string();
        uev_a.devname = devname_a.to_string();
        uev_a.devpath = format!(
            "/sys/devices/virtio-mmio-cmdline/virtio-mmio.0/virtio0/block/{}",
            devname_a
        );
        let matcher_a = MmioBlockMatcher::new(devname_a);

        let mut uev_b = uev_a.clone();
        uev_b.devpath = format!(
            "/sys/devices/virtio-mmio-cmdline/virtio-mmio.4/virtio4/block/{}",
            devname_b
        );
        let matcher_b = MmioBlockMatcher::new(devname_b);

        assert!(matcher_a.is_match(&uev_a));
        assert!(matcher_b.is_match(&uev_b));
        assert!(!matcher_b.is_match(&uev_a));
        assert!(!matcher_a.is_match(&uev_b));
    }

    #[test]
    fn test_split_vfio_pci_option() {
        assert_eq!(
            split_vfio_pci_option("0000:01:00.0=02/01"),
            Some(("0000:01:00.0", "02/01"))
        );
        assert_eq!(split_vfio_pci_option("0000:01:00.0=02/01=rubbish"), None);
        assert_eq!(split_vfio_pci_option("0000:01:00.0"), None);
    }

    #[test]
    fn test_pci_driver_override() {
        let testdir = tempdir().expect("failed to create tmpdir");
        let syspci = testdir.path(); // Path to mock /sys/bus/pci

        let dev0 = pci::Address::new(0, 0, pci::SlotFn::new(0, 0).unwrap());
        let dev0path = syspci.join("devices").join(dev0.to_string());
        let dev0drv = dev0path.join("driver");
        let dev0override = dev0path.join("driver_override");

        let drvapath = syspci.join("drivers").join("drv_a");
        let drvaunbind = drvapath.join("unbind");

        let probepath = syspci.join("drivers_probe");

        // Start mocking dev0 as being unbound
        fs::create_dir_all(&dev0path).unwrap();

        pci_driver_override(syspci, dev0, "drv_a").unwrap();
        assert_eq!(fs::read_to_string(&dev0override).unwrap(), "drv_a");
        assert_eq!(fs::read_to_string(&probepath).unwrap(), dev0.to_string());

        // Now mock dev0 already being attached to drv_a
        fs::create_dir_all(&drvapath).unwrap();
        std::os::unix::fs::symlink(&drvapath, dev0drv).unwrap();
        std::fs::remove_file(&probepath).unwrap();

        pci_driver_override(syspci, dev0, "drv_a").unwrap(); // no-op
        assert_eq!(fs::read_to_string(&dev0override).unwrap(), "drv_a");
        assert!(!probepath.exists());

        // Now try binding to a different driver
        pci_driver_override(syspci, dev0, "drv_b").unwrap();
        assert_eq!(fs::read_to_string(&dev0override).unwrap(), "drv_b");
        assert_eq!(fs::read_to_string(&probepath).unwrap(), dev0.to_string());
        assert_eq!(fs::read_to_string(drvaunbind).unwrap(), dev0.to_string());
    }

    #[test]
    fn test_pci_iommu_group() {
        let testdir = tempdir().expect("failed to create tmpdir"); // mock /sys
        let syspci = testdir.path().join("bus").join("pci");

        // Mock dev0, which has no group
        let dev0 = pci::Address::new(0, 0, pci::SlotFn::new(0, 0).unwrap());
        let dev0path = syspci.join("devices").join(dev0.to_string());

        fs::create_dir_all(dev0path).unwrap();

        // Test dev0
        assert!(pci_iommu_group(&syspci, dev0).unwrap().is_none());

        // Mock dev1, which is in group 12
        let dev1 = pci::Address::new(0, 1, pci::SlotFn::new(0, 0).unwrap());
        let dev1path = syspci.join("devices").join(dev1.to_string());
        let dev1group = dev1path.join("iommu_group");

        fs::create_dir_all(&dev1path).unwrap();
        std::os::unix::fs::symlink("../../../kernel/iommu_groups/12", dev1group).unwrap();

        // Test dev1
        assert_eq!(
            pci_iommu_group(&syspci, dev1).unwrap(),
            Some(IommuGroup(12))
        );

        // Mock dev2, which has a bogus group (dir instead of symlink)
        let dev2 = pci::Address::new(0, 2, pci::SlotFn::new(0, 0).unwrap());
        let dev2path = syspci.join("devices").join(dev2.to_string());
        let dev2group = dev2path.join("iommu_group");

        fs::create_dir_all(dev2group).unwrap();

        // Test dev2
        assert!(pci_iommu_group(&syspci, dev2).is_err());
    }

    #[cfg(target_arch = "s390x")]
    #[tokio::test]
    async fn test_vfio_ap_matcher() {
        let subsystem = "ap";
        let card = "0a";
        let relpath = format!("{}.0001", card);

        let mut uev = Uevent::default();
        uev.action = U_EVENT_ACTION_ADD.to_string();
        uev.subsystem = subsystem.to_string();
        uev.devpath = format!("{}/card{}/{}", AP_ROOT_BUS_PATH, card, relpath);

        let ap_address = ap::Address::from_str(&relpath).unwrap();
        let matcher = ApMatcher::new(ap_address);

        assert!(matcher.is_match(&uev));

        let mut uev_remove = uev.clone();
        uev_remove.action = U_EVENT_ACTION_REMOVE.to_string();
        assert!(!matcher.is_match(&uev_remove));

        let mut uev_other_device = uev.clone();
        uev_other_device.devpath = format!("{}/card{}/{}.0002", AP_ROOT_BUS_PATH, card, card);
        assert!(!matcher.is_match(&uev_other_device));
    }
}
