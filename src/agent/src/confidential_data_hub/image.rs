// Copyright (c) 2021 Alibaba Cloud
// Copyright (c) 2021, 2023 IBM Corporation
// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use safe_path::scoped_join;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use kata_sys_util::validate::verify_id;
use oci_spec::runtime as oci;

use crate::rpc::CONTAINER_BASE;

use kata_types::mount::KATA_VIRTUAL_VOLUME_IMAGE_GUEST_PULL;
use protocols::agent::Storage;

pub const KATA_IMAGE_WORK_DIR: &str = "/run/kata-containers/image/";
const CONFIG_JSON: &str = "config.json";
const KATA_PAUSE_BUNDLE: &str = "/pause_bundle";

const K8S_CONTAINER_TYPE_KEYS: [&str; 2] = [
    "io.kubernetes.cri.container-type",
    "io.kubernetes.cri-o.ContainerType",
];

// Convenience function to obtain the scope logger.
fn sl() -> slog::Logger {
    slog_scope::logger().new(o!("subsystem" => "image"))
}

// Function to copy a file if it does not exist at the destination
// This function creates a dir, writes a file and if necessary,
// overwrites an existing file.
fn copy_if_not_exists(src: &Path, dst: &Path) -> Result<()> {
    if let Some(dst_dir) = dst.parent() {
        fs::create_dir_all(dst_dir)?;
    }
    fs::copy(src, dst)?;
    Ok(())
}

/// get guest pause image process specification
fn get_pause_image_process() -> Result<oci::Process> {
    let guest_pause_bundle = Path::new(KATA_PAUSE_BUNDLE);
    if !guest_pause_bundle.exists() {
        bail!("Pause image not present in rootfs");
    }
    let guest_pause_config = scoped_join(guest_pause_bundle, CONFIG_JSON)?;

    let image_oci = oci::Spec::load(guest_pause_config.to_str().ok_or_else(|| {
        anyhow!(
            "Failed to load the guest pause image config from {:?}",
            guest_pause_config
        )
    })?)
    .context("load image config file")?;

    let image_oci_process = image_oci.process().as_ref().ok_or_else(|| {
            anyhow!("The guest pause image config does not contain a process specification. Please check the pause image.")
        })?;
    Ok(image_oci_process.clone())
}

/// pause image is packaged in rootfs
pub fn unpack_pause_image(cid: &str) -> Result<String> {
    verify_id(cid).context("The guest pause image cid contains invalid characters.")?;

    let guest_pause_bundle = Path::new(KATA_PAUSE_BUNDLE);
    if !guest_pause_bundle.exists() {
        bail!("Pause image not present in rootfs");
    }
    let guest_pause_config = scoped_join(guest_pause_bundle, CONFIG_JSON)?;
    info!(sl(), "use guest pause image cid {:?}", cid);

    let image_oci = oci::Spec::load(guest_pause_config.to_str().ok_or_else(|| {
        anyhow!(
            "Failed to load the guest pause image config from {:?}",
            guest_pause_config
        )
    })?)
    .context("load image config file")?;

    let image_oci_process = image_oci.process().as_ref().ok_or_else(|| {
            anyhow!("The guest pause image config does not contain a process specification. Please check the pause image.")
        })?;
    info!(
        sl(),
        "pause image oci process {:?}",
        image_oci_process.clone()
    );

    // Ensure that the args vector is not empty before accessing its elements.
    // Check the number of arguments.
    let args = if let Some(args_vec) = image_oci_process.args() {
        args_vec
    } else {
        bail!("The number of args should be greater than or equal to one! Please check the pause image.");
    };

    let pause_bundle = scoped_join(CONTAINER_BASE, cid)?;
    fs::create_dir_all(&pause_bundle)?;
    let pause_rootfs = scoped_join(&pause_bundle, "rootfs")?;
    fs::create_dir_all(&pause_rootfs)?;
    info!(sl(), "pause_rootfs {:?}", pause_rootfs);

    copy_if_not_exists(&guest_pause_config, &pause_bundle.join(CONFIG_JSON))?;
    let arg_path = Path::new(&args[0]).strip_prefix("/")?;
    copy_if_not_exists(
        &guest_pause_bundle.join("rootfs").join(arg_path),
        &pause_rootfs.join(arg_path),
    )?;
    Ok(pause_rootfs.display().to_string())
}

