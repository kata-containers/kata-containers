// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow K8s YAML field names.
#![allow(non_snake_case)]

use crate::obj_meta;
use crate::pod;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// See PodTemplate in the Kubernetes API reference.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PodTemplate {
    pub metadata: obj_meta::ObjectMeta,
    pub spec: PodTemplateSpec,
}

/// See PodTemplateSpec in the Kubernetes API reference.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PodTemplateSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nodeSelector: Option<BTreeMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    runtimeClassName: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub containers: Option<Vec<pod::Container>>,
}
