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
