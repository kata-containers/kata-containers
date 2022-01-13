#[cfg(test)]
mod tests {
    use kata_types::annotations::{
        Annotation, KATA_ANNO_CONF_AGENT_CONTAINER_PIPE_SIZE, KATA_ANNO_CONF_AGENT_TRACE,
        KATA_ANNO_CONF_HYPERVISOR_CTLPATH, KATA_ANNO_CONF_HYPERVISOR_DEFAULT_MEMORY,
        KATA_ANNO_CONF_HYPERVISOR_ENABLE_IO_THREADS,
        KATA_ANNO_CONF_HYPERVISOR_FILE_BACKED_MEM_ROOT_DIR, KATA_ANNO_CONF_HYPERVISOR_HUGE_PAGES,
        KATA_ANNO_CONF_HYPERVISOR_JAILER_PATH, KATA_ANNO_CONF_HYPERVISOR_MEMORY_PREALLOC,
        KATA_ANNO_CONF_HYPERVISOR_MEMORY_SLOTS, KATA_ANNO_CONF_HYPERVISOR_PATH,
        KATA_ANNO_CONF_HYPERVISOR_VIRTIO_MEM, KATA_ANNO_CONF_KERNEL_MODULES,
    };
    use kata_types::config::KataConfig;
    use kata_types::config::{QemuConfig, TomlConfig};
    use std::collections::HashMap;
    use std::fs;
    use std::path::Path;
    /*
    #[test]
    fn test_load_qemu_config() {
        let plugin = QemuConfig::new();
        plugin.register();

        let path = env!("CARGO_MANIFEST_DIR");
        let path = Path::new(path).join("tests/texture/configuration-qemu.toml");
        let content = fs::read_to_string(&path).unwrap();
        let config = TomlConfig::load(&content).unwrap();
        let qemu = config.hypervisor.get(HYPERVISOR_NAME_QEMU).unwrap();
        assert_eq!(qemu.path, "/usr/bin/ls");
        assert_eq!(qemu.valid_hypervisor_paths.len(), 2);
        assert_eq!(qemu.valid_hypervisor_paths[0], "/usr/bin/qemu*");
        assert_eq!(qemu.valid_hypervisor_paths[1], "/opt/qemu?");
        qemu.validate_hypervisor_path("/usr/bin/qemu0").unwrap();
        qemu.validate_hypervisor_path("/usr/bin/qemu1").unwrap();
        qemu.validate_hypervisor_path("/usr/bin/qemu2222").unwrap();
        qemu.validate_hypervisor_path("/opt/qemu3").unwrap();
        qemu.validate_hypervisor_path("/opt/qemu").unwrap_err();
        qemu.validate_hypervisor_path("/opt/qemu33").unwrap_err();
        assert_eq!(qemu.ctlpath, "/usr/bin/qemu_ctl");
        assert_eq!(qemu.valid_ctlpaths.len(), 0);
        assert!(qemu.jailer_path.is_empty());
        assert_eq!(qemu.valid_jailer_paths.len(), 0);
        assert_eq!(qemu.disable_nesting_checks, true);
        assert_eq!(qemu.enable_iothreads, true);

        let qemu = config.hypervisor.get(HYPERVISOR_NAME_QEMU).unwrap();
        assert_eq!(qemu.path, "/usr/bin/ls");
        assert_eq!(qemu.valid_hypervisor_paths.len(), 2);
        assert_eq!(qemu.valid_hypervisor_paths[0], "/usr/bin/qemu*");
        assert_eq!(qemu.valid_hypervisor_paths[1], "/opt/qemu?");
        qemu.validate_hypervisor_path("/usr/bin/qemu0").unwrap();
        qemu.validate_hypervisor_path("/usr/bin/qemu1").unwrap();
        qemu.validate_hypervisor_path("/usr/bin/qemu2222").unwrap();
        qemu.validate_hypervisor_path("/opt/qemu3").unwrap();
        qemu.validate_hypervisor_path("/opt/qemu").unwrap_err();
        qemu.validate_hypervisor_path("/opt/qemu33").unwrap_err();
        assert_eq!(qemu.ctlpath, "/usr/bin/ls");
        assert_eq!(qemu.valid_ctlpaths.len(), 0);
        assert!(qemu.jailer_path.is_empty());
        assert_eq!(qemu.valid_jailer_paths.len(), 0);
        assert_eq!(qemu.disable_nesting_checks, true);
        assert_eq!(qemu.enable_iothreads, true);

        assert_eq!(qemu.boot_info.image, "/usr/bin/echo");
        assert_eq!(qemu.boot_info.kernel, "/usr/bin/id");
        assert_eq!(qemu.boot_info.kernel_params, "ro");
        assert_eq!(qemu.boot_info.firmware, "/etc/hostname");

    }
    */
    #[test]
    fn test_change_kernel_config() {
        let path = env!("CARGO_MANIFEST_DIR");
        let path = Path::new(path).join("tests/texture/configuration-anno.toml");
        let content = fs::read_to_string(&path).unwrap();

        let qemu = QemuConfig::new();
        qemu.register();

        let mut config = TomlConfig::load(&content).unwrap();
        let mut anno_hash = HashMap::new();
        anno_hash.insert(
            KATA_ANNO_CONF_KERNEL_MODULES.to_string(),
            "j465 aaa=1;r33w".to_string(),
        );
        let anno = Annotation::new(anno_hash);
        anno.add_agent_annotation(&mut config, &"agent0".to_string());
        let mods = &config.agent.get("agent0").unwrap().kernel_modules;
        assert_eq!(mods[0], "e1000e InterruptThrottleRate=3000,3000,3000 EEE=1");
        assert_eq!(mods[1], "i915_enabled_ppgtt=0");
        assert_eq!(mods[2], "j465 aaa=1");
        assert_eq!(mods[3], "r33w");
    }

