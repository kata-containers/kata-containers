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
