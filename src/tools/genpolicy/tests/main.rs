// Copyright (c) 2024 Edgeless Systems GmbH
//
// SPDX-License-Identifier: Apache-2.0
//

use std::any;
use std::fs::{self, File};
use std::path;
use std::process::Command;
use std::str;

use protocols::agent::{CopyFileRequest, CreateSandboxRequest};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
struct TestCase<T> {
    description: String,
    allowed: bool,
    request: T,
}

/// Run tests from the given directory.
/// The directory is searched under `src/tools/genpolicy/tests/testdata`, and
/// it must contain a `resources.yaml` file as well as a `testcases.json` file.
/// The resources must produce a policy when fed into genpolicy, so there
/// should be exactly one entry with a PodSpec. The test case file must contain
/// a JSON list of [TestCase] instances appropriate for `T`.
fn runtests<T>(test_case_dir: &str)
where
    T: DeserializeOwned + Serialize,
{
    // Prepare temp dir for running genpolicy.

    let workdir = path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(test_case_dir);
    fs::create_dir_all(&workdir)
        .expect("should be able to create directories under CARGO_TARGET_TMPDIR");

    let genpolicy_dir = path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    for base in ["rules.rego", "genpolicy-settings.json"] {
        fs::copy(genpolicy_dir.join(base), workdir.join(base))
            .expect("copying files around should not fail");
    }

    let test_data = genpolicy_dir.join("tests/testdata").join(test_case_dir);
    fs::copy(test_data.join("pod.yaml"), workdir.join("pod.yaml"))
        .expect("copying files around should not fail");

    // Run the command and return the generated policy.

    let output = Command::new(env!("CARGO_BIN_EXE_genpolicy"))
        .current_dir(workdir)
        .args(["-u", "-r", "-y", "pod.yaml"])
        .output()
        .expect("executing the genpolicy command should not fail");

    assert_eq!(
        output.status.code(),
        Some(0),
        "genpolicy failed: {}",
        str::from_utf8(output.stderr.as_slice()).expect("genpolicy should return status code 0")
    );
    let policy = str::from_utf8(output.stdout.as_slice())
        .unwrap()
        .to_string();

    // Set up the policy engine.

    let mut pol = regorus::Engine::new();
    pol.add_policy("policy.rego".to_string(), policy).unwrap();

    // Run through the test cases and evaluate the canned requests.

    let case_file =
        File::open(test_data.join("testcases.json")).expect("test case file should open");
    let test_cases: Vec<TestCase<T>> =
        serde_json::from_reader(case_file).expect("test case file should parse");

    for test_case in test_cases {
        println!("\n== case: {} ==\n", test_case.description);

        let v = serde_json::to_value(&test_case.request).unwrap();
        pol.set_input(v.into());
        let query = format!(
            "data.agent_policy.{}",
            any::type_name::<T>().split("::").last().unwrap()
        );
        assert_eq!(test_case.allowed, pol.eval_deny_query(query, true));
    }
}

#[test]
fn test_copyfile() {
    runtests::<CopyFileRequest>("copyfile");
}

#[test]
fn test_create_sandbox() {
    runtests::<CreateSandboxRequest>("createsandbox");
}
