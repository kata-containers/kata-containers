// Copyright (c) 2019 Kata Containers community
// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeClass {
    #[serde(rename = "apiVersion")]
    pub api_version: Option<String>,
    pub kind: Option<String>,
    pub metadata: Option<Metadata>,
    pub handler: Option<String>,
    pub overhead: Option<Overhead>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub name: Option<String>,
    #[serde(flatten)]
    pub other: HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Overhead {
    #[serde(rename = "podFixed")]
    pub pod_fixed: Option<HashMap<String, serde_yaml::Value>>,
}

/// Set a value in YAML file (similar to yq)
#[cfg(test)]
pub fn set_yaml_value(file_path: &Path, key_path: &str, value: serde_yaml::Value) -> Result<()> {
    let content = std::fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read YAML file: {file_path:?}"))?;

    let mut yaml: serde_yaml::Value =
        serde_yaml::from_str(&content).context("Failed to parse YAML")?;

    let parts: Vec<&str> = key_path.split('.').collect();
    if parts.is_empty() {
        return Err(anyhow::anyhow!("Invalid YAML path: {key_path}"));
    }

    // Navigate to the target location
    let mut current = &mut yaml;
    for (i, part) in parts.iter().enumerate() {
        let is_last = i == parts.len() - 1;

        if is_last {
            // Set the value
            if let Some(map) = current.as_mapping_mut() {
                map.insert(serde_yaml::Value::String(part.to_string()), value.clone());
            } else {
                return Err(anyhow::anyhow!("Cannot set value at non-mapping node"));
            }
        } else {
            // Navigate/create intermediate mappings
            if let Some(map) = current.as_mapping_mut() {
                if !map.contains_key(&serde_yaml::Value::String(part.to_string())) {
                    map.insert(
                        serde_yaml::Value::String(part.to_string()),
                        serde_yaml::Value::Mapping(serde_yaml::Mapping::new()),
                    );
                }
                current = map
                    .get_mut(&serde_yaml::Value::String(part.to_string()))
                    .unwrap();
            } else {
                return Err(anyhow::anyhow!("Path component '{part}' is not a mapping"));
            }
        }
    }

    let updated_content = serde_yaml::to_string(&yaml).context("Failed to serialize YAML")?;

    std::fs::write(file_path, updated_content)
        .with_context(|| format!("Failed to write YAML file: {file_path:?}"))?;

    Ok(())
}

/// Adjust runtime class for NFD (Node Feature Discovery)
pub fn adjust_runtimeclass_for_nfd(file_path: &Path, key: &str, value: i64) -> Result<()> {
    let mut runtimeclass: RuntimeClass = {
        let content = std::fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read YAML file: {file_path:?}"))?;
        serde_yaml::from_str(&content).context("Failed to parse RuntimeClass YAML")?
    };

    // Initialize overhead.podFixed if needed
    if runtimeclass.overhead.is_none() {
        runtimeclass.overhead = Some(Overhead {
            pod_fixed: Some(HashMap::new()),
        });
    }

    if let Some(ref mut overhead) = runtimeclass.overhead {
        if overhead.pod_fixed.is_none() {
            overhead.pod_fixed = Some(HashMap::new());
        }

        if let Some(ref mut pod_fixed) = overhead.pod_fixed {
            pod_fixed.insert(key.to_string(), serde_yaml::Value::Number(value.into()));
        }
    }

    let updated_content =
        serde_yaml::to_string(&runtimeclass).context("Failed to serialize RuntimeClass")?;

    std::fs::write(file_path, updated_content)
        .with_context(|| format!("Failed to write YAML file: {file_path:?}"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_set_yaml_value() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        std::fs::write(path, "key: value\n").unwrap();

        set_yaml_value(
            path,
            "overhead.podFixed.test",
            serde_yaml::Value::Number(1.into()),
        )
        .unwrap();

        let content = std::fs::read_to_string(path).unwrap();
        assert!(content.contains("test"));
    }

    #[test]
    fn test_adjust_runtimeclass_for_nfd() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        let yaml_content = r#"
apiVersion: node.k8s.io/v1
kind: RuntimeClass
metadata:
  name: kata-test
handler: kata-test
"#;
        std::fs::write(path, yaml_content).unwrap();

        adjust_runtimeclass_for_nfd(path, "tdx.intel.com/keys", 1).unwrap();

        let content = std::fs::read_to_string(path).unwrap();
        assert!(content.contains("tdx.intel.com/keys"));
    }
}
