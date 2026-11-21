// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

use crate::{CniCommand, CniError, CniErrorCode};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub const PLUGIN_TYPE: &str = "kata-dan-cni";
pub const SUPPORTED_CNI_VERSIONS: &[&str] = &["0.1.0", "0.2.0", "0.3.0", "0.3.1", "0.4.0", "1.0.0"];

fn default_cni_version() -> String {
    // CNI specifies 0.2.0 for old configurations that omit cniVersion.
    "0.2.0".into()
}

/// Fields common to the CNI request received by a single plugin.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CniNetConf {
    #[serde(default = "default_cni_version")]
    pub cni_version: String,
    pub name: String,
    #[serde(rename = "type")]
    pub plugin_type: String,
    #[serde(default)]
    pub capabilities: BTreeMap<String, bool>,
    #[serde(default)]
    pub prev_result: Option<Value>,
}

/// Complete stdin configuration for the Kata DAN CNI plugin.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetConf {
    #[serde(flatten)]
    pub cni: CniNetConf,

    /// Primary CNI plugin for ordinary Pods.
    pub delegate: NetConfList,

    /// Required RuntimeClass for DAN ADD.
    #[serde(default)]
    pub dan_runtime_class: Option<String>,

    #[serde(default)]
    pub backend: Option<Value>,
    #[serde(default)]
    pub dan_conf_dir: Option<PathBuf>,
    #[serde(default)]
    pub kubeconfig: Option<PathBuf>,
    #[serde(default)]
    pub node_name: Option<String>,

    #[serde(default)]
    pub runtime_config: Map<String, Value>,
}

/// One plugin entry inside a CNI configuration list.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginConf {
    #[serde(rename = "type")]
    pub plugin_type: String,
    #[serde(default)]
    pub capabilities: BTreeMap<String, bool>,
    #[serde(flatten)]
    pub plugin_fields: Map<String, Value>,
}

impl PluginConf {
    fn request_fields(&self) -> Map<String, Value> {
        let mut fields = self.plugin_fields.clone();
        for reserved in ["type", "name", "cniVersion", "runtimeConfig", "prevResult"] {
            fields.remove(reserved);
        }
        fields.insert("type".into(), Value::String(self.plugin_type.clone()));
        fields
    }
}

/// Configuration for a single primary plugin. Chained plugins belong to the outer conflist.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetConfList {
    pub name: String,
    #[serde(default)]
    pub disable_check: bool,
    pub plugins: [PluginConf; 1],
}

impl NetConfList {
    pub fn validate(&self) -> Result<(), CniError> {
        let plugin_type = &self.plugin().plugin_type;
        validate_plugin_type(plugin_type)?;
        if plugin_type == PLUGIN_TYPE {
            return Err(invalid_delegate(format!(
                "delegate.plugins[0] recursively references {PLUGIN_TYPE}"
            )));
        }
        Ok(())
    }

    pub fn plugin(&self) -> &PluginConf {
        &self.plugins[0]
    }

    /// Build delegate stdin, forwarding only declared runtime capabilities.
    pub fn derive_plugin_request(
        &self,
        cni_version: &str,
        command: CniCommand,
        runtime_config: &Map<String, Value>,
        previous_result: Option<&Value>,
    ) -> Value {
        let plugin = self.plugin();
        let mut request = plugin.request_fields();
        request.insert("name".into(), Value::String(self.name.clone()));
        request.insert("cniVersion".into(), Value::String(cni_version.into()));

        let filtered_runtime = runtime_config
            .iter()
            .filter(|(name, _)| plugin.capabilities.get(*name).copied().unwrap_or(false))
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect::<Map<_, _>>();
        if !filtered_runtime.is_empty() {
            request.insert("runtimeConfig".into(), Value::Object(filtered_runtime));
        }

        let include_previous_result = previous_result.is_some()
            && !(command == CniCommand::Del
                && matches!(cni_version, "0.1.0" | "0.2.0" | "0.3.0" | "0.3.1"));
        if let Some(previous_result) = previous_result.filter(|_| include_previous_result) {
            request.insert("prevResult".into(), previous_result.clone());
        }
        Value::Object(request)
    }
}

impl NetConf {
    pub fn parse(input: &[u8]) -> Result<Self, CniError> {
        let value: Value = serde_json::from_slice(input).map_err(|source| {
            CniError::new(
                CniErrorCode::DecodeFailure,
                "failed to decode CNI network configuration",
                source.to_string(),
            )
        })?;
        let cni_version = value
            .get("cniVersion")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let config: Self = serde_json::from_value(value).map_err(|source| {
            let error = CniError::new(
                CniErrorCode::DecodeFailure,
                "failed to decode CNI network configuration",
                source.to_string(),
            );
            match cni_version.as_deref() {
                Some(version) => error.with_cni_version(version),
                None => error,
            }
        })?;
        config
            .validate()
            .map_err(|error| error.with_cni_version(config.cni.cni_version.clone()))?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), CniError> {
        validate_cni_identity(&self.cni.cni_version, &self.cni.name)
    }
}

