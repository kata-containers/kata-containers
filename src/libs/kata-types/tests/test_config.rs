// Copyright (c) 2019-2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//
#[cfg(test)]
mod tests {
    use kata_types::annotations::{
        Annotation, KATA_ANNO_CFG_AGENT_CONTAINER_PIPE_SIZE, KATA_ANNO_CFG_AGENT_TRACE,
        KATA_ANNO_CFG_DISABLE_GUEST_SECCOMP, KATA_ANNO_CFG_ENABLE_PPROF,
        KATA_ANNO_CFG_EXPERIMENTAL, KATA_ANNO_CFG_HYPERVISOR_BLOCK_DEV_CACHE_NOFLUSH,
        KATA_ANNO_CFG_HYPERVISOR_BLOCK_DEV_DRIVER, KATA_ANNO_CFG_HYPERVISOR_CTLPATH,
        KATA_ANNO_CFG_HYPERVISOR_DEFAULT_MEMORY, KATA_ANNO_CFG_HYPERVISOR_DEFAULT_VCPUS,
        KATA_ANNO_CFG_HYPERVISOR_ENABLE_GUEST_SWAP, KATA_ANNO_CFG_HYPERVISOR_ENABLE_HUGEPAGES,
        KATA_ANNO_CFG_HYPERVISOR_ENABLE_IO_THREADS
        KATA_ANNO_CFG_HYPERVISOR_FILE_BACKED_MEM_ROOT_DIR,
        KATA_ANNO_CFG_HYPERVISOR_GUEST_HOOK_PATH, KATA_ANNO_CFG_HYPERVISOR_JAILER_PATH,
        KATA_ANNO_CFG_HYPERVISOR_KERNEL_PATH, KATA_ANNO_CFG_HYPERVISOR_MEMORY_PREALLOC,
        KATA_ANNO_CFG_HYPERVISOR_MEMORY_SLOTS, KATA_ANNO_CFG_HYPERVISOR_PATH,
        KATA_ANNO_CFG_HYPERVISOR_VHOSTUSER_STORE_PATH, KATA_ANNO_CFG_HYPERVISOR_VIRTIO_FS_DAEMON,
        KATA_ANNO_CFG_HYPERVISOR_VIRTIO_FS_EXTRA_ARGS, KATA_ANNO_CFG_HYPERVISOR_VIRTIO_MEM,
        KATA_ANNO_CFG_KERNEL_MODULES, KATA_ANNO_CFG_RUNTIME_NAME,
    };
    use kata_types::config::KataConfig;
    use kata_types::config::{QemuConfig, TomlConfig};
    use std::collections::HashMap;
    use std::fs;
    use std::path::Path;
    #[test]
    fn test_change_config_annotation() {
        let content = include_str!("texture/configuration-anno-0.toml");
        let qemu = QemuConfig::new();
        qemu.register();

        let config = TomlConfig::load(content).unwrap();
        KataConfig::set_active_config(Some(config), "qemu", "agent0");

        std::process::Command::new("mkdir")
            .arg("./hypervisor_path")
            .output()
            .expect("failed to execute process");
        std::process::Command::new("mkdir")
            .arg("./store_path")
            .output()
            .expect("failed to execute process");
        std::process::Command::new("mkdir")
            .arg("./test_hypervisor_hook_path")
            .output()
            .expect("failed to execute process");
        std::process::Command::new("mkdir")
            .arg("./jvm")
            .output()
            .expect("failed to execute process");
        std::process::Command::new("mkdir")
            .arg("./test_file_backend_mem_root")
            .output()
            .expect("failed to execute process");
        std::process::Command::new("mkdir")
            .arg("./test_jailer_path")
            .output()
            .expect("failed to execute process");
        std::process::Command::new("mkdir")
            .arg("./test_kernel_path")
            .output()
            .expect("failed to execute process");
        std::process::Command::new("mkdir")
            .arg("./virtio_fs")
            .output()
            .expect("failed to execute process");
        let mut anno_hash = HashMap::new();
        anno_hash.insert(
            KATA_ANNO_CFG_KERNEL_MODULES.to_string(),
            "j465 aaa=1;r33w".to_string(),
        );
        anno_hash.insert(KATA_ANNO_CFG_AGENT_TRACE.to_string(), "false".to_string());
        anno_hash.insert(
            KATA_ANNO_CFG_AGENT_CONTAINER_PIPE_SIZE.to_string(),
            "3".to_string(),
        );
        anno_hash.insert(
            KATA_ANNO_CFG_HYPERVISOR_PATH.to_string(),
            "./hypervisor_path".to_string(),
        );
        anno_hash.insert(
            KATA_ANNO_CFG_HYPERVISOR_BLOCK_DEV_DRIVER.to_string(),
            "device".to_string(),
        );
        anno_hash.insert(
            KATA_ANNO_CFG_HYPERVISOR_BLOCK_DEV_CACHE_NOFLUSH.to_string(),
            "false".to_string(),
        );
        anno_hash.insert(
            KATA_ANNO_CFG_HYPERVISOR_VHOSTUSER_STORE_PATH.to_string(),
            "./store_path".to_string(),
        );
        anno_hash.insert(
            KATA_ANNO_CFG_DISABLE_GUEST_SECCOMP.to_string(),
            "true".to_string(),
        );
        anno_hash.insert(
            KATA_ANNO_CFG_HYPERVISOR_GUEST_HOOK_PATH.to_string(),
            "./test_hypervisor_hook_path".to_string(),
        );
        anno_hash.insert(
            KATA_ANNO_CFG_HYPERVISOR_MEMORY_PREALLOC.to_string(),
            "false".to_string(),
        );
        anno_hash.insert(
            KATA_ANNO_CFG_HYPERVISOR_CTLPATH.to_string(),
            "./jvm".to_string(),
        );
        anno_hash.insert(
            KATA_ANNO_CFG_HYPERVISOR_DEFAULT_VCPUS.to_string(),
            "12".to_string(),
        );
        anno_hash.insert(KATA_ANNO_CFG_ENABLE_PPROF.to_string(), "false".to_string());
        anno_hash.insert(
            KATA_ANNO_CFG_HYPERVISOR_ENABLE_GUEST_SWAP.to_string(),
            "false".to_string(),
        );
        anno_hash.insert(
            KATA_ANNO_CFG_HYPERVISOR_DEFAULT_MEMORY.to_string(),
            "100MiB".to_string(),
        );
        anno_hash.insert(
            KATA_ANNO_CFG_HYPERVISOR_ENABLE_IO_THREADS.to_string(),
            "false".to_string(),
        );
        anno_hash.insert(
            KATA_ANNO_CFG_HYPERVISOR_ENABLE_IO_THREADS.to_string(),
            "false".to_string(),
        );
        anno_hash.insert(
            KATA_ANNO_CFG_HYPERVISOR_FILE_BACKED_MEM_ROOT_DIR.to_string(),
            "./test_file_backend_mem_root".to_string(),
        );
        anno_hash.insert(
            KATA_ANNO_CFG_HYPERVISOR_ENABLE_HUGEPAGES.to_string(),
            "false".to_string(),
        );
        anno_hash.insert(
            KATA_ANNO_CFG_HYPERVISOR_JAILER_PATH.to_string(),
            "./test_jailer_path".to_string(),
        );
        anno_hash.insert(
            KATA_ANNO_CFG_HYPERVISOR_KERNEL_PATH.to_string(),
            "./test_kernel_path".to_string(),
        );
        anno_hash.insert(
            KATA_ANNO_CFG_HYPERVISOR_MEMORY_SLOTS.to_string(),
            "100".to_string(),
        );
        anno_hash.insert(
            KATA_ANNO_CFG_HYPERVISOR_VIRTIO_FS_EXTRA_ARGS.to_string(),
            "rr,dg,er".to_string(),
        );
        anno_hash.insert(
            KATA_ANNO_CFG_HYPERVISOR_VIRTIO_MEM.to_string(),
            "false".to_string(),
        );
        anno_hash.insert(
            KATA_ANNO_CFG_HYPERVISOR_VIRTIO_FS_DAEMON.to_string(),
            "./virtio_fs".to_string(),
        );
        anno_hash.insert(KATA_ANNO_CFG_EXPERIMENTAL.to_string(), "c,d,e".to_string());

        let anno = Annotation::new(anno_hash);
        let mut config = TomlConfig::load(content).unwrap();

        assert!(anno.update_config_by_annotation(&mut config).is_ok());
        KataConfig::set_active_config(Some(config), "qemu", "agnet0");
        if let Some(ag) = KataConfig::get_default_config().get_agent() {
            assert_eq!(
                ag.kernel_modules[0],
                "e1000e InterruptThrottleRate=3000,3000,3000 EEE=1"
            );

            assert_eq!(ag.kernel_modules[1], "i915_enabled_ppgtt=0");
            assert_eq!(ag.kernel_modules[2], "j465 aaa=1");
            assert_eq!(ag.kernel_modules[3], "r33w");
            assert!(!ag.enable_tracing);
            assert_eq!(ag.container_pipe_size, 3);
        }
        if let Some(hv) = KataConfig::get_default_config().get_hypervisor() {
            assert_eq!(hv.path, "./hypervisor_path".to_string());
            assert_eq!(hv.blockdev_info.block_device_driver, "device");
            assert!(!hv.blockdev_info.block_device_cache_noflush);
            assert!(hv.blockdev_info.block_device_cache_set);
            assert_eq!(hv.blockdev_info.vhost_user_store_path, "./store_path");
            assert_eq!(
                hv.security_info.guest_hook_path,
                "./test_hypervisor_hook_path"
            );
            assert!(!hv.memory_info.enable_mem_prealloc);
            assert_eq!(hv.ctlpath, "./jvm".to_string());
            assert_eq!(hv.cpu_info.default_vcpus, 12);
            assert!(!hv.memory_info.enable_guest_swap);
            assert_eq!(hv.memory_info.default_memory, 100);
            assert!(!hv.enable_iothreads);
            assert!(!hv.enable_iothreads);
            assert_eq!(
                hv.memory_info.file_mem_backend,
                "./test_file_backend_mem_root"
            );
            assert!(!hv.memory_info.enable_hugepages);
            assert_eq!(hv.jailer_path, "./test_jailer_path".to_string());
            assert_eq!(hv.boot_info.kernel, "./test_kernel_path");
            assert_eq!(hv.memory_info.memory_slots, 100);
            assert_eq!(hv.shared_fs.virtio_fs_extra_args[5], "rr");
            assert_eq!(hv.shared_fs.virtio_fs_extra_args[6], "dg");
            assert_eq!(hv.shared_fs.virtio_fs_extra_args[7], "er");
            assert!(!hv.memory_info.enable_virtio_mem);
            assert_eq!(hv.shared_fs.virtio_fs_daemon, "./virtio_fs");
        }

        assert!(
            KataConfig::get_active_config()
                .get_config()
                .runtime
                .disable_guest_seccomp
        );

        assert!(
            !KataConfig::get_active_config()
                .get_config()
                .runtime
                .enable_pprof
        );
        assert_eq!(
            KataConfig::get_active_config()
                .get_config()
                .runtime
                .experimental,
            ["a", "b", "c", "d", "e"]
        );
        std::process::Command::new("rmdir")
            .arg("./hypervisor_path")
            .output()
            .expect("failed to execute process");
        std::process::Command::new("rmdir")
            .arg("./test_hypervisor_hook_path")
            .output()
            .expect("failed to execute process");

        std::process::Command::new("rmdir")
            .arg("./test_file_backend_mem_root")
            .output()
            .expect("failed to execute process");

        std::process::Command::new("rmdir")
            .arg("./test_jailer_path")
            .output()
            .expect("failed to execute process");
        std::process::Command::new("rmdir")
            .arg("./test_kernel_path")
            .output()
            .expect("failed to execute process");
        std::process::Command::new("rmdir")
            .arg("./virtio_fs")
            .output()
            .expect("failed to execute process");
        std::process::Command::new("rmdir")
            .arg("./jvm")
            .output()
            .expect("failed to execute process");
        std::process::Command::new("rmdir")
            .arg("./store_path")
            .output()
            .expect("failed to execute process");
    }

