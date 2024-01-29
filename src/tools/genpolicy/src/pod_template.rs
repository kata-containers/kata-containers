// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use crate::obj_meta;
use crate::pod;

use serde::{Deserialize, Serialize};

/// Reference / Kubernetes API / Workload /  Resources / PodTemplate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PodTemplate {
    apiVersion: String,
    kind: String,
    metadata: obj_meta::ObjectMeta,
    spec: PodTemplateSpec,
}

/// Reference / Kubernetes API / Workload /  Resources / PodTemplate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PodTemplateSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<obj_meta::ObjectMeta>,

    pub spec: pod::PodSpec,
}
