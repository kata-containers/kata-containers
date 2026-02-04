// Copyright (c) 2019 Kata Containers community
// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Result};
use std::path::Path;
use toml_edit::{DocumentMut, Item, Value};

/// Parse a TOML path into components, respecting quoted keys
/// Example: `.plugins."io.containerd.cri.v1.runtime".containerd.runtimes."kata"`
/// Results in: ["plugins", "io.containerd.cri.v1.runtime", "containerd", "runtimes", "kata"]
fn parse_toml_path(path: &str) -> Result<Vec<String>> {
    let path = path.trim_start_matches('.');
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = path.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                // Don't include the quote characters themselves
            }
            '.' if !in_quotes => {
                if !current.is_empty() {
                    parts.push(current);
                    current = String::new();
                }
            }
            _ => {
                current.push(ch);
            }
        }
    }

    if !current.is_empty() {
        parts.push(current);
    }

    if in_quotes {
        return Err(anyhow::anyhow!("Unmatched quotes in TOML path: {path}"));
    }

    if parts.is_empty() {
        return Err(anyhow::anyhow!("Invalid TOML path: {path}"));
    }

    Ok(parts)
}

/// Set a TOML value at a given path (e.g., ".plugins.cri.containerd.runtimes.kata.runtime_type")
pub fn set_toml_value(file_path: &Path, path: &str, value: &str) -> Result<()> {
    let content = std::fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read TOML file: {file_path:?}"))?;

    let mut doc = content
        .parse::<DocumentMut>()
        .context("Failed to parse TOML")?;

    let parts = parse_toml_path(path)?;

    let mut current_table = doc.as_table_mut();
    for (i, part) in parts.iter().enumerate() {
        let is_last = i == parts.len() - 1;

        if is_last {
            // Set the value
            let value_item = parse_toml_value(value);
            current_table[part.as_str()] = value_item;
        } else {
            // Navigate/create intermediate tables
            if !current_table.contains_key(part.as_str()) {
                let mut new_table = toml_edit::Table::new();
                new_table.set_implicit(true); // Make intermediate tables implicit
                current_table.insert(part, Item::Table(new_table));
            }
            current_table = current_table
                .get_mut(part.as_str())
                .and_then(|item| item.as_table_mut())
                .ok_or_else(|| anyhow::anyhow!("Path component '{part}' is not a table"))?;
        }
    }

    std::fs::write(file_path, doc.to_string())
        .with_context(|| format!("Failed to write TOML file: {file_path:?}"))?;

    Ok(())
}

/// Get a TOML value at a given path
pub fn get_toml_value(file_path: &Path, path: &str) -> Result<String> {
    let content = std::fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read TOML file: {file_path:?}"))?;

    let doc = content
        .parse::<DocumentMut>()
        .context("Failed to parse TOML")?;

    let parts = parse_toml_path(path)?;

    // Navigate through the document table
    let table = doc.as_table();
    let mut current_item: Option<&Item> = None;
    let mut current_table = table;

    for (i, part) in parts.iter().enumerate() {
        let is_last = i == parts.len() - 1;

        if let Some(item) = current_table.get(part.as_str()) {
            if is_last {
                current_item = Some(item);
                break;
            } else {
                // Handle both Table and inline table (Value::InlineTable)
                match item {
                    Item::Table(t) => {
                        current_table = t;
                    }
                    Item::Value(toml_edit::Value::InlineTable(_t)) => {
                        // Inline tables are not fully supported for navigation
                        // The test has been updated to use proper table structure
                        return Err(anyhow::anyhow!("Inline table navigation not fully supported. Use proper table structure: [plugins.cri] instead of cri = {{ ... }}"));
                    }
                    _ => {
                        return Err(anyhow::anyhow!("Path component '{part}' is not a table"));
                    }
                }
            }
        } else {
            return Err(anyhow::anyhow!("Path component '{part}' not found"));
        }
    }

    let current = current_item.ok_or_else(|| anyhow::anyhow!("Path not found"))?;

    match current {
        Item::Value(Value::String(s)) => Ok(s.value().to_string()),
        Item::Value(Value::Integer(i)) => Ok(i.value().to_string()),
        Item::Value(Value::Float(f)) => Ok(f.value().to_string()),
        Item::Value(Value::Boolean(b)) => Ok(b.value().to_string()),
        Item::Value(Value::Array(a)) => {
            let values: Vec<String> = a
                .iter()
                .map(|v| match v {
                    Value::String(s) => s.value().to_string(),
                    _ => format!("{v:?}"),
                })
                .collect();
            Ok(format!("[{}]", values.join(", ")))
        }
        _ => Err(anyhow::anyhow!(
            "Value at path '{path}' is not a simple value"
        )),
    }
}

/// Append to a TOML array
pub fn append_to_toml_array(file_path: &Path, path: &str, value: &str) -> Result<()> {
    let content = std::fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read TOML file: {file_path:?}"))?;

    let mut doc = content
        .parse::<DocumentMut>()
        .context("Failed to parse TOML")?;

    let parts = parse_toml_path(path)?;

    // Navigate to the array
    let mut current = doc.as_table_mut();
    for (i, part) in parts.iter().enumerate() {
        let is_last = i == parts.len() - 1;

        if is_last {
            // This is the array itself - use .get() to avoid panic on missing key
            let key_exists = current.get(part.as_str()).is_some();
            if !key_exists {
                current.insert(part.as_str(), Item::Value(Value::Array(toml_edit::Array::new())));
            }
            if let Some(Item::Value(Value::Array(arr))) = current.get_mut(part.as_str()) {
                let value_item = parse_toml_value(value);
                if let Item::Value(val) = value_item {
                    // Check if value already exists (idempotency)
                    // Normalize by trimming quotes for comparison
                    let value_str = val.to_string();
                    let value_normalized = value_str.trim().trim_matches('"');

                    let already_exists = arr.iter().any(|existing| {
                        let existing_str = existing.to_string();
                        let existing_normalized = existing_str.trim().trim_matches('"');
                        existing_normalized == value_normalized
                    });

                    if !already_exists {
                        arr.push(val);
                    }
                }
            } else {
                return Err(anyhow::anyhow!("Path component '{part}' is not an array"));
            }
        } else {
            // Navigate through intermediate tables - use .get() to avoid panic
            let key_exists = current.get(part.as_str()).is_some();
            if !key_exists {
                let mut new_table = toml_edit::Table::new();
                new_table.set_implicit(true); // Make intermediate tables implicit
                current.insert(part.as_str(), Item::Table(new_table));
            }
            current = current
                .get_mut(part.as_str())
                .and_then(|item| item.as_table_mut())
                .ok_or_else(|| anyhow::anyhow!("Path component '{part}' is not a table"))?;
        }
    }

    std::fs::write(file_path, doc.to_string())
        .with_context(|| format!("Failed to write TOML file: {file_path:?}"))?;

    Ok(())
}