    #[test]
    fn test_change_trace_config() {
        let path = env!("CARGO_MANIFEST_DIR");
        let path = Path::new(path).join("tests/texture/configuration-anno.toml");
        let content = fs::read_to_string(&path).unwrap();

        let qemu = QemuConfig::new();
        qemu.register();

        let mut config = TomlConfig::load(&content).unwrap();
        let mut anno_hash = HashMap::new();
        anno_hash.insert(KATA_ANNO_CONF_AGENT_TRACE.to_string(), "false".to_string());
        let anno = Annotation::new(anno_hash);
        anno.add_agent_enable_trace(&mut config, &"agent0".to_string());
        let enable_trace = &config.agent.get("agent0").unwrap().enable_tracing;
        assert!(!enable_trace);
    }

    #[test]
    fn test_change_pipe_config() {
        let path = env!("CARGO_MANIFEST_DIR");
        let path = Path::new(path).join("tests/texture/configuration-anno.toml");
        let content = fs::read_to_string(&path).unwrap();

        let qemu = QemuConfig::new();
        qemu.register();

        let mut config = TomlConfig::load(&content).unwrap();
        let mut anno_hash = HashMap::new();
        anno_hash.insert(
            KATA_ANNO_CONF_AGENT_CONTAINER_PIPE_SIZE.to_string(),
            "3".to_string(),
        );
        let anno = Annotation::new(anno_hash);
        anno.add_agent_container_pipe_size(&mut config, &"agent0".to_string());
        let pipe_size = &config.agent.get("agent0").unwrap().container_pipe_size;
        assert_eq!(pipe_size, &3);
    }

    #[test]
    fn test_change_hypervisor_path() {
        let path = env!("CARGO_MANIFEST_DIR");
        let path = Path::new(path).join("tests/texture/configuration-anno.toml");
        let content = fs::read_to_string(&path).unwrap();

        let qemu = QemuConfig::new();
        qemu.register();

        let config = TomlConfig::load(&content).unwrap();
        KataConfig::set_active_config(config, &"qemu", &"agent0");

        let mut anno_hash = HashMap::new();
        anno_hash.insert(
            KATA_ANNO_CONF_HYPERVISOR_PATH.to_string(),
            "/usr/bin/lsns".to_string(),
        );
        let anno = Annotation::new(anno_hash);

        let path = env!("CARGO_MANIFEST_DIR");
        let path = Path::new(path).join("tests/texture/configuration-anno.toml");
        let content = fs::read_to_string(&path).unwrap();
        let mut config = TomlConfig::load(&content).unwrap();
        assert!(anno
            .add_hypervisor_path(&mut config, &"qemu".to_string())
            .is_ok());
        KataConfig::set_active_config(config, &"qemu", &"agent");
        if let Some(hv) = KataConfig::get_default_config().get_hypervisor() {
            assert_eq!(hv.path, "/usr/bin/lsns".to_string());
        }
    }

