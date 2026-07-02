// Copyright (c) 2025 Edgeless Systems GmbH
//
// SPDX-License-Identifier: Apache-2.0
//

use assert_cmd::prelude::*;
use std::fs::{self};
use std::io::Write;
use std::path;
use std::process::Command;

#[test]
fn config_map_in_separate_file_config_map_flag() -> Result<(), Box<dyn std::error::Error>> {
    // Prepare temp dir for running genpolicy.
    let test_case_dir = "config_map_separate_file_config_map_flag";
    let pod_yaml_name = "pod_with_config_map_ref.yaml";
    let config_file = "config_map.yaml";
    let workdir = prepare_workdir(test_case_dir, &[pod_yaml_name, config_file]);

    let mut cmd = Command::cargo_bin("genpolicy")?;
    cmd.arg("--yaml-file").arg(workdir.join(pod_yaml_name));
    cmd.assert().failure();

    let mut cmd = Command::cargo_bin("genpolicy")?;
    cmd.arg("--yaml-file").arg(workdir.join(pod_yaml_name));
    cmd.arg("--config-map-file").arg(workdir.join(config_file));
    cmd.assert().success();

    Ok(())
}

#[test]
fn config_map_in_separate_file_workdir_flag() -> Result<(), Box<dyn std::error::Error>> {
    // Prepare temp dir for running genpolicy.
    let test_case_dir = "config_map_separate_file_workdir_flag";
    let pod_yaml_name = "pod_with_config_map_ref.yaml";
    let config_file = "config_map.yaml";
    let workdir = prepare_workdir(test_case_dir, &[pod_yaml_name, config_file]);

    let mut cmd = Command::cargo_bin("genpolicy")?;
    cmd.arg("--yaml-file").arg(workdir.join(pod_yaml_name));
    cmd.assert().failure();

    let mut cmd = Command::cargo_bin("genpolicy")?;
    cmd.arg("--yaml-file").arg(workdir.join(pod_yaml_name));
    cmd.arg("--config-file").arg(workdir.join(config_file));
    cmd.assert().success();

    Ok(())
}

#[test]
fn secret_in_separate_file() -> Result<(), Box<dyn std::error::Error>> {
    // Prepare temp dir for running genpolicy.
    let test_case_dir = "secret_separate_file";
    let pod_yaml_name = "pod_with_secret_ref.yaml";
    let config_file = "secret.yaml";
    let workdir = prepare_workdir(test_case_dir, &[pod_yaml_name, config_file]);

    let mut cmd = Command::cargo_bin("genpolicy")?;
    cmd.arg("--yaml-file").arg(workdir.join(pod_yaml_name));
    cmd.assert().failure();

    let mut cmd = Command::cargo_bin("genpolicy")?;
    cmd.arg("--yaml-file").arg(workdir.join(pod_yaml_name));
    cmd.arg("--config-file").arg(workdir.join(config_file));
    cmd.assert().success();

    Ok(())
}

#[test]
fn guest_pull_missing_supplemental_groups_exits() -> Result<(), Box<dyn std::error::Error>> {
    let test_case_dir = "guest_pull_missing_supplemental_groups";
    let workdir = prepare_workdir(test_case_dir, &[]);
    let pod_yaml_path = workdir.join("missing_supplemental_groups.yaml");
    fs::write(
        &pod_yaml_path,
        r#"---
apiVersion: v1
kind: Pod
metadata:
  name: missing-supplemental-groups
spec:
  restartPolicy: Never
  runtimeClassName: kata-cc
  containers:
    - name: busybox
      image: "quay.io/prometheus/busybox:latest"
      command:
        - /bin/sh
      args:
        - "-c"
        - echo hello
"#,
    )?;

    let mut cmd = Command::cargo_bin("genpolicy")?;
    cmd.arg("--yaml-file").arg(&pod_yaml_path);

    let output = cmd.output()?;
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ERROR: guest_pull is enabled"));
    assert!(stderr.contains("Set explicit Kubernetes securityContext values"));
    assert!(!stderr.contains("      runAsUser: 0"), "{stderr}");
    assert!(!stderr.contains("      runAsGroup: 0"), "{stderr}");
    assert!(stderr.contains("supplementalGroups: [10]"));

    Ok(())
}