    #[test]
    fn test_fail_to_change_block_device_driver_because_not_enabled() {
        let content = include_str!("texture/configuration-anno-1.toml");

        let qemu = QemuConfig::new();
        qemu.register();

        let config = TomlConfig::load(content).unwrap();
        KataConfig::set_active_config(Some(config), "qemu", "agent0");

        let mut anno_hash = HashMap::new();
        anno_hash.insert(
            KATA_ANNO_CFG_HYPERVISOR_BLOCK_DEV_DRIVER.to_string(),
            "fvfvfvfvf".to_string(),
        );
        let anno = Annotation::new(anno_hash);
        let mut config = TomlConfig::load(content).unwrap();

        assert!(anno.update_config_by_annotation(&mut config).is_ok());
        if let Some(hv) = KataConfig::get_default_config().get_hypervisor() {
            assert_eq!(hv.blockdev_info.block_device_driver, "virtio-blk");
        }
    }

    #[test]
    fn test_fail_to_change_enable_guest_swap_because_not_enabled() {
        let content = include_str!("texture/configuration-anno-1.toml");

        let qemu = QemuConfig::new();
        qemu.register();

        let config = TomlConfig::load(content).unwrap();
        KataConfig::set_active_config(Some(config), "qemu", "agent0");

        let mut anno_hash = HashMap::new();
        anno_hash.insert(
            KATA_ANNO_CFG_HYPERVISOR_ENABLE_GUEST_SWAP.to_string(),
            "false".to_string(),
        );
        let anno = Annotation::new(anno_hash);
        let mut config = TomlConfig::load(content).unwrap();

        assert!(anno.update_config_by_annotation(&mut config).is_ok());
        if let Some(hv) = KataConfig::get_default_config().get_hypervisor() {
            assert!(hv.memory_info.enable_guest_swap)
        }
    }