/// check whether the image is for sandbox or for container.
pub fn is_sandbox(image_metadata: &HashMap<String, String>) -> bool {
    let mut is_sandbox = false;
    for key in K8S_CONTAINER_TYPE_KEYS.iter() {
        if let Some(value) = image_metadata.get(key as &str) {
            if value == "sandbox" {
                is_sandbox = true;
                break;
            }
        }
    }
    is_sandbox
}

/// get_process overrides the OCI process spec with pause image process spec if needed
pub fn get_process(
    ocip: &oci::Process,
    oci: &oci::Spec,
    storages: Vec<Storage>,
) -> Result<oci::Process> {
    let mut guest_pull = false;
    for storage in storages {
        if storage.driver == KATA_VIRTUAL_VOLUME_IMAGE_GUEST_PULL {
            guest_pull = true;
            break;
        }
    }
    if guest_pull {
        if let Some(a) = oci.annotations() {
            if is_sandbox(a) {
                return get_pause_image_process();
            }
        }
    }

    Ok(ocip.clone())
}


#[cfg(test)]
mod tests {
    use super::*;
    use oci_spec::runtime as oci;
    use rstest::rstest;
    use std::collections::HashMap;
    use std::fs;
    use tempfile::tempdir;

    // Helper to create metadata with annotation
    fn create_metadata(key: &str, value: &str) -> HashMap<String, String> {
        let mut metadata = HashMap::new();
        metadata.insert(key.to_string(), value.to_string());
        metadata
    }

    #[rstest]
    #[case::simple_copy("source.txt", "subdir/dest.txt", b"test content", true)]
    #[case::nested_dirs("source.txt", "deep/nested/path/dest.txt", b"test", true)]
    #[case::overwrite_existing("source.txt", "dest.txt", b"new content", true)]
    fn test_copy_if_not_exists(
        #[case] src_name: &str,
        #[case] dst_name: &str,
        #[case] content: &[u8],
        #[case] should_succeed: bool,
    ) {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let src_path = temp_dir.path().join(src_name);
        let dst_path = temp_dir.path().join(dst_name);

        fs::write(&src_path, content).expect("Failed to write source file");

        // For overwrite test, create existing destination
        if dst_name == "dest.txt" {
            fs::write(&dst_path, b"old content").expect("Failed to write dest");
        }

        let result = copy_if_not_exists(&src_path, &dst_path);
        assert_eq!(result.is_ok(), should_succeed);

        if should_succeed {
            assert!(dst_path.exists(), "Destination file should exist");
            let read_content = fs::read(&dst_path).expect("Failed to read dest file");
            assert_eq!(read_content, content);
        }
    }