fn validate_cni_identity(cni_version: &str, name: &str) -> Result<(), CniError> {
    if !SUPPORTED_CNI_VERSIONS.contains(&cni_version) {
        return Err(CniError::new(
            CniErrorCode::IncompatibleVersion,
            "unsupported CNI version",
            format!(
                "cniVersion={cni_version} is not one of {}",
                SUPPORTED_CNI_VERSIONS.join(", ")
            ),
        ));
    }
    validate_network_name(name)?;
    Ok(())
}

fn validate_plugin_type(plugin_type: &str) -> Result<(), CniError> {
    if plugin_type.is_empty() || plugin_type.contains('/') {
        return Err(invalid_delegate(
            "delegate.plugins[0].type is not a safe CNI binary name",
        ));
    }
    Ok(())
}

fn validate_network_name(name: &str) -> Result<(), CniError> {
    let valid = !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
        && name
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric);
    if !valid {
        return Err(CniError::new(
            CniErrorCode::InvalidNetworkConfig,
            "invalid network name",
            "name must start with an ASCII alphanumeric and contain only ASCII alphanumerics, '_', '.', or '-'",
        ));
    }
    Ok(())
}

fn invalid_delegate(details: impl Into<String>) -> CniError {
    CniError::new(
        CniErrorCode::InvalidNetworkConfig,
        "invalid delegate configuration",
        details,
    )
}

#[cfg(test)]
mod tests {
    use crate::{CniCommand, CniErrorCode, NetConf};
    use serde_json::{json, Value};

    fn config() -> Value {
        json!({
            "cniVersion": "1.0.0",
            "name": "pod-network",
            "type": "kata-dan-cni",
            "delegate": {"name": "kube-ovn", "plugins": [{"type": "kube-ovn"}]}
        })
    }

    #[test]
    fn parse_config() {
        let mut value = config();
        value.as_object_mut().expect("object").remove("cniVersion");
        let parsed =
            NetConf::parse(&serde_json::to_vec(&value).expect("serialize")).expect("legacy config");
        assert_eq!(parsed.cni.cni_version, "0.2.0");

        for (field, replacement, code) in [
            (
                "cniVersion",
                json!("1.1.0"),
                CniErrorCode::IncompatibleVersion,
            ),
            (
                "name",
                json!("../escape"),
                CniErrorCode::InvalidNetworkConfig,
            ),
            ("delegate", json!(null), CniErrorCode::DecodeFailure),
        ] {
            let mut value = config();
            value[field] = replacement;
            let error = NetConf::parse(&serde_json::to_vec(&value).expect("serialize"))
                .expect_err("invalid config");
            assert_eq!(error.code(), code.value());
            assert_eq!(
                error.response().cni_version.as_deref(),
                value["cniVersion"].as_str()
            );
        }
        assert_eq!(
            NetConf::parse(b"not JSON")
                .expect_err("malformed JSON")
                .code(),
            CniErrorCode::DecodeFailure.value()
        );
    }

    #[test]
    fn delegate_request() {
        let mut value = config();
        value["delegate"]["plugins"][0] = json!({
            "type": "kube-ovn", "capabilities": {"portMappings": true, "bandwidth": false},
            "name": "injected", "cniVersion": "9.0.0", "runtimeConfig": {"evil": true},
            "prevResult": {"injected": true}, "server_socket": "/run/ovn.sock"
        });
        let parsed =
            NetConf::parse(&serde_json::to_vec(&value).expect("serialize")).expect("valid config");
        let runtime = json!({"portMappings": [{"hostPort": 8080}], "bandwidth": {}, "other": 42});
        let previous = json!({"cniVersion": "1.0.0", "interfaces": [{"name": "eth0"}]});
        let request = parsed.delegate.derive_plugin_request(
            "1.0.0",
            CniCommand::Add,
            runtime.as_object().expect("object"),
            Some(&previous),
        );
        assert_eq!(
            request,
            json!({
                "type": "kube-ovn", "name": "kube-ovn", "cniVersion": "1.0.0",
                "server_socket": "/run/ovn.sock", "runtimeConfig": {"portMappings": [{"hostPort": 8080}]},
                "prevResult": previous
            })
        );
        for version in ["0.1.0", "0.2.0", "0.3.0", "0.3.1", "0.4.0", "1.0.0"] {
            let request = parsed.delegate.derive_plugin_request(
                version,
                CniCommand::Del,
                &Default::default(),
                Some(&previous),
            );
            assert_eq!(
                request.get("prevResult").is_some(),
                matches!(version, "0.4.0" | "1.0.0")
            );
            assert!(request.get("runtimeConfig").is_none());
        }
    }
}
