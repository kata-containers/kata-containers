// Copyright (c) 2026 Ant Group
// SPDX-License-Identifier: Apache-2.0

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