#[test]
fn guest_pull_run_as_user_requires_non_default_group() -> Result<(), Box<dyn std::error::Error>> {
    let test_case_dir = "guest_pull_run_as_user_requires_non_default_group";
    let workdir = prepare_workdir(test_case_dir, &[]);
    let pod_yaml_path = workdir.join("run_as_user_requires_non_default_group.yaml");
    fs::write(
        &pod_yaml_path,
        r#"---
apiVersion: v1
kind: Pod
metadata:
  name: run-as-user-requires-non-default-group
spec:
  restartPolicy: Never
  runtimeClassName: kata-cc
  securityContext:
    runAsUser: 33
  containers:
    - name: busybox
      image: "quay.io/prometheus/busybox:latest"
      command:
        - /bin/sh
      args:
        - "-c"
        - echo hello
"#,
    )?;

    let mut cmd = Command::cargo_bin("genpolicy")?;
    cmd.arg("--yaml-file").arg(&pod_yaml_path);

    let output = cmd.output()?;
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ERROR: guest_pull is enabled"));
    assert!(stderr.contains("      runAsUser: 33"), "{stderr}");
    assert!(stderr.contains("      runAsGroup: 33"), "{stderr}");
    assert!(!stderr.contains("supplementalGroups:"), "{stderr}");

    Ok(())
}

#[test]
fn guest_pull_default_zero_values_are_optional() -> Result<(), Box<dyn std::error::Error>> {
    let cases = &[
        (
            "run_as_user_without_run_as_group",
            r#"---
apiVersion: v1
kind: Pod
metadata:
  name: run-as-user
spec:
  restartPolicy: Never
  runtimeClassName: kata-cc
  securityContext:
    runAsUser: 1000
  containers:
    - name: busybox
      image: "quay.io/prometheus/busybox:latest"
      command:
        - /bin/sh
      args:
        - "-c"
        - echo hello
"#,
        ),
        (
            "supplemental_groups_without_run_as_user_or_group",
            r#"---
apiVersion: v1
kind: Pod
metadata:
  name: run-supplemental
spec:
  restartPolicy: Never
  runtimeClassName: kata-cc
  securityContext:
    supplementalGroups:
      - 10
  containers:
    - name: busybox
      image: "quay.io/prometheus/busybox:latest"
      command:
        - /bin/sh
      args:
        - "-c"
        - echo hello
"#,
        ),
    ];

    for (name, yaml) in cases {
        let test_case_dir = format!("guest_pull_default_security_context_values_{name}");
        let workdir = prepare_workdir(&test_case_dir, &[]);
        let pod_yaml_path = workdir.join(format!("{name}.yaml"));
        fs::write(&pod_yaml_path, yaml)?;

        let mut cmd = Command::cargo_bin("genpolicy")?;
        cmd.arg("--yaml-file").arg(&pod_yaml_path);
        cmd.assert().success();
    }

    Ok(())
}

#[test]
fn guest_pull_split_security_context_succeeds() -> Result<(), Box<dyn std::error::Error>> {
    let cases = &[
        (
            "split_user_group_security_context",
            r#"---
apiVersion: v1
kind: Pod
metadata:
  name: split-user-group-security-context
spec:
  restartPolicy: Never
  runtimeClassName: kata-cc
  securityContext:
    runAsUser: 0
    supplementalGroups:
      - 10
  containers:
    - name: busybox
      image: "quay.io/prometheus/busybox:latest"
      command:
        - /bin/sh
      args:
        - "-c"
        - echo hello
      securityContext:
        runAsGroup: 0
"#,
        ),
        (
            "split_security_context",
            r#"---
apiVersion: v1
kind: Pod
metadata:
  name: split-security-context
spec:
  restartPolicy: Never
  runtimeClassName: kata-cc
  securityContext:
    supplementalGroups:
      - 10
  containers:
    - name: busybox
      image: "quay.io/prometheus/busybox:latest"
      command:
        - /bin/sh
      args:
        - "-c"
        - echo hello
      securityContext:
        runAsUser: 0
        runAsGroup: 0
"#,
        ),
    ];

    for (name, yaml) in cases {
        let test_case_dir = format!("guest_pull_{name}");
        let workdir = prepare_workdir(&test_case_dir, &[]);
        let pod_yaml_path = workdir.join(format!("{name}.yaml"));
        fs::write(&pod_yaml_path, yaml)?;

        let mut cmd = Command::cargo_bin("genpolicy")?;
        cmd.arg("--yaml-file").arg(&pod_yaml_path);
        cmd.assert().success();
    }

    Ok(())
}

