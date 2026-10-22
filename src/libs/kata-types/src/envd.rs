// Copyright (c) 2026 Ant Group
// SPDX-License-Identifier: Apache-2.0

use serde::Deserialize;

/// Default envd executable in the guest image.
pub const ENVD_GUEST_PATH: &str = "/usr/libexec/kata-containers/kata-envd";
/// Container destination on /dev tmpfs, including for read-only root filesystems.
pub const ENVD_CONTAINER_PATH: &str = "/dev/.kata-containers/kata-envd";

/// Sandbox annotation configuration; the runtime expands args before agent delivery.
#[derive(Debug, Clone, serde::Serialize, Deserialize, PartialEq, Eq)]
pub struct EnvdAnnotation {
    /// Guest listening port, available to the caller for host port mapping.
    pub port: u16,
    /// Complete argument vector, excluding argv[0]; the runtime expands `{port}`.
    pub args: Vec<String>,
    /// Optional guest executable path; omitted values use `ENVD_GUEST_PATH`.
    /// The runtime consumes this field when preparing the container mount.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guest_path: Option<String>,
}

impl EnvdAnnotation {
    /// Parse and validate an envd sandbox annotation.
    pub fn parse(raw: &str) -> anyhow::Result<Self> {
        use anyhow::{ensure, Context};

        ensure!(
            raw.trim_start().starts_with('{'),
            "envd annotation must be a JSON object"
        );
        let config: Self = serde_json::from_str(raw).context("invalid envd annotation")?;
        ensure!(config.port != 0, "envd port must be between 1 and 65535");
        ensure!(
            config.args.iter().all(|arg| !arg.contains('\0')),
            "envd args must not contain NUL"
        );
        if let Some(path) = &config.guest_path {
            ensure!(
                std::path::Path::new(path).is_absolute() && !path.contains('\0'),
                "envd guest_path must be an absolute path without NUL"
            );
        }
        Ok(config)
    }

    /// Expand port placeholders while preserving argument boundaries.
    pub fn resolve_args(&mut self) {
        let port = self.port.to_string();
        for arg in &mut self.args {
            *arg = arg.replace("{port}", &port);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::EnvdAnnotation;

    #[test]
    fn rejects_malformed_or_incomplete_requests() {
        for raw in [
            "",
            "null",
            "[]",
            "{",
            "{}",
            r#"{"args":[]}"#,
            r#"{"port":49983}"#,
            r#"{"port":0,"args":[]}"#,
            r#"{"port":65536,"args":[]}"#,
            r#"{"port":-1,"args":[]}"#,
            r#"{"port":1.5,"args":[]}"#,
            r#"{"port":null,"args":[]}"#,
            r#"{"port":49983,"args":null}"#,
            r#"{"port":49983,"args":"serve"}"#,
            r#"{"port":49983,"args":[null]}"#,
            r#"{"port":49983,"args":["\u0000"]}"#,
            r#"{"port":49983,"args":[],"guest_path":""}"#,
            r#"{"port":49983,"args":[],"guest_path":"bin/envd"}"#,
            r#"{"port":49983,"args":[],"guest_path":"/bin/envd\u0000"}"#,
            r#"{"port":49983,"args":[],"guest_path":1}"#,
            r#"{"port":49983,"args":[],"guest_path":[]}"#,
        ] {
            assert!(EnvdAnnotation::parse(raw).is_err(), "{}", raw);
        }
    }

    #[test]
    fn guest_path_is_optional_and_preserves_custom_locations() {
        let config = EnvdAnnotation::parse(r#"{"port":49983,"args":[]}"#).unwrap();
        assert_eq!(config.guest_path, None);
        assert_eq!(
            serde_json::to_value(&config).unwrap(),
            serde_json::json!({"port":49983,"args":[]})
        );

        for path in [
            "/run/kata-extensions/envd/bin/kata-envd",
            "/opt/custom/envd with spaces",
        ] {
            let request = serde_json::json!({"port":49983,"args":[],"guest_path":path});
            let config = EnvdAnnotation::parse(&request.to_string()).unwrap();
            assert_eq!(config.guest_path.as_deref(), Some(path));
            assert_eq!(serde_json::to_value(&config).unwrap(), request);
        }
    }

    #[test]
    fn preserves_argument_boundaries_and_empty_vector() {
        for args in [
            vec![
                "--addr=:{port}",
                "{port}:{port}",
                "two words",
                "$(literal)",
                "",
            ],
            vec![],
        ] {
            let raw = serde_json::json!({"port":49983,"args":args});
            let mut config = EnvdAnnotation::parse(&raw.to_string()).unwrap();
            config.resolve_args();
            assert_eq!(
                config.args,
                args.iter()
                    .map(|arg| arg.replace("{port}", "49983"))
                    .collect::<Vec<_>>()
            );
        }
    }
}