    #[test]
    fn test_copy_if_not_exists_nonexistent_source() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let result = copy_if_not_exists(
            &temp_dir.path().join("nonexistent.txt"),
            &temp_dir.path().join("dest.txt")
        );
        assert!(result.is_err(), "Should fail with nonexistent source");
    }

    #[rstest]
    #[case::cri_sandbox("io.kubernetes.cri.container-type", "sandbox", true)]
    #[case::crio_sandbox("io.kubernetes.cri-o.ContainerType", "sandbox", true)]
    #[case::cri_container("io.kubernetes.cri.container-type", "container", false)]
    #[case::case_sensitive_mismatch("io.kubernetes.cri.container-type", "Sandbox", false)]
    #[case::whitespace_mismatch("io.kubernetes.cri.container-type", " sandbox ", false)]
    fn test_is_sandbox_variations(
        #[case] key: &str,
        #[case] value: &str,
        #[case] expected: bool,
    ) {
        let metadata = create_metadata(key, value);
        assert_eq!(is_sandbox(&metadata), expected);
    }

    #[test]
    fn test_is_sandbox_with_no_metadata() {
        assert!(!is_sandbox(&HashMap::new()), "Empty metadata should not be sandbox");
    }

    #[test]
    fn test_is_sandbox_with_multiple_keys() {
        let mut metadata = create_metadata("io.kubernetes.cri.container-type", "container");
        metadata.insert("io.kubernetes.cri-o.ContainerType".to_string(), "sandbox".to_string());
        assert!(is_sandbox(&metadata), "Should identify sandbox when any key matches");
    }


    // Helper to create a test process
    fn create_test_process() -> oci::Process {
        oci::ProcessBuilder::default()
            .args(vec!["test".to_string()])
            .build()
            .expect("Failed to build process")
    }

    // Helper to create storage with specific driver
    fn create_storage(driver: &str) -> protocols::agent::Storage {
        let mut storage = protocols::agent::Storage::new();
        storage.driver = driver.to_string();
        storage
    }

    #[rstest]
    #[case::no_storage(vec![], None, true)]
    #[case::non_guest_pull_storage(vec![create_storage("other-driver")], None, true)]
    #[case::guest_pull_non_sandbox(
        vec![create_storage(kata_types::mount::KATA_VIRTUAL_VOLUME_IMAGE_GUEST_PULL)],
        Some(create_metadata("io.kubernetes.cri.container-type", "container")),
        true
    )]
    fn test_get_process_variations(
        #[case] storages: Vec<protocols::agent::Storage>,
        #[case] annotations: Option<HashMap<String, String>>,
        #[case] should_return_original: bool,
    ) {
        let process = create_test_process();
        let mut spec_builder = oci::SpecBuilder::default().process(process.clone());
        
        if let Some(ann) = annotations {
            spec_builder = spec_builder.annotations(ann);
        }
        
        let spec = spec_builder.build().expect("Failed to build spec");
        let result = get_process(&process, &spec, storages);

        assert!(result.is_ok(), "get_process should succeed");
        if should_return_original {
            let returned_process = result.unwrap();
            assert_eq!(returned_process.args(), process.args());
        }
    }

    #[rstest]
    #[case::path_traversal("../malicious")]
    #[case::contains_slashes("container/with/slash")]
    #[case::null_byte("container\0null")]
    #[case::empty_string("")]
    fn test_unpack_pause_image_rejects_invalid_cid(#[case] invalid_cid: &str) {
        let result = unpack_pause_image(invalid_cid);
        assert!(
            result.is_err(),
            "Should reject invalid container ID: '{}'",
            invalid_cid
        );
    }

    // Helper to verify error message contains expected text
    fn assert_error_contains(result: Result<impl std::fmt::Debug>, expected_msg: &str) {
        assert!(result.is_err(), "Should return an error");
        if let Err(e) = result {
            let error_msg = format!("{}", e);
            assert!(
                error_msg.contains(expected_msg),
                "Error should mention '{}': {}",
                expected_msg,
                error_msg
            );
        }
    }

    #[test]
    fn test_unpack_pause_image_no_pause_bundle() {
        let result = unpack_pause_image("valid-container-id");
        assert_error_contains(result, "Pause image not present");
    }

    #[test]
    fn test_get_pause_image_process_no_bundle() {
        let result = get_pause_image_process();
        assert_error_contains(result, "Pause image not present");
    }

    #[test]
    fn test_k8s_container_type_keys() {
        assert_eq!(K8S_CONTAINER_TYPE_KEYS.len(), 2);
        assert_eq!(K8S_CONTAINER_TYPE_KEYS[0], "io.kubernetes.cri.container-type");
        assert_eq!(K8S_CONTAINER_TYPE_KEYS[1], "io.kubernetes.cri-o.ContainerType");
    }
}