    #[test]
    fn test_fail_to_change_hypervisor_path() {
        let path = env!("CARGO_MANIFEST_DIR");
        let path = Path::new(path).join("tests/texture/configuration-anno.toml");
        let content = fs::read_to_string(&path).unwrap();

        let qemu = QemuConfig::new();
        qemu.register();

        let config = TomlConfig::load(&content).unwrap();
        KataConfig::set_active_config(config, &"qemu", &"agent0");

        let mut anno_hash = HashMap::new();
        anno_hash.insert(
            KATA_ANNO_CONF_HYPERVISOR_PATH.to_string(),
            "/usr/bin/nl".to_string(),
        );
        let anno = Annotation::new(anno_hash);

        let path = env!("CARGO_MANIFEST_DIR");
        let path = Path::new(path).join("tests/texture/configuration-anno.toml");
        let content = fs::read_to_string(&path).unwrap();
        let mut config = TomlConfig::load(&content).unwrap();
        assert!(anno
            .add_hypervisor_path(&mut config, &"qemu".to_string())
            .is_err());
    }

    #[test]
    fn test_change_hypervisor_jailer_path() {
        let path = env!("CARGO_MANIFEST_DIR");
        let path = Path::new(path).join("tests/texture/configuration-anno.toml");
        let content = fs::read_to_string(&path).unwrap();

        let qemu = QemuConfig::new();
        qemu.register();

        let config = TomlConfig::load(&content).unwrap();
        KataConfig::set_active_config(config, &"qemu", &"agent0");

        let mut anno_hash = HashMap::new();
        anno_hash.insert(
            KATA_ANNO_CONF_HYPERVISOR_JAILER_PATH.to_string(),
            "/usr/lib/rust".to_string(),
        );
        let anno = Annotation::new(anno_hash);

        let content = fs::read_to_string(&path).unwrap();
        let mut config = TomlConfig::load(&content).unwrap();

        assert!(anno
            .add_hypervisor_jailer_path(&mut config, &"qemu".to_string())
            .is_ok());
        KataConfig::set_active_config(config, &"qemu", &"agent0");
        if let Some(hv) = KataConfig::get_default_config().get_hypervisor() {
            assert_eq!(hv.jailer_path, "/usr/lib/rust".to_string());
        }
    }

    #[test]
    fn test_fail_to_change_hypervisor_jailer_path() {
        let path = env!("CARGO_MANIFEST_DIR");
        let path = Path::new(path).join("tests/texture/configuration-anno.toml");
        let content = fs::read_to_string(&path).unwrap();

        let qemu = QemuConfig::new();
        qemu.register();
        let config = TomlConfig::load(&content).unwrap();
        KataConfig::set_active_config(config, &"qemu", &"agent0");

        let mut anno_hash = HashMap::new();
        anno_hash.insert(
            KATA_ANNO_CONF_HYPERVISOR_JAILER_PATH.to_string(),
            "/usr/lib/jvm".to_string(),
        );
        let anno = Annotation::new(anno_hash);

        let content = fs::read_to_string(&path).unwrap();
        let mut config = TomlConfig::load(&content).unwrap();

        assert!(anno
            .add_hypervisor_jailer_path(&mut config, &"qemu".to_string())
            .is_err());
    }

    #[test]
    fn test_change_hypervisor_ctl_path() {
        let path = env!("CARGO_MANIFEST_DIR");
        let path = Path::new(path).join("tests/texture/configuration-anno.toml");
        let content = fs::read_to_string(&path).unwrap();

        let qemu = QemuConfig::new();
        qemu.register();

        let config = TomlConfig::load(&content).unwrap();
        KataConfig::set_active_config(config, &"qemu", &"agent0");

        let mut anno_hash = HashMap::new();
        anno_hash.insert(
            KATA_ANNO_CONF_HYPERVISOR_CTLPATH.to_string(),
            "/usr/lib/jvm".to_string(),
        );
        let anno = Annotation::new(anno_hash);

        let content = fs::read_to_string(&path).unwrap();
        let mut config = TomlConfig::load(&content).unwrap();

        assert!(anno
            .add_hypervisor_ctlpath(&mut config, &"qemu".to_string())
            .is_ok());
        KataConfig::set_active_config(config, &"qemu", &"agent0");
        if let Some(hv) = KataConfig::get_default_config().get_hypervisor() {
            assert_eq!(hv.ctlpath, "/usr/lib/jvm".to_string());
        }
    }

