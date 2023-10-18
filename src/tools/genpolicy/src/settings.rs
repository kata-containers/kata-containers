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
#[derive(Debug, Deserialize, Serialize)]
pub struct Settings {
    pub pause_container: policy::KataSpec,
    pub other_container: policy::KataSpec,
    pub volumes: Volumes,
    pub kata_config: KataConfig,
    pub request_defaults: policy::RequestDefaults,
    pub common: policy::CommonData,
    pub mount_destinations: Vec<String>,
}

/// Volume settings loaded from genpolicy-settings.json.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Volumes {
    pub emptyDir: EmptyDirVolume,
    pub emptyDir_memory: EmptyDirVolume,
    pub configMap: ConfigMapVolume,
    pub confidential_configMap: ConfigMapVolume,
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

/// Data corresponding to the kata runtime config file data, loaded from
/// genpolicy-settings.json.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KataConfig {
    pub confidential_guest: bool,
}

impl Settings {
    pub fn new(settings_file: &str) -> Self {
        debug!("Loading settings file...");
        if let Ok(file) = File::open(settings_file) {
            let settings: Self = serde_json::from_reader(file).unwrap();
            debug!("settings = {:?}", &settings);
            settings
        } else {
            panic!("Cannot open file {}. Please copy it to the current directory or specify the path to it using the -i parameter.", 
                settings_file);
        }
    }

    pub fn get_container_settings(&self, is_pause_container: bool) -> &policy::KataSpec {
        if is_pause_container {
            &self.pause_container
        } else {
            &self.other_container
        }
    }
}
