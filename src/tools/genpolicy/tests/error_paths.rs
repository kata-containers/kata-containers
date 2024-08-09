// Copyright (c) 2024 Microsoft
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fs;
use std::path;

#[cfg(test)]
fn execute_error_path_test(test_name: &str) -> std::process::Output {
    let workdir = path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("error_paths");
    fs::create_dir_all(&workdir)
        .expect("should be able to create directories under CARGO_TARGET_TMPDIR");

    let genpolicy_dir = path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    for base in ["rules.rego", "genpolicy-settings.json"] {
        fs::copy(genpolicy_dir.join(base), workdir.join(base))
            .expect("copying files around should not fail");
    }

    let test_input_dir = genpolicy_dir.join("tests/error_paths/");
    let test_yaml = format!("{test_name}.yaml");
    fs::copy(test_input_dir.join(&test_yaml), workdir.join(&test_yaml))
        .expect("copying files around should not fail");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_genpolicy"))
        .current_dir(workdir)
        .args(["-u", "-y", &test_yaml])
        .output()
        .expect("executing the genpolicy command should not fail");
    println!("genpolicy output: {:?}", &output);
    output
}

// The container image used by this pod YAML defines the destination
// of a volume mount without also defining the source of the mount.
// genpolicy rejects such images if the YAML file doesn't define the
// corresponding mount source information. Mount sources are required
// to generate a reasonable confidential containers policy.
#[test]
fn volume_source_missing() {
    let output = execute_error_path_test("volume_source_missing");
    assert_ne!(output.status.code(), Some(0));
    
    let std_err_output = std::str::from_utf8(&output.stderr)
        .expect("genpolicy stderr output should be in utf8 format")
        .to_string();
    println!("genpolicy stderr output:\n{std_err_output}");
    
    assert!(std_err_output.contains("Unsupported policy inputs"));
    assert!(std_err_output.contains("Please define volume mount"));
}

// This is similar to the test case above, but the YAML file defines the
// source of the mount, so genpolicy is successful.
#[test]
fn volume_source_present() {
    let output = execute_error_path_test("volume_source_present");
    assert_eq!(output.status.code(), Some(0));
}