    #[test]
    fn test_fail_to_change_hypervisor_ctl_path() {
        let path = env!("CARGO_MANIFEST_DIR");
        let path = Path::new(path).join("tests/texture/configuration-anno.toml");
        let content = fs::read_to_string(&path).unwrap();

        let qemu = QemuConfig::new();
        qemu.register();

        let config = TomlConfig::load(&content).unwrap();
        KataConfig::set_active_config(config, &"qemu", &"agent0");

        let mut anno_hash = HashMap::new();
        anno_hash.insert(
            KATA_ANNO_CONF_HYPERVISOR_CTLPATH.to_string(),
            "/usr/lib/rust".to_string(),
        );
        let anno = Annotation::new(anno_hash);

        let content = fs::read_to_string(&path).unwrap();
        let mut config = TomlConfig::load(&content).unwrap();

        assert!(anno
            .add_hypervisor_ctlpath(&mut config, &"qemu".to_string())
            .is_err());
    }

    #[test]
    fn test_change_enable_iothreads() {
        let path = env!("CARGO_MANIFEST_DIR");
        let path = Path::new(path).join("tests/texture/configuration-anno.toml");
        let content = fs::read_to_string(&path).unwrap();

        let qemu = QemuConfig::new();
        qemu.register();

        let config = TomlConfig::load(&content).unwrap();
        KataConfig::set_active_config(config, &"qemu", &"agent0");

        let mut anno_hash = HashMap::new();
        anno_hash.insert(
            KATA_ANNO_CONF_HYPERVISOR_ENABLE_IO_THREADS.to_string(),
            "false".to_string(),
        );
        let anno = Annotation::new(anno_hash);

        let content = fs::read_to_string(&path).unwrap();
        let mut config = TomlConfig::load(&content).unwrap();

        assert!(anno
            .add_enable_io_threads(&mut config, &"qemu".to_string())
            .is_ok());
        KataConfig::set_active_config(config, &"qemu", &"agent0");
        if let Some(hv) = KataConfig::get_default_config().get_hypervisor() {
            assert!(!hv.enable_iothreads);
        }
    }

    #[test]
    fn test_fail_to_change_enable_iothreads() {
        let path = env!("CARGO_MANIFEST_DIR");
        let path = Path::new(path).join("tests/texture/configuration-anno1.toml");
        let content = fs::read_to_string(&path).unwrap();

        let qemu = QemuConfig::new();
        qemu.register();
        let config = TomlConfig::load(&content).unwrap();
        KataConfig::set_active_config(config, &"qemu", &"agent0");

        let mut anno_hash = HashMap::new();
        anno_hash.insert(
            KATA_ANNO_CONF_HYPERVISOR_ENABLE_IO_THREADS.to_string(),
            "false".to_string(),
        );
        let anno = Annotation::new(anno_hash);
        let mut config = TomlConfig::load(&content).unwrap();

        assert!(anno
            .add_enable_io_threads(&mut config, &"qemu".to_string())
            .is_err());
    }

    #[test]
    fn test_change_default_memory() {
        let path = env!("CARGO_MANIFEST_DIR");
        let path = Path::new(path).join("tests/texture/configuration-anno.toml");
        let content = fs::read_to_string(&path).unwrap();

        let qemu = QemuConfig::new();
        qemu.register();

        let config = TomlConfig::load(&content).unwrap();
        KataConfig::set_active_config(config, &"qemu", &"agent0");

        let mut anno_hash = HashMap::new();
        anno_hash.insert(
            KATA_ANNO_CONF_HYPERVISOR_DEFAULT_MEMORY.to_string(),
            "100".to_string(),
        );
        let anno = Annotation::new(anno_hash);
        let mut config = TomlConfig::load(&content).unwrap();

        assert!(anno
            .add_hypervisor_default_memory(&mut config, &"qemu".to_string())
            .is_ok(),);

        KataConfig::set_active_config(config, &"qemu", &"agent0");
        if let Some(hv) = KataConfig::get_default_config().get_hypervisor() {
            assert_eq!(hv.memory_info.default_memory, 100);
        }
    }