    #[test]
    fn test_fail_to_change_hypervisor_path_because_of_invalid_path() {
        let content = include_str!("texture/configuration-anno-0.toml");

        let qemu = QemuConfig::new();
        qemu.register();

        let config = TomlConfig::load(content).unwrap();
        KataConfig::set_active_config(Some(config), "qemu", "agent0");

        let mut anno_hash = HashMap::new();
        anno_hash.insert(
            KATA_ANNO_CFG_HYPERVISOR_PATH.to_string(),
            "/usr/bin/nle".to_string(),
        );
        let anno = Annotation::new(anno_hash);

        let path = env!("CARGO_MANIFEST_DIR");
        let path = Path::new(path).join("tests/texture/configuration-anno-0.toml");
        let content = fs::read_to_string(path).unwrap();
        let mut config = TomlConfig::load(&content).unwrap();
        assert!(anno.update_config_by_annotation(&mut config).is_err());
    }

    #[test]
    fn test_fail_to_change_kernel_path_because_of_invalid_path() {
        let path = env!("CARGO_MANIFEST_DIR");
        let path = Path::new(path).join("tests/texture/configuration-anno-0.toml");
        let content = fs::read_to_string(path).unwrap();

        let qemu = QemuConfig::new();
        qemu.register();

        let config = TomlConfig::load(&content).unwrap();
        KataConfig::set_active_config(Some(config), "qemu", "agent0");

        let mut anno_hash = HashMap::new();
        anno_hash.insert(
            KATA_ANNO_CFG_HYPERVISOR_KERNEL_PATH.to_string(),
            "/usr/bin/cdcd".to_string(),
        );
        let anno = Annotation::new(anno_hash);
        let mut config = TomlConfig::load(&content).unwrap();

        assert!(anno.update_config_by_annotation(&mut config).is_err());
    }

