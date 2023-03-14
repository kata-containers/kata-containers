#[cfg(test)]
use super::*;

#[test]
fn serialize_and_deserialize_spec() {
    let spec: Spec = Default::default();
    let json_string = serde_json::to_string(&spec).unwrap();
    let new_spec = serde_json::from_str(&json_string).unwrap();
    assert_eq!(spec, new_spec);
}

#[test]
fn test_linux_device_cgroup_to_string() {
    let ldc = LinuxDeviceCgroupBuilder::default()
        .allow(true)
        .typ(LinuxDeviceType::B)
        .access("rwm".to_string())
        .build()
        .expect("build device cgroup");
    assert_eq!(ldc.to_string(), "b *:* rwm");

    let ldc = LinuxDeviceCgroupBuilder::default()
        .allow(true)
        .typ(LinuxDeviceType::A)
        .major(1)
        .minor(9)
        .access("rwm".to_string())
        .build()
        .expect("build device cgroup");
    assert_eq!(ldc.to_string(), "a 1:9 rwm");
}

#[test]
fn test_load_sample_spec() {
    let fixture_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/runtime/test/fixture/sample.json");
    let err = Spec::load(fixture_path);
    assert!(err.is_ok(), "failed to load spec: {:?}", err);
}
