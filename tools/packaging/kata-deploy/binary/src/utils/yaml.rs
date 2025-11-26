// Copyright (c) 2019 Kata Containers community
// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
use anyhow::{Context, Result};
#[cfg(test)]
use std::path::Path;

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
}
