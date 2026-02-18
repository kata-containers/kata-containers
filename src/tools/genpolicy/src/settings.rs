// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow OCI spec field names.
#![allow(non_snake_case)]

use crate::policy;

use log::debug;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::File;
use std::path::Path;
use std::str;

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

/// Deep-merge `override_val` into `base`. Objects are merged recursively;
/// arrays and other values are replaced by the override.
fn deep_merge_json(base: &mut Value, override_val: &Value) {
    match (base, override_val) {
        (Value::Object(ref mut base_map), Value::Object(override_map)) => {
            for (k, v) in override_map {
                if let Some(base_v) = base_map.get_mut(k) {
                    deep_merge_json(base_v, v);
                } else {
                    base_map.insert(k.clone(), v.clone());
                }
            }
        }
        (base_val, override_val) => {
            *base_val = override_val.clone();
        }
    }
}

impl Settings {
    /// Load settings from a file or from a settings directory (base + genpolicy-settings.d drop-ins).
    /// If `json_settings_path` is a directory, loads `genpolicy-settings.json` and merges
    /// all `genpolicy-settings.d/*.json` in lexicographic order.
    pub fn new(json_settings_path: &str) -> Self {
        debug!("Loading settings from {}...", json_settings_path);
        let path = Path::new(json_settings_path);
        let value: Value = if path.is_dir() {
            let base_path = path.join("genpolicy-settings.json");
            let file = File::open(&base_path).unwrap_or_else(|e| {
                panic!(
                    "Cannot open base settings file {}. Please ensure the path is a settings directory containing genpolicy-settings.json, or pass a settings file. Error: {}",
                    base_path.display(),
                    e
                );
            });
            let mut base: Value = serde_json::from_reader(file)
                .unwrap_or_else(|e| panic!("Invalid JSON in {}: {}", base_path.display(), e));
            let drop_in_dir = path.join("genpolicy-settings.d");
            if drop_in_dir.is_dir() {
                let mut entries: Vec<_> = std::fs::read_dir(&drop_in_dir)
                    .unwrap_or_else(|e| {
                        panic!(
                            "Cannot read settings drop-in dir {}: {}",
                            drop_in_dir.display(),
                            e
                        )
                    })
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
                    .collect();
                entries.sort_by_cached_key(|e| e.file_name());
                for entry in entries {
                    let p = entry.path();
                    debug!("Merging drop-in: {}", p.display());
                    let file = File::open(&p)
                        .unwrap_or_else(|e| panic!("Cannot open drop-in {}: {}", p.display(), e));
                    let override_val: Value = serde_json::from_reader(file).unwrap_or_else(|e| {
                        panic!("Invalid JSON in drop-in {}: {}", p.display(), e)
                    });
                    deep_merge_json(&mut base, &override_val);
                }
            }
            base
        } else {
            let file = File::open(json_settings_path).unwrap_or_else(|_| {
                panic!(
                    "Cannot open file {json_settings_path}. Please copy it to the current directory or specify the path to it using the -j parameter."
                );
            });
            serde_json::from_reader(file)
                .unwrap_or_else(|e| panic!("Invalid JSON in {json_settings_path}: {e}"))
        };
        let settings: Self = serde_json::from_value(value)
            .unwrap_or_else(|e| panic!("Settings structure invalid after loading: {e}"));
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