    #[test]
    fn test_fail_to_change_default_memory() {
        let path = env!("CARGO_MANIFEST_DIR");
        let path = Path::new(path).join("tests/texture/configuration-anno.toml");
        let content = fs::read_to_string(&path).unwrap();

        let qemu = QemuConfig::new();
        qemu.register();

        let config = TomlConfig::load(&content).unwrap();
        KataConfig::set_active_config(config, &"qemu", &"agent0");

        let mut anno_hash = HashMap::new();
        anno_hash.insert(
            KATA_ANNO_CONF_HYPERVISOR_DEFAULT_MEMORY.to_string(),
            "10".to_string(),
        );
        let anno = Annotation::new(anno_hash);
        let mut config = TomlConfig::load(&content).unwrap();

        assert!(anno
            .add_hypervisor_default_memory(&mut config, &"qemu".to_string())
            .is_err());
    }

    #[test]
    fn test_change_memory_slots() {
        let path = env!("CARGO_MANIFEST_DIR");
        let path = Path::new(path).join("tests/texture/configuration-anno.toml");
        let content = fs::read_to_string(&path).unwrap();

        let qemu = QemuConfig::new();
        qemu.register();

        let config = TomlConfig::load(&content).unwrap();
        KataConfig::set_active_config(config, &"qemu", &"agent0");

        let mut anno_hash = HashMap::new();
        anno_hash.insert(
            KATA_ANNO_CONF_HYPERVISOR_MEMORY_SLOTS.to_string(),
            "100".to_string(),
        );
        let anno = Annotation::new(anno_hash);
        let mut config = TomlConfig::load(&content).unwrap();

        assert!(anno
            .add_hypervisor_mem_slots(&mut config, &"qemu".to_string())
            .is_ok());
        KataConfig::set_active_config(config, &"qemu", &"agent0");
        if let Some(hv) = KataConfig::get_default_config().get_hypervisor() {
            assert_eq!(hv.memory_info.memory_slots, 100);
        }
    }

    #[test]
    fn test_fail_to_change_memory_slots() {
        let path = env!("CARGO_MANIFEST_DIR");
        let path = Path::new(path).join("tests/texture/configuration-anno.toml");
        let content = fs::read_to_string(&path).unwrap();
        let config = TomlConfig::load(&content).unwrap();
        KataConfig::set_active_config(config, &"qemu", &"agent0");

        let qemu = QemuConfig::new();
        qemu.register();

        let mut anno_hash = HashMap::new();
        anno_hash.insert(
            KATA_ANNO_CONF_HYPERVISOR_MEMORY_SLOTS.to_string(),
            "-1".to_string(),
        );
        let anno = Annotation::new(anno_hash);
        let mut config = TomlConfig::load(&content).unwrap();

        assert!(anno
            .add_hypervisor_mem_slots(&mut config, &"qemu".to_string())
            .is_err());
    }

    #[test]
    fn test_change_enable_memory_prealloc() {
        let path = env!("CARGO_MANIFEST_DIR");
        let path = Path::new(path).join("tests/texture/configuration-anno.toml");
        let content = fs::read_to_string(&path).unwrap();

        let qemu = QemuConfig::new();
        qemu.register();

        let config = TomlConfig::load(&content).unwrap();
        KataConfig::set_active_config(config, &"qemu", &"agent0");

        let mut anno_hash = HashMap::new();
        anno_hash.insert(
            KATA_ANNO_CONF_HYPERVISOR_MEMORY_PREALLOC.to_string(),
            "false".to_string(),
        );
        let anno = Annotation::new(anno_hash);
        let mut config = TomlConfig::load(&content).unwrap();

        assert!(anno
            .add_hypervisor_memory_prealloc(&mut config, &"qemu".to_string(),)
            .is_ok());
        KataConfig::set_active_config(config, &"qemu", &"agent0");
        if let Some(hv) = KataConfig::get_default_config().get_hypervisor() {
            assert!(!hv.memory_info.enable_mem_prealloc);
        }
    }

    #[test]
    fn test_fail_to_change_memory_prealloc() {
        let path = env!("CARGO_MANIFEST_DIR");
        let path = Path::new(path).join("tests/texture/configuration-anno1.toml");
        let content = fs::read_to_string(&path).unwrap();

        let qemu = QemuConfig::new();
        qemu.register();

        let config = TomlConfig::load(&content).unwrap();
        KataConfig::set_active_config(config, &"qemu", &"agent0");

        let mut anno_hash = HashMap::new();
        anno_hash.insert(
            KATA_ANNO_CONF_HYPERVISOR_MEMORY_PREALLOC.to_string(),
            "flase".to_string(),
        );
        let anno = Annotation::new(anno_hash);
        let mut config = TomlConfig::load(&content).unwrap();

        assert!(anno
            .add_hypervisor_memory_prealloc(&mut config, &"qemu".to_string())
            .is_err());
    }