/// Remove from a TOML array
pub fn remove_from_toml_array(file_path: &Path, path: &str, value: &str) -> Result<()> {
    let content = std::fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read TOML file: {file_path:?}"))?;

    let mut doc = content
        .parse::<DocumentMut>()
        .context("Failed to parse TOML")?;

    let parts = parse_toml_path(path)?;

    // Normalize the value to remove quotes for comparison
    let normalized_value = value.trim_matches('"');

    // Navigate to the array
    let mut current = doc.as_table_mut();
    for (i, part) in parts.iter().enumerate() {
        let is_last = i == parts.len() - 1;

        if is_last {
            // This is the array itself - remove matching values
            if let Some(Item::Value(Value::Array(arr))) = current.get_mut(part.as_str()) {
                arr.retain(|v| match v {
                    Value::String(s) => s.value() != normalized_value,
                    _ => true,
                });
            } else {
                return Err(anyhow::anyhow!("Path component '{part}' is not an array"));
            }
            break;
        } else {
            // Navigate through intermediate tables - use .get() to avoid panic
            let key_exists = current.get(part.as_str()).is_some();
            if !key_exists {
                let mut new_table = toml_edit::Table::new();
                new_table.set_implicit(true); // Make intermediate tables implicit
                current.insert(part.as_str(), Item::Table(new_table));
            }
            current = current
                .get_mut(part.as_str())
                .and_then(|item| item.as_table_mut())
                .ok_or_else(|| anyhow::anyhow!("Path component '{part}' is not a table"))?;
        }
    }

    std::fs::write(file_path, doc.to_string())
        .with_context(|| format!("Failed to write TOML file: {file_path:?}"))?;

    Ok(())
}

/// Get a TOML array value as a Vec<String>
pub fn get_toml_array(file_path: &Path, path: &str) -> Result<Vec<String>> {
    let content = std::fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read TOML file: {file_path:?}"))?;

    let doc = content
        .parse::<DocumentMut>()
        .context("Failed to parse TOML")?;

    let parts = parse_toml_path(path)?;

    let mut current_table = doc.as_table();
    for (i, part) in parts.iter().enumerate() {
        let is_last = i == parts.len() - 1;

        if let Some(item) = current_table.get(part.as_str()) {
            if is_last {
                match item {
                    Item::Value(Value::Array(arr)) => {
                        let values: Vec<String> = arr
                            .iter()
                            .map(|v| match v {
                                Value::String(s) => s.value().to_string(),
                                _ => format!("{v:?}"),
                            })
                            .collect();
                        return Ok(values);
                    }
                    _ => return Err(anyhow::anyhow!("Path '{path}' is not an array")),
                }
            } else {
                current_table = item
                    .as_table()
                    .ok_or_else(|| anyhow::anyhow!("Path component '{part}' is not a table"))?;
            }
        } else {
            return Ok(Vec::new()); // Return empty array if not found
        }
    }

    Ok(Vec::new())
}

/// Set a TOML array value
pub fn set_toml_array(file_path: &Path, path: &str, values: &[String]) -> Result<()> {
    let content = std::fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read TOML file: {file_path:?}"))?;

    let mut doc = content
        .parse::<DocumentMut>()
        .context("Failed to parse TOML")?;

    let parts = parse_toml_path(path)?;

    let mut current_table = doc.as_table_mut();
    for (i, part) in parts.iter().enumerate() {
        let is_last = i == parts.len() - 1;

        if is_last {
            let mut array = toml_edit::Array::new();
            for val in values {
                array.push(Value::String(toml_edit::Formatted::new(val.clone())));
            }
            current_table[part.as_str()] = Item::Value(Value::Array(array));
        } else {
            if !current_table.contains_key(part.as_str()) {
                let mut new_table = toml_edit::Table::new();
                new_table.set_implicit(true); // Make intermediate tables implicit
                current_table.insert(part, Item::Table(new_table));
            }
            current_table = current_table
                .get_mut(part.as_str())
                .and_then(|item| item.as_table_mut())
                .ok_or_else(|| anyhow::anyhow!("Path component '{part}' is not a table"))?;
        }
    }

    std::fs::write(file_path, doc.to_string())
        .with_context(|| format!("Failed to write TOML file: {file_path:?}"))?;

    Ok(())
}

