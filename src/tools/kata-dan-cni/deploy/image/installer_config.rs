// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

//! Render the project's CNI template with node-specific DAN settings.

use serde_json::Value;
use std::path::Path;

pub fn render_installer_config(
    template: &[u8],
    node_name: &str,
    runtime_class: &str,
    dan_conf_dir: &str,
) -> Result<Value, String> {
    let node_settings = [
        ("nodeName", node_name),
        ("danRuntimeClass", runtime_class),
        ("danConfDir", dan_conf_dir),
    ];
    for (field, value) in node_settings {
        if value.is_empty() {
            return Err(format!("{field} is required"));
        }
    }
    // Kata and CNI must resolve DAN files from the same absolute directory.
    if !Path::new(dan_conf_dir).is_absolute() {
        return Err("danConfDir must be an absolute path".into());
    }
    let mut rendered: Value = serde_json::from_slice(template)
        .map_err(|error| format!("invalid Kata DAN conflist template: {error}"))?;
    let wrapper = rendered
        .pointer_mut("/plugins/0")
        .and_then(Value::as_object_mut)
        .ok_or("Kata DAN template must contain a plugin object at plugins[0]")?;
    for (field, value) in node_settings {
        wrapper.insert(field.into(), value.into());
    }
    Ok(rendered)
}