    #[test]
    fn test_fail_to_change_memory_slots_because_of_less_than_zero() {
        let content = include_str!("texture/configuration-anno-0.toml");
        let config = TomlConfig::load(content).unwrap();
        KataConfig::set_active_config(Some(config), "qemu", "agent0");

        let qemu = QemuConfig::new();
        qemu.register();

        let mut anno_hash = HashMap::new();
        anno_hash.insert(
            KATA_ANNO_CFG_HYPERVISOR_MEMORY_SLOTS.to_string(),
            "-1".to_string(),
        );
        let anno = Annotation::new(anno_hash);
        let mut config = TomlConfig::load(content).unwrap();

        assert!(anno.update_config_by_annotation(&mut config).is_err());
    }

    #[test]
    fn test_fail_to_change_default_memory_because_less_than_min_memory_size() {
        let content = include_str!("texture/configuration-anno-0.toml");

        let qemu = QemuConfig::new();
        qemu.register();

        let config = TomlConfig::load(content).unwrap();
        KataConfig::set_active_config(Some(config), "qemu", "agent0");

        let mut anno_hash = HashMap::new();
        anno_hash.insert(
            KATA_ANNO_CFG_HYPERVISOR_DEFAULT_MEMORY.to_string(),
            "10".to_string(),
        );
        let anno = Annotation::new(anno_hash);
        let mut config = TomlConfig::load(content).unwrap();

        assert!(anno.update_config_by_annotation(&mut config).is_err());
    }

