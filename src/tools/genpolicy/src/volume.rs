// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use crate::pod;

use serde::{Deserialize, Serialize};

/// See Reference / Kubernetes API / Config and Storage Resources / Volume.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Volume {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub emptyDir: Option<EmptyDirVolumeSource>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostPath: Option<HostPathVolumeSource>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub persistentVolumeClaim: Option<PersistentVolumeClaimVolumeSource>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub configMap: Option<ConfigMapVolumeSource>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub azureFile: Option<AzureFileVolumeSource>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub projected: Option<ProjectedVolumeSource>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret: Option<SecretVolumeSource>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub downwardAPI: Option<DownwardAPIVolumeSource>, // TODO: additional fields.
}

/// See Reference / Kubernetes API / Config and Storage Resources / Volume.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HostPathVolumeSource {
    pub path: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
}

/// See Reference / Kubernetes API / Config and Storage Resources / Volume.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmptyDirVolumeSource {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub medium: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub sizeLimit: Option<String>,
}

/// See Reference / Kubernetes API / Config and Storage Resources / Volume.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersistentVolumeClaimVolumeSource {
    pub claimName: String,
    // TODO: additional fields.
}

/// See Reference / Kubernetes API / Config and Storage Resources / Volume.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigMapVolumeSource {
    pub name: String,
    pub items: Option<Vec<KeyToPath>>,
    optional: Option<bool>,
    // TODO: additional fields.
}

/// See Reference / Kubernetes API / Config and Storage Resources / Volume.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeyToPath {
    pub key: String,
    pub path: String,
}

/// See Reference / Kubernetes API / Config and Storage Resources / Volume.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AzureFileVolumeSource {
    pub secretName: String,
    pub shareName: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub readOnly: Option<bool>,
}

/// See Reference / Kubernetes API / Config and Storage Resources / Volume.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectedVolumeSource {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defaultMode: Option<i32>,
    // TODO: additional fields.
}

/// See Reference / Kubernetes API / Config and Storage Resources / Volume.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SecretVolumeSource {
    secretName: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub defaultMode: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<KeyToPath>>,
    // TODO: additional fields.
}

/// See Reference / Kubernetes API / Config and Storage Resources / Volume.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DownwardAPIVolumeSource {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<DownwardAPIVolumeFile>>,
    // TODO: additional fields.
}

/// See Reference / Kubernetes API / Config and Storage Resources / Volume.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DownwardAPIVolumeFile {
    pub path: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub fieldRef: Option<pod::ObjectFieldSelector>,
}
