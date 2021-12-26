use kata_types::config::{QemuConfig, TomlConfig, HYPERVISOR_NAME_QEMU};
use std::fs;
use std::path::Path;

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

    assert_eq!(qemu.cpu_info.cpu_features, "pmu=off,vmx=off");
    assert_eq!(qemu.cpu_info.default_vcpus, 2);
    assert_eq!(qemu.cpu_info.default_maxvcpus, 64);
}
