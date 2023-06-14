// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use serde::{Deserialize, Serialize};

// See Volumes in the Kubernetes API reference.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Volume {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub emptyDir: Option<EmptyDirVolume>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostPath: Option<HostPathVolume>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub persistentVolumeClaim: Option<VolumeClaimVolume>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub configMap: Option<ConfigMapVolume>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub azureFile: Option<AzureFileVolumeSource>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostPathVolume {
    pub path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmptyDirVolume {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sizeLimit: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VolumeClaimVolume {
    pub claimName: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigMapVolume {
    pub name: String,
    pub items: Vec<ConfigMapVolumeItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigMapVolumeItem {
    pub key: String,
    pub path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AzureFileVolumeSource {
    pub secretName: String,
    pub shareName: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub readOnly: Option<bool>,
}
