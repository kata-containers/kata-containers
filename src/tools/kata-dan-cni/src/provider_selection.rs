// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

use cni_protocol::{CniError, CniErrorCode, NetConf};
use dan_core::Provider;
use k8s_openapi::api::core::v1::Pod;
use kube::Client;
use provider_kube_ovn::KubeOvnProvider;
use serde_json::Value;

/// Build the DAN Provider selected by the delegate plugin type.
pub(crate) fn build_configured_provider(
    config: &NetConf,
    client: Client,
    pod: Pod,
) -> Result<Box<dyn Provider>, CniError> {
    let plugin = config.delegate.plugin();
    if plugin.plugin_type != "kube-ovn" {
        return Err(invalid_selection(format!(
            "unsupported DAN Provider selector delegate plugin type={}",
            plugin.plugin_type
        )));
    }
    let network_provider = match plugin.plugin_fields.get("provider") {
        None | Some(Value::Null) => "ovn",
        Some(Value::String(provider)) => provider.as_str(),
        Some(_) => {
            return Err(invalid_selection(
                "delegate.plugins[0].provider must be a string",
            ));
        }
    };
    let node_name = config
        .node_name
        .as_deref()
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| invalid_selection("nodeName is required for Kube-OVN Provider"))?;

    let provider = KubeOvnProvider::new(client, node_name.to_string())
        .with_provider(network_provider)
        .map_err(|error| invalid_selection(error.to_string()))?
        .with_pod(pod)
        .with_config_routes(plugin.plugin_fields.get("routes").cloned())
        .map_err(|error| invalid_selection(error.to_string()))?;
    Ok(Box::new(provider))
}

fn invalid_selection(message: impl Into<String>) -> CniError {
    CniError::new(
        CniErrorCode::InvalidNetworkConfig,
        "invalid DAN Provider selection",
        message.into(),
    )
}