#[test]
fn output_behavior() -> Result<(), Box<dyn std::error::Error>> {
    struct TestCase {
        name: &'static str,
        flag: Option<&'static str>,
        use_yaml_file: bool,
        expect_yaml_in_stdout: bool,
        expect_base64_in_stdout: bool,
        expect_raw_in_stdout: bool,
    }

    let test_cases = &[
        // --yaml-file alone: file modified, no stdout
        TestCase {
            name: "yaml_file_only",
            flag: None,
            use_yaml_file: true,
            expect_yaml_in_stdout: false,
            expect_base64_in_stdout: false,
            expect_raw_in_stdout: false,
        },
        // --yaml-file --base64-out: file modified, base64 to stdout
        TestCase {
            name: "yaml_file_with_base64_out",
            flag: Some("--base64-out"),
            use_yaml_file: true,
            expect_yaml_in_stdout: false,
            expect_base64_in_stdout: true,
            expect_raw_in_stdout: false,
        },
        // --yaml-file --raw-out: file modified, raw to stdout
        TestCase {
            name: "yaml_file_with_raw_out",
            flag: Some("--raw-out"),
            use_yaml_file: true,
            expect_yaml_in_stdout: false,
            expect_base64_in_stdout: false,
            expect_raw_in_stdout: true,
        },
        // stdin alone: annotated YAML to stdout
        TestCase {
            name: "stdin_only",
            flag: None,
            use_yaml_file: false,
            expect_yaml_in_stdout: true,
            expect_base64_in_stdout: false,
            expect_raw_in_stdout: false,
        },
        // stdin --base64-out: only base64 to stdout (suppress YAML)
        TestCase {
            name: "stdin_with_base64_out",
            flag: Some("--base64-out"),
            use_yaml_file: false,
            expect_yaml_in_stdout: false,
            expect_base64_in_stdout: true,
            expect_raw_in_stdout: false,
        },
        // stdin --raw-out: only raw to stdout (suppress YAML)
        TestCase {
            name: "stdin_with_raw_out",
            flag: Some("--raw-out"),
            use_yaml_file: false,
            expect_yaml_in_stdout: false,
            expect_base64_in_stdout: false,
            expect_raw_in_stdout: true,
        },
    ];

    for tc in test_cases.iter() {
        let workdir = prepare_workdir(tc.name, &["simple_pod.yaml"]);
        let pod_yaml_path = workdir.join("simple_pod.yaml");

        let output = if tc.use_yaml_file {
            let mut cmd = Command::cargo_bin("genpolicy")?;
            cmd.arg("--yaml-file").arg(&pod_yaml_path);
            if let Some(flag) = tc.flag {
                cmd.arg(flag);
            }
            cmd.output()?
        } else {
            let pod_yaml_content = fs::read_to_string(&pod_yaml_path)?;
            let mut cmd = Command::cargo_bin("genpolicy")?;
            cmd.current_dir(&workdir);
            if let Some(flag) = tc.flag {
                cmd.arg(flag);
            }
            let mut child = cmd
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .spawn()?;
            child
                .stdin
                .take()
                .unwrap()
                .write_all(pod_yaml_content.as_bytes())?;
            child.wait_with_output()?
        };

        assert!(output.status.success(), "{}: command failed", tc.name);

        let stdout = String::from_utf8_lossy(&output.stdout);
        let has_yaml = stdout.contains("apiVersion:");
        let has_raw = stdout.contains("policy_data");
        let has_base64 = !stdout.trim().is_empty() && !has_yaml && !has_raw;

        assert_eq!(
            has_yaml, tc.expect_yaml_in_stdout,
            "{}: yaml in stdout",
            tc.name
        );
        assert_eq!(
            has_raw, tc.expect_raw_in_stdout,
            "{}: raw policy in stdout",
            tc.name
        );
        assert_eq!(
            has_base64, tc.expect_base64_in_stdout,
            "{}: base64 policy in stdout",
            tc.name
        );
    }

    Ok(())
}

fn prepare_workdir(test_case_dir: &str, files_to_copy: &[&str]) -> path::PathBuf {
    // Prepare temp dir for running genpolicy.
    let workdir = path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(test_case_dir);
    fs::create_dir_all(&workdir)
        .expect("should be able to create directories under CARGO_TARGET_TMPDIR");

    let testdata_dir =
        path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/generate/testdata");

    // Make sure that workdir is empty.
    for entry in fs::read_dir(&workdir).expect("should be able to read directories") {
        let entry = entry.expect("should be able to read directory entries");
        fs::remove_file(entry.path()).expect("should be able to remove files");
    }

    for file in files_to_copy {
        fs::copy(testdata_dir.join(file), workdir.join(file))
            .expect("copying files around should not fail");
    }

    let genpolicy_dir = path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    for base in ["rules.rego", "genpolicy-settings.json"] {
        fs::copy(genpolicy_dir.join(base), workdir.join(base))
            .expect("copying files around should not fail");
    }

    workdir
}
