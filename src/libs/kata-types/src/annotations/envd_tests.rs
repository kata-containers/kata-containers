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
    ] {
        assert!(EnvdAnnotation::parse(raw).is_err(), "{}", raw);
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
