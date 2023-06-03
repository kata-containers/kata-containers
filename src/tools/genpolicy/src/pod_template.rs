// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use crate::obj_meta;
use crate::pod;

use serde::{Deserialize, Serialize};

/// Reference / Kubernetes API / Workload/  Resources / PodTemplate.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PodTemplate {
    apiVersion: String,
    kind: String,
    pub metadata: obj_meta::ObjectMeta,
    pub spec: PodTemplateSpec,
}

/// Reference / Kubernetes API / Workload/  Resources / PodTemplate.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PodTemplateSpec {
    pub metadata: obj_meta::ObjectMeta,
    pub spec: pod::PodSpec,
}