    #[test]
    fn test_fail_to_change_default_vcpus_becuase_more_than_max_cpu_size() {
        let content = include_str!("texture/configuration-anno-0.toml");

        let qemu = QemuConfig::new();
        qemu.register();

        let config = TomlConfig::load(content).unwrap();
        KataConfig::set_active_config(Some(config), "qemu", "agent0");

        let mut anno_hash = HashMap::new();
        anno_hash.insert(
            KATA_ANNO_CFG_HYPERVISOR_DEFAULT_VCPUS.to_string(),
            "400".to_string(),
        );
        let anno = Annotation::new(anno_hash);
        let mut config = TomlConfig::load(content).unwrap();

        assert!(anno.update_config_by_annotation(&mut config).is_err());
    }

    #[test]
    fn test_fail_to_change_enable_guest_swap_because_invalid_input() {
        let content = include_str!("texture/configuration-anno-0.toml");

        let qemu = QemuConfig::new();
        qemu.register();

        let config = TomlConfig::load(content).unwrap();
        KataConfig::set_active_config(Some(config), "qemu", "agent0");

        let mut anno_hash = HashMap::new();
        anno_hash.insert(
            KATA_ANNO_CFG_HYPERVISOR_ENABLE_GUEST_SWAP.to_string(),
            "false1".to_string(),
        );
        let anno = Annotation::new(anno_hash);
        let mut config = TomlConfig::load(content).unwrap();

        assert!(anno.update_config_by_annotation(&mut config).is_err());
    }

    #[test]
    fn test_fail_to_change_default_vcpus_becuase_invalid_input() {
        let content = include_str!("texture/configuration-anno-0.toml");

        let qemu = QemuConfig::new();
        qemu.register();

        let config = TomlConfig::load(content).unwrap();
        KataConfig::set_active_config(Some(config), "qemu", "agent0");

        let mut anno_hash = HashMap::new();
        anno_hash.insert(
            KATA_ANNO_CFG_HYPERVISOR_DEFAULT_VCPUS.to_string(),
            "ddc".to_string(),
        );
        let anno = Annotation::new(anno_hash);
        let mut config = TomlConfig::load(content).unwrap();

        assert!(anno.update_config_by_annotation(&mut config).is_err());
    }

    #[test]
    fn test_fail_to_change_runtime_name() {
        let content = include_str!("texture/configuration-anno-0.toml");

        let qemu = QemuConfig::new();
        qemu.register();

        let config = TomlConfig::load(content).unwrap();
        KataConfig::set_active_config(Some(config), "qemu", "agent0");
        let mut anno_hash = HashMap::new();
        anno_hash.insert(
            KATA_ANNO_CFG_RUNTIME_NAME.to_string(),
            "other-container".to_string(),
        );
        let anno = Annotation::new(anno_hash);
        let mut config = TomlConfig::load(content).unwrap();
        assert!(anno.update_config_by_annotation(&mut config).is_err());
    }
}
