// Copyright (c) 2025 Edgeless Systems GmbH
//
// SPDX-License-Identifier: Apache-2.0
//

use assert_cmd::prelude::*;
use std::fs::{self};
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
