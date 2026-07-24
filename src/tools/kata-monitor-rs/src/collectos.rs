// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tokio::task::JoinSet;

use crate::config::RuntimeConfig;
use crate::cache::SandboxCache;
use crate::client::ShimClient;

pub struct MetricsCollector {
    runtime_config: RuntimeConfig,
    sandbox_cache: Arc<SandboxCache>,
    timeout: Duration,
}

impl MetricsCollector {
    pub fn new(
        runtime_config: RuntimeConfig,
        sandbox_cache: Arc<SandboxCache>,
        timeout: Duration,
    ) -> Self {
        Self {
            runtime_config,
            sandbox_cache,
            timeout,
        }
    }

    pub async fn collect_all(&self) -> Result<String> {
        let sandboxes = self.sandbox_cache.get_all().await;
        if sandboxes.is_empty() {
            return Ok(String::new());
        }

        let mut join_set = JoinSet::new();

        for sandbox_id in sandboxes {
            let socket_path = self.runtime_config.socket_path(&sandbox_id);
            let timeout = self.timeout;

            join_set.spawn(async move {
                let client = ShimClient::new(socket_path, timeout);
                match client.get("/metrics").await {
                    Ok(body) => Some((sandbox_id, body)),
                    Err(e) => {
                        tracing::error!(error = %e, "failed to get metrics from shim");
                        None
                    }
                }
            });
        }

        let mut per_sandbox_metrics: Vec<String> = Vec::new();
        while let Some(result) = join_set.join_next().await {
            if let Ok(Some((sandbox_id, body))) = result {
                let annotated = inject_sandbox_label(&body, &sandbox_id);
                per_sandbox_metrics.push(annotated);
            }
        }

        Ok(merge_metric_families(&per_sandbox_metrics))
    }

    pub async fn collect_single(&self, sandbox_id: &str) -> Result<String> {
        let socket_path = self.runtime_config.socket_path(sandbox_id);
        let client = ShimClient::new(socket_path, self.timeout);
        let body = client.get("/metrics").await?;
        Ok(String::from_utf8_lossy(&body).to_string())
    }
}

fn merge_metric_families(all_sandbox_metrics: &[String]) -> String {
    type MetricFamily = (Option<String>, Option<String>, Vec<String>);
    let mut families: HashMap<String, MetricFamily> = HashMap::new();
    let mut family_order: Vec<String> = Vec::new();

    for metrics_text in all_sandbox_metrics {
        let mut current_family: Option<String> = None;

        for line in metrics_text.lines() {
            if line.starts_with("# HELP ") {
                let family_name = extract_family_name_from_comment(line);
                current_family = Some(family_name.clone());
                let entry = families
                    .entry(family_name.clone())
                    .or_insert_with(|| (None, None, Vec::new()));
                if entry.0.is_none() {
                    entry.0 = Some(line.to_string());
                    family_order.push(family_name);
                }
            } else if line.starts_with("# TYPE ") {
                let family_name = extract_family_name_from_comment(line);
                current_family = Some(family_name.clone());
                let entry = families
                    .entry(family_name)
                    .or_insert_with(|| (None, None, Vec::new()));
                if entry.1.is_none() {
                    entry.1 = Some(line.to_string());
                }
            } else if line.is_empty() {
                current_family = None;
            } else {
                let family_name = if let Some(ref name) = current_family {
                    name.clone()
                } else {
                    extract_family_name_from_data_line(line)
                };
                let entry = families.entry(family_name.clone()).or_insert_with(|| {
                    family_order.push(family_name);
                    (None, None, Vec::new())
                });
                entry.2.push(line.to_string());
            }
        }
    }

    let mut output = String::new();
    for family_name in &family_order {
        if let Some((help, type_line, data_lines)) = families.get(family_name) {
            if let Some(h) = help {
                output.push_str(h);
                output.push('\n');
            }
            if let Some(t) = type_line {
                output.push_str(t);
                output.push('\n');
            }
            for dl in data_lines {
                output.push_str(dl);
                output.push('\n');
            }
        }
    }
    output
}

fn extract_family_name_from_comment(line: &str) -> String {
    line.split(' ').nth(2).unwrap_or("").to_string()
}

fn extract_family_name_from_data_line(line: &str) -> String {
    let name = line.split(['{', ' ']).next().unwrap_or("");
    strip_metric_suffix(name).to_string()
}

fn strip_metric_suffix(name: &str) -> &str {
    const SUFFIXES: &[&str] = &["_total", "_bucket", "_sum", "_count", "_created"];
    for suffix in SUFFIXES {
        if let Some(base) = name.strip_suffix(suffix) {
            return base;
        }
    }
    name
}

fn inject_sandbox_label(raw_metrics: &[u8], sandbox_id: &str) -> String {
    let text = String::from_utf8_lossy(raw_metrics);
    let mut output = String::with_capacity(text.len() + text.len() / 4);

    for line in text.lines() {
        if line.starts_with('#') {
            output.push_str(&rename_metric_line(line));
            output.push('\n');
        } else if line.is_empty() {
            output.push('\n');
        } else {
            output.push_str(&add_sandbox_id_label(line, sandbox_id));
            output.push('\n');
        }
    }
    output
}

fn rename_metric_line(line: &str) -> String {
    if line.contains(" go_") || line.contains(" process_") {
        line.replace(" go_", " kata_shim_go_")
            .replace(" process_", " kata_shim_process_")
    } else {
        line.to_string()
    }
}

fn add_sandbox_id_label(line: &str, sandbox_id: &str) -> String {
    let label = format!("sandbox_id=\"{}\"", escape_label_value(sandbox_id));

    if let Some(brace_pos) = line.find('{') {
        let (before, after) = line.split_at(brace_pos + 1);
        let renamed = rename_metric_name_in_prefix(before);
        format!("{}{},{}", renamed, label, after)
    } else if let Some(space_pos) = line.find(' ') {
        let (name, value) = line.split_at(space_pos);
        let renamed_name = rename_metric_name(name);
        format!("{}{{{}}} {}", renamed_name, label, value.trim())
    } else {
        line.to_string()
    }
}

fn rename_metric_name(name: &str) -> String {
    if name.starts_with("go_") || name.starts_with("process_") {
        format!("kata_shim_{}", name)
    } else {
        name.to_string()
    }
}

fn rename_metric_name_in_prefix(before_brace: &str) -> String {
    if before_brace.starts_with("go_") || before_brace.starts_with("process_") {
        format!("kata_shim_{}", before_brace)
    } else {
        before_brace.to_string()
    }
}

fn escape_label_value(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

pub fn filter_metrics_by_family(metrics: &str, filter_families: &[&str]) -> String {
    if filter_families.is_empty() {
        return metrics.to_string();
    }

    let mut output = String::new();
    let mut include_block = false;

    for line in metrics.lines() {
        if line.starts_with("# HELP ") || line.starts_with("# TYPE ") {
            let parts: Vec<&str> = line.splitn(4, ' ').collect();
            if parts.len() >= 3 {
                let metric_name = parts[2];
                include_block = filter_families.iter().any(|f| metric_name.starts_with(f));
            }
        } else if line.is_empty() {
            if include_block {
                output.push('\n');
            }
            continue;
        }

        if include_block {
            output.push_str(line);
            output.push('\n');
        } else if !line.starts_with('#') && !line.is_empty() {
            let metric_name = line.split(['{', ' ']).next().unwrap_or("");
            if filter_families.iter().any(|f| metric_name.starts_with(f)) {
                output.push_str(line);
                output.push('\n');
            }
        }
    }
    output
}
