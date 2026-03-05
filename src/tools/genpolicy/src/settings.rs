// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow OCI spec field names.
#![allow(non_snake_case)]

use crate::policy;

use json_patch::{patch, Patch};
use log::debug;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::Path;

/// Policy settings loaded from genpolicy-settings.json.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Settings {
    pub pause_container: policy::KataSpec,
    pub other_container: policy::KataSpec,
    pub volumes: Volumes,
    pub devices: policy::Devices,
    pub kata_config: KataConfig,
    pub cluster_config: policy::ClusterConfig,
    pub request_defaults: policy::RequestDefaults,
    pub common: policy::CommonData,
    pub mount_destinations: Vec<String>,
    pub sandbox: policy::SandboxData,
}

/// Volume settings loaded from genpolicy-settings.json.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Volumes {
    pub emptyDir: EmptyDirVolume,
    pub emptyDir_memory: EmptyDirVolume,
    pub configMap: ConfigMapVolume,
    pub image_volume: ImageVolume,
}

/// EmptyDir volume settings loaded from genpolicy-settings.json.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmptyDirVolume {
    pub mount_type: String,
    pub mount_source: String,
    pub mount_point: String,
    pub driver: String,
    pub fstype: String,
    pub options: Vec<String>,
    pub source: String,
}

/// ConfigMap volume settings loaded from genpolicy-settings.json.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigMapVolume {
    pub mount_type: String,
    pub mount_source: String,
    pub mount_point: String,
    pub driver: String,
    pub fstype: String,
    pub options: Vec<String>,
}

/// Container image volume settings loaded from genpolicy-settings.json.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageVolume {
    pub mount_type: String,
    pub mount_source: String,
    pub driver: String,
    pub source: String,
    pub fstype: String,
    pub options: Vec<String>,
}

/// Data corresponding to the kata runtime config file data, loaded from
/// genpolicy-settings.json.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KataConfig {
    pub oci_version: String,
    pub enable_configmap_secret_storages: bool,
}

/// Drop-ins in genpolicy-settings.d/ must be RFC 6902 JSON Patch documents (JSON array of
/// operations: add, remove, replace, move, copy, test). This allows precise control (e.g. array
/// indices) and optional `test` for assertions.
impl Settings {
    pub fn new(json_settings_path: &str) -> Self {
        debug!("Loading settings from: {}", json_settings_path);
        let path = Path::new(json_settings_path);
        let (base_path, drop_in_dir) = if path.is_dir() {
            (
                path.join("genpolicy-settings.json"),
                Some(path.join("genpolicy-settings.d")),
            )
        } else {
            (path.to_path_buf(), None)
        };

        let mut base: Value = {
            let contents = fs::read_to_string(&base_path).unwrap_or_else(|e| {
                panic!(
                    "Cannot read {}: {}. Specify the path using the -j parameter.",
                    base_path.display(),
                    e
                )
            });
            serde_json::from_str(&contents)
                .unwrap_or_else(|e| panic!("Invalid JSON in {}: {}", base_path.display(), e))
        };

        if let Some(ref drop_in_dir) = drop_in_dir {
            if drop_in_dir.is_dir() {
                let mut entries: Vec<_> = fs::read_dir(drop_in_dir)
                    .unwrap_or_else(|e| {
                        panic!("Cannot read drop-in dir {}: {}", drop_in_dir.display(), e)
                    })
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
                    .collect();
                entries.sort_by_cached_key(|e| e.file_name());
                for entry in entries {
                    let p = entry.path();
                    debug!("Applying drop-in: {:?}", p);
                    let contents = fs::read_to_string(&p)
                        .unwrap_or_else(|e| panic!("Cannot read drop-in {}: {}", p.display(), e));
                    let patch_ops: Patch = serde_json::from_str(&contents).unwrap_or_else(|e| {
                        panic!("Invalid JSON Patch in drop-in {}: {}", p.display(), e)
                    });
                    patch(&mut base, &patch_ops).unwrap_or_else(|e| {
                        panic!("Failed to apply JSON Patch from {}: {}", p.display(), e)
                    });
                }
            }
        }

        let settings: Self = serde_json::from_value(base)
            .unwrap_or_else(|e| panic!("Merged settings are invalid: {}", e));
        debug!("settings = {:?}", &settings);
        Self::validate_settings(&settings);
        settings
    }

    pub fn get_container_settings(&self, is_pause_container: bool) -> &policy::KataSpec {
        if is_pause_container {
            &self.pause_container
        } else {
            &self.other_container
        }
    }

    fn validate_settings(settings: &Self) {
        if let Some(commands) = &settings.request_defaults.ExecProcessRequest.commands {
            if !commands.is_empty() {
                panic!("The settings field <request_defaults.ExecProcessRequest.commands> has been deprecated. \
                    Please use <request_defaults.ExecProcessRequest.allowed_commands> instead.");
            }
        }
    }
}
