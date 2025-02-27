// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow OCI spec field names.
#![allow(non_snake_case)]

use crate::policy;

use log::debug;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::str;

/// Policy settings loaded from genpolicy-settings.json.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Settings {
    pub pause_container: policy::KataSpec,
    pub other_container: policy::KataSpec,
    pub volumes: Volumes,
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
    pub confidential_emptyDir: EmptyDirVolume,
    pub emptyDir_memory: EmptyDirVolume,
    pub configMap: ConfigMapVolume,
    pub confidential_configMap: ConfigMapVolume,
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
    pub confidential_guest: bool,
    pub oci_version: String,
}

impl Settings {
    pub fn new(json_settings_path: &str) -> Self {
        debug!("Loading settings file...");
        if let Ok(file) = File::open(json_settings_path) {
            let settings: Self = serde_json::from_reader(file).unwrap();
            debug!("settings = {:?}", &settings);
            Self::validate_settings(&settings);
            settings
        } else {
            panic!("Cannot open file {}. Please copy it to the current directory or specify the path to it using the -j parameter.",
                json_settings_path);
        }
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
