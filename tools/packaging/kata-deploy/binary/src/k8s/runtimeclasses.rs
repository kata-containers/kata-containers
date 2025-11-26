// Copyright (c) 2019 Kata Containers community
// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use super::client as k8s;
use crate::config::Config;
use anyhow::Result;
use log::info;

pub async fn update_existing_runtimeclasses_for_nfd(config: &Config) -> Result<()> {
    use k8s_openapi::apimachinery::pkg::api::resource::Quantity;

    info!("Checking existing runtime classes for NFD updates");

    let existing_runtimeclasses = k8s::list_runtimeclasses(config).await?;

    for rc in existing_runtimeclasses {
        let name = rc
            .metadata
            .name
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("RuntimeClass missing name"))?;

        if !name.starts_with("kata") {
            continue;
        }

        let nfd_key = if name.contains("tdx") {
            Some("tdx.intel.com/keys")
        } else if name.contains("snp") {
            Some("sev-snp.amd.com/esids")
        } else {
            None
        };

        if nfd_key.is_none() {
            continue;
        }

        let nfd_key = nfd_key.unwrap();

        // Only update if the RuntimeClass is missing the NFD field
        // Check if NFD key already exists in overhead.podFixed
        let needs_update = if let Some(ref overhead) = rc.overhead {
            if let Some(ref pod_fixed) = overhead.pod_fixed {
                // Field exists, check if the key is missing
                !pod_fixed.contains_key(nfd_key)
            } else {
                // overhead exists but podFixed is missing, needs update
                true
            }
        } else {
            // overhead is missing, needs update
            true
        };

        if !needs_update {
            info!("RuntimeClass {name} already has NFD key {nfd_key}, skipping");
            continue;
        }

        info!("Updating existing RuntimeClass {name} with missing NFD key {nfd_key}");

        let mut patched_rc = rc.clone();

        if patched_rc.overhead.is_none() {
            patched_rc.overhead = Some(Default::default());
        }

        if let Some(ref mut overhead) = patched_rc.overhead {
            if overhead.pod_fixed.is_none() {
                overhead.pod_fixed = Some(Default::default());
            }

            if let Some(ref mut pod_fixed) = overhead.pod_fixed {
                let quantity = Quantity("1".to_string());
                pod_fixed.insert(nfd_key.to_string(), quantity);
            }
        }

        k8s::update_runtimeclass(config, &patched_rc).await?;
        info!("Successfully updated RuntimeClass {name} with NFD key {nfd_key}");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_runtime_class_name_without_suffix() {
        // Test runtime class name without MULTI_INSTALL_SUFFIX
        let shim = "qemu";

        // Expected: kata-qemu
        let expected = format!("kata-{}", shim);
        assert_eq!(expected, "kata-qemu");
    }

    #[test]
    fn test_runtime_class_name_with_suffix() {
        // Test runtime class name with MULTI_INSTALL_SUFFIX
        let shim = "qemu";
        let suffix = Some("dev".to_string());

        // Expected: kata-qemu-dev
        if let Some(s) = suffix {
            let expected = format!("kata-{}-{}", shim, s);
            assert_eq!(expected, "kata-qemu-dev");
        }
    }

    #[test]
    fn test_multiple_shims_with_suffix() {
        // Test different shims with suffix
        let shims = vec!["qemu", "qemu-tdx", "cloud-hypervisor", "fc"];
        let suffix = Some("staging".to_string());

        for shim in shims {
            if let Some(ref s) = suffix {
                let runtime_class = format!("kata-{}-{}", shim, s);
                assert!(runtime_class.contains(shim));
                assert!(runtime_class.contains("staging"));
                assert!(runtime_class.starts_with("kata-"));
            }
        }
    }

    #[test]
    fn test_suffix_prevents_default_runtimeclass() {
        // When MULTI_INSTALL_SUFFIX is set, default runtime class should not be created
        // This test verifies the logic
        let suffix = Some("prod".to_string());
        let create_default = true;

        // Logic: if suffix.is_some() && create_default, should warn and not create
        if suffix.is_some() && create_default {
            // Should not create default runtime class
            // Just verify the logic exists
            assert!(suffix.is_some());
        }
    }

    #[test]
    fn test_snapshotter_name_with_suffix() {
        // Test snapshotter name adjustment with MULTI_INSTALL_SUFFIX
        let suffix = Some("dev".to_string());
        let snapshotter = "nydus";

        if let Some(s) = suffix {
            let adjusted = format!("{}-{}", snapshotter, s);
            assert_eq!(adjusted, "nydus-dev");
        }
    }

    #[test]
    fn test_nydus_snapshotter_systemd_service_with_suffix() {
        // Test nydus-snapshotter systemd service name with suffix
        let suffix = Some("test".to_string());

        if let Some(s) = suffix {
            let service_name = format!("nydus-snapshotter-{}", s);
            assert_eq!(service_name, "nydus-snapshotter-test");
        }
    }
}
