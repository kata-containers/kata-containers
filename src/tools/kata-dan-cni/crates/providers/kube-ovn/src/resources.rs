// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

use dan_core::ProviderError;
use kube::core::{ApiResource, DynamicObject, GroupVersionKind};
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SubnetSpec {
    #[serde(default)]
    pub(crate) mtu: u32,
    #[serde(default)]
    pub(crate) logical_gateway: bool,
    #[serde(default)]
    pub(crate) u2o_interconnection: bool,
    #[serde(default)]
    pub(crate) node_network: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SubnetStatus {
    #[serde(default, rename = "u2oInterconnectionIP")]
    pub(crate) u2o_interconnection_ip: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub(crate) struct SubnetData {
    pub(crate) spec: SubnetSpec,
    #[serde(default)]
    pub(crate) status: SubnetStatus,
}

pub(crate) fn api_resource() -> ApiResource {
    ApiResource::from_gvk_with_plural(
        &GroupVersionKind::gvk("kubeovn.io", "v1", "Subnet"),
        "subnets",
    )
}

pub(crate) fn parse(object: DynamicObject) -> Result<SubnetData, ProviderError> {
    let name = object.metadata.name;
    serde_json::from_value(object.data).map_err(|error| {
        let context = name.filter(|name| !name.is_empty()).map_or_else(
            || "Kube-OVN Subnet".to_string(),
            |name| format!("Kube-OVN Subnet `{name}`"),
        );
        ProviderError::InvalidData(format!("invalid {context} data: {error}"))
    })
}