fn parse_toml_value(value: &str) -> Item {
    use toml_edit::Formatted;

    // Try to parse as different types
    if matches!(value, "true" | "false") {
        return Item::Value(Value::Boolean(Formatted::new(value == "true")));
    }

    if let Ok(i) = value.parse::<i64>() {
        return Item::Value(Value::Integer(Formatted::new(i)));
    }

    if let Ok(f) = value.parse::<f64>() {
        return Item::Value(Value::Float(Formatted::new(f)));
    }

    // Check if it's an array
    if value.starts_with('[') && value.ends_with(']') {
        let array_str = &value[1..value.len() - 1];
        let mut array = toml_edit::Array::new();
        for item in array_str.split(',') {
            let item = item.trim().trim_matches('"');
            array.push(Value::String(Formatted::new(item.to_string())));
        }
        return Item::Value(Value::Array(array));
    }

    // Default to string
    // If the value is quoted, extract the content; otherwise use as-is
    let string_value = if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    };
    Item::Value(Value::String(Formatted::new(string_value)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_set_toml_value() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        std::fs::write(path, "[plugins]\n").unwrap();

        set_toml_value(
            path,
            ".plugins.cri.runtime_type",
            "\"io.containerd.kata.v2\"",
        )
        .unwrap();
        let content = std::fs::read_to_string(path).unwrap();
        assert!(content.contains("runtime_type"));
    }

    #[test]
    fn test_get_toml_value() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        // Use a proper table structure instead of inline table
        std::fs::write(path, "[plugins]\n[plugins.cri]\nruntime_type = \"test\"\n").unwrap();

        let value = get_toml_value(path, ".plugins.cri.runtime_type").unwrap();
        assert_eq!(value, "test");
    }

    #[test]
    fn test_append_to_toml_array() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        std::fs::write(path, "imports = []\n").unwrap();

        append_to_toml_array(path, ".imports", "\"/path/to/file\"").unwrap();
        let content = std::fs::read_to_string(path).unwrap();
        assert!(content.contains("/path/to/file"));
    }

    #[test]
    fn test_get_toml_array() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        std::fs::write(
            path,
            "[hypervisor]\n[hypervisor.qemu]\nenable_annotations = [\"annotation1\", \"annotation2\"]\n",
        )
        .unwrap();

        let values = get_toml_array(path, "hypervisor.qemu.enable_annotations").unwrap();
        assert_eq!(values.len(), 2);
        assert_eq!(values[0], "annotation1");
        assert_eq!(values[1], "annotation2");
    }

    #[test]
    fn test_get_toml_array_empty() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        std::fs::write(
            path,
            "[hypervisor]\n[hypervisor.qemu]\nenable_annotations = []\n",
        )
        .unwrap();

        let values = get_toml_array(path, "hypervisor.qemu.enable_annotations").unwrap();
        assert_eq!(values.len(), 0);
    }

    #[test]
    fn test_get_toml_array_nonexistent() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        std::fs::write(path, "[hypervisor]\n").unwrap();

        let values = get_toml_array(path, "hypervisor.qemu.enable_annotations").unwrap();
        assert_eq!(values.len(), 0);
    }

    #[test]
    fn test_set_toml_array() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        std::fs::write(path, "[hypervisor]\n[hypervisor.qemu]\n").unwrap();

        let values = vec![
            "annotation1".to_string(),
            "annotation2".to_string(),
            "annotation3".to_string(),
        ];
        set_toml_array(path, "hypervisor.qemu.enable_annotations", &values).unwrap();

        let result = get_toml_array(path, "hypervisor.qemu.enable_annotations").unwrap();
        assert_eq!(result, values);
    }

    #[test]
    fn test_set_toml_array_replace() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        std::fs::write(
            path,
            "[hypervisor]\n[hypervisor.qemu]\nenable_annotations = [\"old1\", \"old2\"]\n",
        )
        .unwrap();

        let new_values = vec!["new1".to_string(), "new2".to_string(), "new3".to_string()];
        set_toml_array(path, "hypervisor.qemu.enable_annotations", &new_values).unwrap();

        let result = get_toml_array(path, "hypervisor.qemu.enable_annotations").unwrap();
        assert_eq!(result, new_values);
    }

    #[test]
    fn test_remove_from_toml_array() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        std::fs::write(path, "imports = [\"/path1\", \"/path2\", \"/path3\"]\n").unwrap();

        remove_from_toml_array(path, ".imports", "/path2").unwrap();

        let values = get_toml_array(path, "imports").unwrap();
        assert_eq!(values.len(), 2);
        assert!(values.contains(&"/path1".to_string()));
        assert!(values.contains(&"/path3".to_string()));
        assert!(!values.contains(&"/path2".to_string()));
    }

    #[test]
    fn test_hierarchical_toml_paths() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        std::fs::write(path, "").unwrap();

        // Test setting nested values with hierarchical paths
        set_toml_value(path, "hypervisor.qemu.enable_debug", "true").unwrap();
        set_toml_value(path, "runtime.enable_debug", "true").unwrap();
        set_toml_value(path, "agent.kata.enable_debug", "false").unwrap();

        // Verify all values are set correctly
        let hypervisor_debug = get_toml_value(path, "hypervisor.qemu.enable_debug").unwrap();
        let runtime_debug = get_toml_value(path, "runtime.enable_debug").unwrap();
        let agent_debug = get_toml_value(path, "agent.kata.enable_debug").unwrap();

        assert_eq!(hypervisor_debug, "true");
        assert_eq!(runtime_debug, "true");
        assert_eq!(agent_debug, "false");
    }

    #[test]
    fn test_toml_value_types() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        std::fs::write(path, "").unwrap();

        // Test different value types
        set_toml_value(path, "test.string_value", "test_string").unwrap();
        set_toml_value(path, "test.bool_value", "true").unwrap();
        set_toml_value(path, "test.int_value", "42").unwrap();

        let string_val = get_toml_value(path, "test.string_value").unwrap();
        let bool_val = get_toml_value(path, "test.bool_value").unwrap();
        let int_val = get_toml_value(path, "test.int_value").unwrap();

        assert_eq!(string_val, "test_string");
        assert_eq!(bool_val, "true");
        assert_eq!(int_val, "42");
    }

    #[test]
    fn test_realistic_kata_config_structure() {
        // Test with actual kata configuration from runtime-rs test fixtures
        let config_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .map(|p| p.join("src/runtime-rs/tests/texture/configuration-qemu.toml"));

        if let Some(ref path) = config_path {
            if path.exists() {
                // Create a temp copy to modify
                let temp_file = NamedTempFile::new().unwrap();
                let temp_path = temp_file.path();
                std::fs::copy(path, temp_path).unwrap();

                // Test reading from actual config structure
                let machine_type = get_toml_value(temp_path, "hypervisor.qemu.machine_type");
                assert!(
                    machine_type.is_ok(),
                    "Should read machine_type from real config"
                );

                let default_vcpus = get_toml_value(temp_path, "hypervisor.qemu.default_vcpus");
                assert!(
                    default_vcpus.is_ok(),
                    "Should read default_vcpus from real config"
                );

                // Test modifying kernel_params on real config
                let current = get_toml_value(temp_path, "hypervisor.qemu.kernel_params")
                    .unwrap_or_default();
                let new_value = format!("{} agent.log=debug", current.trim_matches('"'));
                let result = set_toml_value(
                    temp_path,
                    "hypervisor.qemu.kernel_params",
                    &format!("\"{}\"", new_value),
                );
                assert!(result.is_ok(), "Should be able to set kernel_params");

                // Test enabling debug on real config
                let result = set_toml_value(temp_path, "hypervisor.qemu.enable_debug", "true");
                assert!(result.is_ok(), "Should be able to set enable_debug");
            }
        }
    }

    #[test]
    fn test_annotations_array_operations() {
        // Test realistic annotation array operations
        let file = NamedTempFile::new().unwrap();
        let path = file.path();

        std::fs::write(
            path,
            r#"[hypervisor.qemu]
enable_annotations = ["kernel", "image", "initrd"]
"#,
        )
        .unwrap();

        // Get existing annotations
        let annotations = get_toml_array(path, "hypervisor.qemu.enable_annotations").unwrap();
        assert_eq!(annotations.len(), 3);
        assert!(annotations.contains(&"kernel".to_string()));

        // Add more annotations
        let mut all_annotations = annotations;
        all_annotations.push("kernel_params".to_string());
        all_annotations.push("firmware".to_string());
        all_annotations.sort();
        all_annotations.dedup();

        set_toml_array(path, "hypervisor.qemu.enable_annotations", &all_annotations).unwrap();

        let updated = get_toml_array(path, "hypervisor.qemu.enable_annotations").unwrap();
        assert_eq!(updated.len(), 5);
        assert!(updated.contains(&"kernel_params".to_string()));
        assert!(updated.contains(&"firmware".to_string()));
    }

    #[test]
    fn test_multiple_hypervisor_sections() {
        // Test config with multiple hypervisor types (like in tests)
        let file = NamedTempFile::new().unwrap();
        let path = file.path();

        let config = r#"
[hypervisor.qemu]
path = "/usr/bin/qemu-system-x86_64"
enable_debug = false

[hypervisor.clh]
path = "/usr/bin/cloud-hypervisor"
enable_debug = true

[runtime]
enable_debug = false
"#;
        std::fs::write(path, config).unwrap();

        // Test accessing different hypervisor sections
        assert_eq!(
            get_toml_value(path, "hypervisor.qemu.enable_debug").unwrap(),
            "false"
        );
        assert_eq!(
            get_toml_value(path, "hypervisor.clh.enable_debug").unwrap(),
            "true"
        );

        // Modify one without affecting the other
        set_toml_value(path, "hypervisor.qemu.enable_debug", "true").unwrap();
        assert_eq!(
            get_toml_value(path, "hypervisor.qemu.enable_debug").unwrap(),
            "true"
        );
        assert_eq!(
            get_toml_value(path, "hypervisor.clh.enable_debug").unwrap(),
            "true"
        );
    }

    #[test]
    fn test_proxy_configuration() {
        // Test realistic proxy configuration scenario
        let file = NamedTempFile::new().unwrap();
        let path = file.path();

        std::fs::write(
            path,
            r#"[hypervisor.qemu]
kernel_params = "console=hvc0"
"#,
        )
        .unwrap();

        // Set kernel_params with proxy settings
        set_toml_value(
            path,
            "hypervisor.qemu.kernel_params",
            "\"console=hvc0 agent.https_proxy=http://proxy.example.com:8080 agent.no_proxy=localhost,127.0.0.1\"",
        )
        .unwrap();

        let content = std::fs::read_to_string(path).unwrap();
        assert!(content.contains("agent.https_proxy=http://proxy.example.com:8080"));
        assert!(content.contains("agent.no_proxy=localhost,127.0.0.1"));
    }

    #[test]
    fn test_runtime_rs_cloud_hypervisor_config() {
        // Test with actual cloud-hypervisor config from runtime-rs
        let config_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .map(|p| p.join("src/runtime-rs/config/configuration-cloud-hypervisor.toml.in"));

        if let Some(ref path) = config_path {
            if path.exists() {
                // Read the template
                let content = std::fs::read_to_string(path).unwrap();

                // Replace template variables with test values
                let content = content
                    .replace("@CLHPATH@", "/opt/kata/bin/cloud-hypervisor")
                    .replace(
                        "@KERNELPATH_CLH@",
                        "/opt/kata/share/kata-containers/vmlinux.container",
                    )
                    .replace(
                        "@IMAGEPATH@",
                        "/opt/kata/share/kata-containers/kata-containers.img",
                    )
                    .replace("@DEFROOTFSTYPE@", "\"ext4\"")
                    .replace("@VMROOTFSDRIVER_CLH@", "virtio-blk")
                    .replace("@FIRMWAREPATH@", "")
                    .replace("@DEFENABLEANNOTATIONS@", "[]")
                    .replace("@CLHVALIDHYPERVISORPATHS@", "[]")
                    .replace("@KERNELPARAMS@", "")
                    .replace("@DEFVCPUS@", "1")
                    .replace("@DEFMAXVCPUS@", "0")
                    .replace("@DEFMEMSZ@", "2048")
                    .replace("@DEFBRIDGES@", "1")
                    .replace("@DEFNETWORKMODEL_CLH@", "tcfilter")
                    .replace("@DEFDISABLEGUESTSECCOMP@", "true")
                    .replace("@DEFAULTEXPFEATURES@", "[]")
                    .replace("@DEFVIRTIOFSCACHE@", "auto")
                    .replace("@DEFVIRTIOFSCACHESIZE@", "0")
                    .replace("@DEFVIRTIOFSEXTRAARGS@", "[]")
                    .replace("@DEFVIRTIOFSDAEMON@", "virtiofsd")
                    .replace("@DEFSHAREDFS_CLH_VIRTIOFS@", "virtio-fs")
                    .replace("@HYPERVISOR_CLH@", "cloud-hypervisor")
                    .replace("@PROJECT_NAME@", "kata-containers")
                    .replace("@PROJECT_TYPE@", "kata")
                    .replace("@RUNTIMENAME@", "kata-runtime")
                    .replace("@DEFSTATICRESOURCEMGMT_CLH@", "false")
                    .replace("@DEFSANDBOXCGROUPONLY_CLH@", "false")
                    .replace("@DEFCREATECONTAINERTIMEOUT@", "60")
                    .replace("@DEFBINDMOUNTS@", "[]")
                    .replace("@DEFDANCONF@", "")
                    .replace("@PIPESIZE@", "0");

                // Replace any remaining @...@ placeholders with 0 (for numeric fields)
                let content = regex::Regex::new(r"=\s*@[A-Z_0-9]+@")
                    .unwrap()
                    .replace_all(&content, "= 0")
                    .to_string();

                // Create temp file with resolved config
                let temp_file = NamedTempFile::new().unwrap();
                let temp_path = temp_file.path();
                std::fs::write(temp_path, content).unwrap();

                // Verify cloud-hypervisor specific fields exist
                let vm_rootfs =
                    get_toml_value(temp_path, "hypervisor.cloud-hypervisor.vm_rootfs_driver");
                assert!(
                    vm_rootfs.is_ok(),
                    "Should have vm_rootfs_driver field: {:?}",
                    vm_rootfs.err()
                );

                // Test modifying cloud-hypervisor config
                let result = set_toml_value(
                    temp_path,
                    "hypervisor.cloud-hypervisor.enable_debug",
                    "true",
                );
                assert!(
                    result.is_ok(),
                    "Should be able to set enable_debug on cloud-hypervisor config"
                );
            }
        }
    }

    #[test]
    fn test_runtime_rs_dragonball_config() {
        // Test with actual dragonball config from runtime-rs
        let config_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .map(|p| p.join("src/runtime-rs/config/configuration-dragonball.toml.in"));

        if let Some(ref path) = config_path {
            if path.exists() {
                // Read and prepare template
                let content = std::fs::read_to_string(path).unwrap();
                let content = content
                    .replace("@DBPATH@", "/opt/kata/runtime-rs/bin/dragonball")
                    .replace(
                        "@KERNELPATH_DB@",
                        "/opt/kata/share/kata-containers/vmlinux.container",
                    )
                    .replace(
                        "@IMAGEPATH@",
                        "/opt/kata/share/kata-containers/kata-containers.img",
                    )
                    .replace("@DEFROOTFSTYPE@", "\"ext4\"")
                    .replace("@KERNELPARAMS_DB@", "")
                    .replace("@DEFVCPUS@", "1")
                    .replace("@DEFMAXVCPUS_DB@", "0")
                    .replace("@DEFMEMSZ@", "2048")
                    .replace("@DEFMAXMEMSZ@", "2048")
                    .replace("@DEFENABLEANNOTATIONS@", "[]")
                    .replace("@DBVALIDHYPERVISORPATHS@", "[]")
                    .replace("@DEFBRIDGES@", "1")
                    .replace("@DEFNETWORKMODEL_DB@", "tcfilter")
                    .replace("@VMROOTFSDRIVER_DB@", "virtio-blk")
                    .replace("@DEFBLOCKSTORAGEDRIVER_DB@", "virtio-blk")
                    .replace("@DEFDISABLEGUESTSECCOMP@", "true")
                    .replace("@DEFVIRTIOFSCACHE@", "auto")
                    .replace("@DEFVIRTIOFSCACHESIZE@", "0")
                    .replace("@DEFVIRTIOFSEXTRAARGS@", "[]")
                    .replace("@FIRMWAREPATH@", "")
                    .replace("@DBSHAREDFS@", "virtio-fs")
                    .replace("@DBCTLPATH@", "")
                    .replace("@HYPERVISOR_DB@", "dragonball")
                    .replace("@PROJECT_NAME@", "kata-containers")
                    .replace("@PROJECT_TYPE@", "kata")
                    .replace("@RUNTIMENAME@", "kata-runtime")
                    .replace("@DEFSTATICRESOURCEMGMT_DB@", "false")
                    .replace("@DEFSANDBOXCGROUPONLY_DB@", "false")
                    .replace("@DEFCREATECONTAINERTIMEOUT@", "60")
                    .replace("@DEFBINDMOUNTS@", "[]")
                    .replace("@DEFDANCONF@", "")
                    .replace("@DEFAULTEXPFEATURES@", "[]")
                    .replace("@PIPESIZE@", "0");

                // Replace any remaining @...@ placeholders with 0 (for numeric fields)
                let content = regex::Regex::new(r"=\s*@[A-Z_0-9]+@")
                    .unwrap()
                    .replace_all(&content, "= 0")
                    .to_string();

                let temp_file = NamedTempFile::new().unwrap();
                let temp_path = temp_file.path();
                std::fs::write(temp_path, content).unwrap();

                // Test hierarchical path access works
                let rootfs = get_toml_value(temp_path, "hypervisor.dragonball.rootfs_type");
                assert!(
                    rootfs.is_ok(),
                    "Should read rootfs_type from dragonball config: {:?}",
                    rootfs.err()
                );

                // Test setting kernel parameters
                let current = get_toml_value(temp_path, "hypervisor.dragonball.kernel_params")
                    .unwrap_or_default();
                let new_value = format!("{} agent.log=debug", current.trim_matches('"'));
                let result = set_toml_value(
                    temp_path,
                    "hypervisor.dragonball.kernel_params",
                    &format!("\"{}\"", new_value),
                );
                assert!(result.is_ok(), "Should set kernel_params");
            }
        }
    }

    #[test]
    fn test_go_vs_rust_runtime_configs() {
        // Test that both Go and Rust runtime configs work with hierarchical paths using actual configs
        let base_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .and_then(|p| p.parent());

        if let Some(base) = base_path {
            // Go runtime config
            let go_config = base.join("src/runtime/config/configuration-qemu.toml.in");
            // Rust runtime config
            let rust_config =
                base.join("src/runtime-rs/config/configuration-cloud-hypervisor.toml.in");

            if go_config.exists() && rust_config.exists() {
                // Create temp copies
                let go_temp = NamedTempFile::new().unwrap();
                let rust_temp = NamedTempFile::new().unwrap();

                // Prepare Go config
                let go_content = std::fs::read_to_string(&go_config)
                    .unwrap()
                    .replace("@QEMUPATH@", "/opt/kata/bin/qemu-system-x86_64")
                    .replace(
                        "@KERNELPATH@",
                        "/opt/kata/share/kata-containers/vmlinuz.container",
                    )
                    .replace(
                        "@IMAGEPATH@",
                        "/opt/kata/share/kata-containers/kata-containers.img",
                    )
                    .replace("@MACHINETYPE@", "q35")
                    .replace("@DEFROOTFSTYPE@", "\"ext4\"")
                    .replace("@DEFENABLEANNOTATIONS@", "[]")
                    .replace("@QEMUVALIDHYPERVISORPATHS@", "[]")
                    .replace("@KERNELPARAMS@", "")
                    .replace("@FIRMWAREPATH@", "")
                    .replace("@FIRMWAREVOLUMEPATH@", "")
                    .replace("@MACHINEACCELERATORS@", "")
                    .replace("@DEFSECCOMPSANDBOXPARAM@", "")
                    .replace("@CPUFEATURES@", "")
                    .replace("@DEFVCPUS@", "1")
                    .replace("@DEFMAXVCPUS@", "0")
                    .replace("@DEFMEMSZ@", "2048")
                    .replace("@DEFMAXMEMSZ@", "0")
                    .replace("@DEFMEMSLOTS@", "10")
                    .replace("@DEFBRIDGES@", "1")
                    .replace("@DEFNETWORKMODEL_QEMU@", "tcfilter")
                    .replace("@DEFDISABLEGUESTSECCOMP@", "true")
                    .replace("@DEFDISABLEGUESTEMPTYDIR@", "false")
                    .replace("@DEFDISABLEGUESTSELINUX@", "false")
                    .replace("@DEFDISABLESELINUX@", "false")
                    .replace("@DEFDISABLEBLOCK@", "false")
                    .replace("@DEFDISABLEIMAGENVDIMM@", "false")
                    .replace("@DEFENABLEIOTHREADS@", "false")
                    .replace("@DEFENABLEVHOSTUSERSTORE@", "false")
                    .replace("@DEFENTROPYSOURCE@", "/dev/urandom")
                    .replace("@DEFFILEMEMBACKEND@", "")
                    .replace("@DEFVALIDFILEMEMBACKENDS@", "[]")
                    .replace("@DEFBLOCKSTORAGEDRIVER_QEMU@", "virtio-blk")
                    .replace("@DEFBLOCKDEVICEAIO_QEMU@", "io_uring")
                    .replace("@DEFAULTEXPFEATURES@", "[]")
                    .replace("@DEFBINDMOUNTS@", "[]")
                    .replace("@DEFDANCONF@", "")
                    .replace("@DEFCREATECONTAINERTIMEOUT@", "60")
                    .replace("@PIPESIZE@", "0")
                    .replace("@PROJECT_NAME@", "kata-containers")
                    .replace("@PROJECT_TYPE@", "kata")
                    .replace("@RUNTIMENAME@", "kata-runtime")
                    .replace("@HYPERVISOR_QEMU@", "qemu")
                    .replace("@DEFSHAREDFS_QEMU_VIRTIOFS@", "virtio-fs")
                    .replace("@DEFVIRTIOFSDAEMON@", "virtiofsd")
                    .replace("@DEFVIRTIOFSCACHE@", "auto")
                    .replace("@DEFVIRTIOFSCACHESIZE@", "0")
                    .replace("@DEFVIRTIOFSEXTRAARGS@", "[]")
                    .replace("@DEFSANDBOXCGROUPONLY_QEMU@", "false")
                    .replace("@DEFSTATICRESOURCEMGMT_QEMU@", "false")
                    .replace("@VMROOTFSDRIVER_QEMU@", "virtio-blk")
                    .replace("@DEFVALIDVIRTIOFSDAEMONPATHS@", "[]")
                    .replace("@DEFMSIZE9P@", "8192");

                // Replace any remaining @...@ placeholders with 0 (for numeric fields)
                let go_content = regex::Regex::new(r"=\s*@[A-Z_0-9]+@")
                    .unwrap()
                    .replace_all(&go_content, "= 0")
                    .to_string();

                // Replace any @...@ placeholders in table headers (e.g., [agent.@PROJECT_TYPE@])
                let go_content = regex::Regex::new(r"\[@[A-Z_0-9]+@\]")
                    .unwrap()
                    .replace_all(&go_content, "[placeholder]")
                    .to_string();

                // Also replace placeholders inside table names (e.g., [agent.@PROJECT_TYPE@])
                let go_content = regex::Regex::new(r"\.@[A-Z_0-9]+@")
                    .unwrap()
                    .replace_all(&go_content, ".placeholder")
                    .to_string();

                std::fs::write(go_temp.path(), go_content).unwrap();

                // Prepare Rust config
                let rust_content = std::fs::read_to_string(&rust_config)
                    .unwrap()
                    .replace("@CLHPATH@", "/opt/kata/runtime-rs/bin/cloud-hypervisor")
                    .replace(
                        "@KERNELPATH_CLH@",
                        "/opt/kata/share/kata-containers/vmlinux.container",
                    )
                    .replace(
                        "@IMAGEPATH@",
                        "/opt/kata/share/kata-containers/kata-containers.img",
                    )
                    .replace("@DEFROOTFSTYPE@", "\"ext4\"")
                    .replace("@VMROOTFSDRIVER_CLH@", "virtio-blk")
                    .replace("@FIRMWAREPATH@", "")
                    .replace("@DEFENABLEANNOTATIONS@", "[]")
                    .replace("@CLHVALIDHYPERVISORPATHS@", "[]")
                    .replace("@KERNELPARAMS@", "")
                    .replace("@DEFVCPUS@", "1");

                // Replace any remaining @...@ placeholders with 0 (for numeric fields)
                let rust_content = regex::Regex::new(r"=\s*@[A-Z_0-9]+@")
                    .unwrap()
                    .replace_all(&rust_content, "= 0")
                    .to_string();

                // Replace any @...@ placeholders in table headers (e.g., [agent.@PROJECT_TYPE@])
                let rust_content = regex::Regex::new(r"\[@[A-Z_0-9]+@\]")
                    .unwrap()
                    .replace_all(&rust_content, "[placeholder]")
                    .to_string();

                // Also replace placeholders inside table names (e.g., [agent.@PROJECT_TYPE@])
                let rust_content = regex::Regex::new(r"\.@[A-Z_0-9]+@")
                    .unwrap()
                    .replace_all(&rust_content, ".placeholder")
                    .to_string();

                std::fs::write(rust_temp.path(), rust_content).unwrap();

                // Both should support the same hierarchical path operations
                // enable_debug is in the [runtime] section for both configs
                let go_result = set_toml_value(go_temp.path(), "runtime.enable_debug", "true");
                let rust_result = set_toml_value(rust_temp.path(), "runtime.enable_debug", "true");

                assert!(
                    go_result.is_ok(),
                    "Should set enable_debug on Go runtime config: {:?}",
                    go_result.err()
                );
                assert!(
                    rust_result.is_ok(),
                    "Should set enable_debug on Rust runtime config: {:?}",
                    rust_result.err()
                );

                // Verify values were set
                assert_eq!(
                    get_toml_value(go_temp.path(), "runtime.enable_debug").unwrap(),
                    "true"
                );
                assert_eq!(
                    get_toml_value(rust_temp.path(), "runtime.enable_debug").unwrap(),
                    "true"
                );
            }
        }
    }

    #[test]
    fn test_get_toml_value_nonexistent_file() {
        let result = get_toml_value(Path::new("/nonexistent/file.toml"), "some.path");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to read TOML file"));
    }

    #[test]
    fn test_get_toml_value_invalid_toml() {
        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path();

        // Write invalid TOML
        std::fs::write(temp_path, "this is not [ valid toml {").unwrap();

        let result = get_toml_value(temp_path, "some.path");
        assert!(result.is_err(), "Should fail parsing invalid TOML");
        // Just verify it's an error, don't check specific message
    }

    #[test]
    fn test_get_toml_value_nonexistent_path() {
        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path();

        // Write valid TOML
        std::fs::write(temp_path, "[section]\nkey = \"value\"").unwrap();

        let result = get_toml_value(temp_path, "nonexistent.path");
        assert!(result.is_err(), "Should fail for nonexistent path");
    }

    #[test]
    fn test_set_toml_value_nonexistent_file() {
        let result = set_toml_value(
            Path::new("/nonexistent/file.toml"),
            "some.path",
            "\"value\"",
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_set_toml_value_invalid_toml() {
        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path();

        // Write invalid TOML
        std::fs::write(temp_path, "this is not [ valid toml {").unwrap();

        let result = set_toml_value(temp_path, "some.path", "\"value\"");
        assert!(result.is_err(), "Should fail parsing invalid TOML");
    }

    #[test]
    fn test_append_to_toml_array_nonexistent_file() {
        let result = append_to_toml_array(
            Path::new("/nonexistent/file.toml"),
            "some.array",
            "\"value\"",
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_append_to_toml_array_not_an_array() {
        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path();

        // Write TOML with a string, not an array
        std::fs::write(temp_path, "[section]\nkey = \"value\"").unwrap();

        let result = append_to_toml_array(temp_path, "section.key", "\"item\"");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not an array"));
    }

    #[test]
    fn test_get_toml_array_not_an_array() {
        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path();

        // Write TOML with a string, not an array
        std::fs::write(temp_path, "[section]\nkey = \"value\"").unwrap();

        let result = get_toml_array(temp_path, "section.key");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not an array"));
    }

    #[test]
    fn test_set_toml_value_idempotent() {
        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path();

        // Initial TOML
        std::fs::write(temp_path, "[hypervisor.qemu]\npath = \"/usr/bin/qemu\"\n").unwrap();

        // Set value first time
        set_toml_value(temp_path, "hypervisor.qemu.path", "\"/opt/kata/bin/qemu\"").unwrap();
        let value1 = get_toml_value(temp_path, "hypervisor.qemu.path").unwrap();

        // Set same value second time
        set_toml_value(temp_path, "hypervisor.qemu.path", "\"/opt/kata/bin/qemu\"").unwrap();
        let value2 = get_toml_value(temp_path, "hypervisor.qemu.path").unwrap();

        // Should be identical
        assert_eq!(value1, value2, "set_toml_value must be idempotent");
        // get_toml_value returns the value without TOML formatting quotes
        assert_eq!(value1, "/opt/kata/bin/qemu");
    }

    #[test]
    fn test_set_toml_array_idempotent() {
        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path();

        // Initial TOML
        std::fs::write(temp_path, "[hypervisor.qemu]\nvalid_annotations = []\n").unwrap();

        let values = vec!["annotation1".to_string(), "annotation2".to_string()];

        // Set array first time
        set_toml_array(temp_path, "hypervisor.qemu.valid_annotations", &values).unwrap();
        let array1 = get_toml_array(temp_path, "hypervisor.qemu.valid_annotations").unwrap();

        // Set same array second time
        set_toml_array(temp_path, "hypervisor.qemu.valid_annotations", &values).unwrap();
        let array2 = get_toml_array(temp_path, "hypervisor.qemu.valid_annotations").unwrap();

        // Should be identical
        assert_eq!(array1, array2, "set_toml_array must be idempotent");
        assert_eq!(array1.len(), 2);
    }

    #[test]
    fn test_append_to_toml_array_prevents_duplicates() {
        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path();

        // Initial TOML with empty array
        std::fs::write(
            temp_path,
            "[hypervisor.qemu]\nvalid_annotations = [\"ann1\"]\n",
        )
        .unwrap();

        // Append first time
        append_to_toml_array(temp_path, "hypervisor.qemu.valid_annotations", "\"ann2\"").unwrap();
        let array1 = get_toml_array(temp_path, "hypervisor.qemu.valid_annotations").unwrap();

        // Append existing value - should not duplicate
        append_to_toml_array(temp_path, "hypervisor.qemu.valid_annotations", "\"ann2\"").unwrap();
        let array2 = get_toml_array(temp_path, "hypervisor.qemu.valid_annotations").unwrap();

        // Should be identical (no duplication)
        assert_eq!(
            array1, array2,
            "append_to_toml_array must prevent duplicates"
        );
        assert_eq!(array1.len(), 2);
        assert!(array1.contains(&"ann1".to_string()));
        assert!(array1.contains(&"ann2".to_string()));
    }

    /// Test for Mariner issue: append to non-existent imports array
    /// On CBL Mariner, /etc/containerd/config.toml might not have an imports array,
    /// and we need to create it when appending the kata-deploy drop-in path.
    /// This previously caused a panic: "index not found" at toml.rs:177
    #[test]
    fn test_append_to_imports_array_when_missing() {
        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path();

        // Simulate a containerd config without an imports array (like on Mariner)
        // This is a minimal containerd config that does NOT have the imports field
        let mariner_config = r#"version = 2

[plugins]
  [plugins."io.containerd.grpc.v1.cri"]
    sandbox_image = "mcr.microsoft.com/oss/kubernetes/pause:3.6"
"#;
        std::fs::write(temp_path, mariner_config).unwrap();

        // This used to panic with "index not found" because the imports array didn't exist
        let drop_in_path = "/opt/kata/containerd/config.d/kata-deploy.toml";
        let result = append_to_toml_array(temp_path, ".imports", &format!("\"{}\"", drop_in_path));

        // Should succeed, not panic
        assert!(
            result.is_ok(),
            "append_to_toml_array should create imports array if missing: {:?}",
            result
        );

        // Verify the imports array was created with the correct value
        let imports = get_toml_array(temp_path, ".imports").unwrap();
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0], drop_in_path);

        // Verify the rest of the config is preserved
        let content = std::fs::read_to_string(temp_path).unwrap();
        assert!(content.contains("version = 2"));
        assert!(content.contains("sandbox_image"));
    }

    #[test]
    fn test_annotations_array_operations_idempotent() {
        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path();

        // Create realistic TOML config
        let toml_content = r#"
[hypervisor.qemu]
path = "/opt/kata/bin/qemu"
valid_annotations = ["io.katacontainers.config.hypervisor.default_vcpus"]
"#;
        std::fs::write(temp_path, toml_content).unwrap();

        let new_annotations = vec![
            "tdx.intel.com/keys".to_string(),
            "io.katacontainers.config.hypervisor.default_memory".to_string(),
        ];

        // First append
        for annotation in &new_annotations {
            append_to_toml_array(
                temp_path,
                "hypervisor.qemu.valid_annotations",
                &format!("\"{}\"", annotation),
            )
            .unwrap();
        }
        let array1 = get_toml_array(temp_path, "hypervisor.qemu.valid_annotations").unwrap();

        // Second append (same values) - should be idempotent
        for annotation in &new_annotations {
            append_to_toml_array(
                temp_path,
                "hypervisor.qemu.valid_annotations",
                &format!("\"{}\"", annotation),
            )
            .unwrap();
        }
        let array2 = get_toml_array(temp_path, "hypervisor.qemu.valid_annotations").unwrap();

        // Arrays should be identical
        assert_eq!(array1, array2, "Annotation operations must be idempotent");
        assert_eq!(array1.len(), 3); // Original + 2 new ones
    }

    #[test]
    fn test_imports_array_no_duplicates() {
        // Test that simulates the containerd imports array behavior
        // This ensures we never get duplicate imports like:
        // imports = ['/etc/containerd/conf.d/*.toml', '"/opt/kata/containerd/config.d/kata-deploy.toml"', "/opt/kata/containerd/config.d/kata-deploy.toml"]

        let temp_dir = tempfile::tempdir().unwrap();
        let temp_path = temp_dir.path().join("config.toml");

        // Start with a config that has an existing import
        let initial_content = r#"
version = 2

imports = ["/etc/containerd/conf.d/*.toml"]
"#;
        std::fs::write(&temp_path, initial_content).unwrap();

        // Simulate adding the kata-deploy drop-in path multiple times
        // (as might happen if configure_containerd is called multiple times)
        let kata_drop_in = "/opt/kata/containerd/config.d/kata-deploy.toml";

        // First add - with quotes (as done in the code)
        append_to_toml_array(&temp_path, "imports", &format!("\"{}\"", kata_drop_in)).unwrap();
        let imports1 = get_toml_array(&temp_path, "imports").unwrap();
        assert_eq!(imports1.len(), 2, "Should have 2 imports after first add");
        assert!(imports1.contains(&kata_drop_in.to_string()));

        // Second add - should NOT create duplicate
        append_to_toml_array(&temp_path, "imports", &format!("\"{}\"", kata_drop_in)).unwrap();
        let imports2 = get_toml_array(&temp_path, "imports").unwrap();
        assert_eq!(
            imports2.len(),
            2,
            "Should still have 2 imports (no duplicate)"
        );
        assert_eq!(imports1, imports2, "Arrays should be identical");

        // Third add - without extra quotes
        append_to_toml_array(&temp_path, "imports", kata_drop_in).unwrap();
        let imports3 = get_toml_array(&temp_path, "imports").unwrap();
        assert_eq!(
            imports3.len(),
            2,
            "Should still have 2 imports (no duplicate even without quotes)"
        );
        assert_eq!(imports1, imports3, "Arrays should be identical");

        // Verify the exact content
        let final_content = std::fs::read_to_string(&temp_path).unwrap();
        let occurrences = final_content.matches("kata-deploy").count();
        assert_eq!(
            occurrences, 1,
            "kata-deploy should appear exactly once in the file"
        );

        // Verify no double-quoted strings
        assert!(
            !final_content.contains(r#"'"/opt/kata"#),
            "Should not have double quotes like '\"/opt/kata'"
        );
        assert!(
            !final_content.contains(r#"\""/opt/kata"#),
            "Should not have escaped double quotes"
        );
    }

    #[test]
    fn test_remove_from_toml_array_with_quotes() {
        // Test that removing from arrays works correctly with quoted values
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let temp_path = temp_file.path();

        // Initial TOML with imports array
        let initial_content = r#"
version = 2

imports = ["/etc/containerd/conf.d/*.toml", "/opt/kata/containerd/config.d/kata-deploy.toml"]
"#;
        std::fs::write(temp_path, initial_content).unwrap();

        // Verify initial state
        let imports_before = get_toml_array(temp_path, "imports").unwrap();
        assert_eq!(imports_before.len(), 2);
        assert!(
            imports_before.contains(&"/opt/kata/containerd/config.d/kata-deploy.toml".to_string())
        );

        // Remove the kata-deploy import (with quotes, as done in cleanup)
        remove_from_toml_array(
            temp_path,
            ".imports",
            "\"/opt/kata/containerd/config.d/kata-deploy.toml\"",
        )
        .unwrap();

        // Verify it was removed
        let imports_after = get_toml_array(temp_path, "imports").unwrap();
        assert_eq!(
            imports_after.len(),
            1,
            "Should have 1 import remaining after removal"
        );
        assert!(
            !imports_after.contains(&"/opt/kata/containerd/config.d/kata-deploy.toml".to_string())
        );
        assert_eq!(imports_after[0], "/etc/containerd/conf.d/*.toml");

        // Test removing with value without quotes should also work
        std::fs::write(temp_path, initial_content).unwrap();
        remove_from_toml_array(
            temp_path,
            ".imports",
            "/opt/kata/containerd/config.d/kata-deploy.toml",
        )
        .unwrap();

        let imports_after2 = get_toml_array(temp_path, "imports").unwrap();
        assert_eq!(imports_after2.len(), 1, "Should work without quotes too");
    }

    #[test]
    fn test_quoted_plugin_id_paths() {
        // Test that paths with quoted keys (like containerd v2/v3 plugin IDs) work correctly
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let temp_path = temp_file.path();

        // Initial empty TOML
        std::fs::write(temp_path, "").unwrap();

        // Test v3 config format (version = 3)
        // Path: .plugins."io.containerd.cri.v1.runtime".containerd.runtimes.kata-qemu.runtime_type
        set_toml_value(
            temp_path,
            ".plugins.\"io.containerd.cri.v1.runtime\".containerd.runtimes.kata-qemu.runtime_type",
            "\"io.containerd.kata-qemu.v2\"",
        )
        .unwrap();

        let content = std::fs::read_to_string(temp_path).unwrap();
        assert!(
            content.contains(
                "[plugins.\"io.containerd.cri.v1.runtime\".containerd.runtimes.kata-qemu]"
            ),
            "Should create correct section header with quoted plugin ID"
        );
        assert!(
            content.contains("runtime_type = \"io.containerd.kata-qemu.v2\""),
            "Should set runtime_type value"
        );

        // Test v2 config format (version = 2)
        let temp_file2 = tempfile::NamedTempFile::new().unwrap();
        let temp_path2 = temp_file2.path();
        std::fs::write(temp_path2, "").unwrap();

        set_toml_value(
            temp_path2,
            ".plugins.\"io.containerd.grpc.v1.cri\".containerd.runtimes.kata-fc.runtime_type",
            "\"io.containerd.kata-fc.v2\"",
        )
        .unwrap();

        let content2 = std::fs::read_to_string(temp_path2).unwrap();
        assert!(
            content2
                .contains("[plugins.\"io.containerd.grpc.v1.cri\".containerd.runtimes.kata-fc]"),
            "Should create correct section header with v2 quoted plugin ID"
        );

        // Test reading back values
        let runtime_type = get_toml_value(
            temp_path,
            ".plugins.\"io.containerd.cri.v1.runtime\".containerd.runtimes.kata-qemu.runtime_type",
        )
        .unwrap();
        assert_eq!(runtime_type, "io.containerd.kata-qemu.v2");
    }
}