    #[test]
    fn test_change_enable_hugepages() {
        let path = env!("CARGO_MANIFEST_DIR");
        let path = Path::new(path).join("tests/texture/configuration-anno.toml");
        let content = fs::read_to_string(&path).unwrap();

        let qemu = QemuConfig::new();
        qemu.register();

        let config = TomlConfig::load(&content).unwrap();
        KataConfig::set_active_config(config, &"qemu", &"agent0");

        let mut anno_hash = HashMap::new();
        anno_hash.insert(
            KATA_ANNO_CONF_HYPERVISOR_HUGE_PAGES.to_string(),
            "false".to_string(),
        );
        let anno = Annotation::new(anno_hash);
        let mut config = TomlConfig::load(&content).unwrap();

        assert!(anno
            .add_hypervisor_enable_hugepages(&mut config, &"qemu".to_string(),)
            .is_ok());
        KataConfig::set_active_config(config, &"qemu", &"agent0");
        if let Some(hv) = KataConfig::get_default_config().get_hypervisor() {
            assert!(!hv.memory_info.enable_hugepages);
        }
    }

    #[test]
    fn test_fail_to_change_enable_hugepages() {
        let path = env!("CARGO_MANIFEST_DIR");
        let path = Path::new(path).join("tests/texture/configuration-anno1.toml");
        let content = fs::read_to_string(&path).unwrap();

        let qemu = QemuConfig::new();
        qemu.register();

        let config = TomlConfig::load(&content).unwrap();
        KataConfig::set_active_config(config, &"qemu", &"agent0");

        let mut anno_hash = HashMap::new();
        anno_hash.insert(
            KATA_ANNO_CONF_HYPERVISOR_HUGE_PAGES.to_string(),
            "flase".to_string(),
        );
        let anno = Annotation::new(anno_hash);
        let mut config = TomlConfig::load(&content).unwrap();

        assert!(anno
            .add_hypervisor_enable_hugepages(&mut config, &"qemu".to_string(),)
            .is_err());
    }

    #[test]
    fn test_change_file_mem_backend() {
        let path = env!("CARGO_MANIFEST_DIR");
        let path = Path::new(path).join("tests/texture/configuration-anno.toml");
        let content = fs::read_to_string(&path).unwrap();

        let qemu = QemuConfig::new();
        qemu.register();
        let config = TomlConfig::load(&content).unwrap();
        KataConfig::set_active_config(config, &"qemu", &"agent0");

        let mut anno_hash = HashMap::new();
        anno_hash.insert(
            KATA_ANNO_CONF_HYPERVISOR_FILE_BACKED_MEM_ROOT_DIR.to_string(),
            "/dev/snd".to_string(),
        );
        let anno = Annotation::new(anno_hash);
        let mut config = TomlConfig::load(&content).unwrap();

        assert!(anno
            .add_hypervisor_file_mem_backend(&mut config, &"qemu".to_string(),)
            .is_ok());
        KataConfig::set_active_config(config, &"qemu", &"agent0");
        if let Some(hv) = KataConfig::get_default_config().get_hypervisor() {
            assert_eq!(hv.memory_info.file_mem_backend, "/dev/snd");
        }
    }

    #[test]
    fn test_change_enable_virtio_mem() {
        let path = env!("CARGO_MANIFEST_DIR");
        let path = Path::new(path).join("tests/texture/configuration-anno.toml");
        let content = fs::read_to_string(&path).unwrap();

        let qemu = QemuConfig::new();
        qemu.register();

        let config = TomlConfig::load(&content).unwrap();
        KataConfig::set_active_config(config, &"qemu", &"agent0");

        let mut anno_hash = HashMap::new();
        anno_hash.insert(
            KATA_ANNO_CONF_HYPERVISOR_VIRTIO_MEM.to_string(),
            "false".to_string(),
        );
        let anno = Annotation::new(anno_hash);
        let mut config = TomlConfig::load(&content).unwrap();

        assert!(anno
            .add_hypervisor_virtio_mem(&mut config, &"qemu".to_string())
            .is_ok());
        KataConfig::set_active_config(config, "qemu", "agent0");
        assert!(
            !KataConfig::get_active_config()
                .get_hypervisor()
                .unwrap()
                .memory_info
                .enable_virtio_mem
        );
    }
}
