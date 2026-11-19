// Copyright (c) 2024 Edgeless Systems GmbH
//
// SPDX-License-Identifier: Apache-2.0
//

#[path = "../common/mod.rs"]
mod common;

#[cfg(test)]
mod tests {
    use super::common::genpolicy_succeeds_stdin;
    use anyhow::Context;
    use std::collections::BTreeMap;
    use std::fmt::{self, Display};
    use std::fs::{self, File};
    use std::path;
    use std::str;

    use json_patch::{patch, Patch};
    use protocols::agent::{
        AddARPNeighborsRequest, CreateContainerRequest, CreateSandboxRequest, ExecProcessRequest,
        RemoveContainerRequest, UpdateInterfaceRequest, UpdateRoutesRequest,
    };
    use serde::{Deserialize, Serialize};

    use kata_agent_policy::policy::{AgentPolicy, PolicyCopyFileRequest};

    #[tokio::test]
    async fn test_state_create_container_remove_without_state_returns_empty_patch() {
        let workdir = tempfile::tempdir().unwrap();
        let policy_path = workdir.path().join("policy.rego");
        let log_path = workdir.path().join("policy.log");
        fs::write(
            &policy_path,
            format!(
                "{}\npolicy_data := {{\"containers\": [{{}}]}}\n",
                include_str!("../../rules.rego"),
            ),
        )
        .unwrap();
        let mut policy = AgentPolicy::new();
        policy
            .initialize(
                slog::Level::Debug.as_usize(),
                policy_path.to_str().unwrap().into(),
                Some(log_path.to_str().unwrap().into()),
            )
            .await
            .unwrap();

        for container_id in ["unknown", "unknown", ""] {
            let request = RemoveContainerRequest {
                container_id: container_id.to_string(),
                ..Default::default()
            };
            let decision = policy
                .evaluate_request(
                    "RemoveContainerRequest",
                    &serde_json::to_string(&request).unwrap(),
                )
                .await
                .unwrap();
            assert!(decision.allowed());
            decision.commit(&mut policy).await.unwrap();
        }
        let responses: Vec<serde_json::Value> = fs::read_to_string(log_path)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .filter_map(|entry| entry.get("response").cloned())
            .collect();
        assert_eq!(
            responses,
            vec![
                serde_json::json!({"allowed": true, "ops": []}),
                serde_json::json!({"allowed": true, "ops": []}),
                serde_json::json!({"allowed": true, "ops": []}),
            ]
        );
    }

    // Translate each test case in testcases.json
    // to one request type.
    #[derive(Clone, Debug, Deserialize, Serialize)]
    #[serde(tag = "kind", content = "request")]
    #[allow(clippy::enum_variant_names)] // The tags need to match the entrypoint logged by the agent.
    enum TestRequest {
        CopyFileRequest(PolicyCopyFileRequest),
        CreateContainerRequest(CreateContainerRequest),
        CreateSandboxRequest(CreateSandboxRequest),
        ExecProcessRequest(ExecProcessRequest),
        RemoveContainerRequest(RemoveContainerRequest),
        UpdateInterfaceRequest(UpdateInterfaceRequest),
        UpdateRoutesRequest(UpdateRoutesRequest),
        AddARPNeighborsRequest(AddARPNeighborsRequest),
    }

    impl Display for TestRequest {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                TestRequest::CopyFileRequest(_) => write!(f, "CopyFileRequest"),
                TestRequest::CreateContainerRequest(_) => write!(f, "CreateContainerRequest"),
                TestRequest::CreateSandboxRequest(_) => write!(f, "CreateSandboxRequest"),
                TestRequest::ExecProcessRequest(_) => write!(f, "ExecProcessRequest"),
                TestRequest::RemoveContainerRequest(_) => write!(f, "RemoveContainerRequest"),
                TestRequest::UpdateInterfaceRequest(_) => write!(f, "UpdateInterfaceRequest"),
                TestRequest::UpdateRoutesRequest(_) => write!(f, "UpdateRoutesRequest"),
                TestRequest::AddARPNeighborsRequest(_) => write!(f, "AddARPNeighborsRequest"),
            }
        }
    }

    impl TestRequest {
        fn is_stateful(&self) -> bool {
            matches!(
                self,
                Self::CreateContainerRequest(_)
                    | Self::CreateSandboxRequest(_)
                    | Self::ExecProcessRequest(_)
                    | Self::RemoveContainerRequest(_)
            )
        }
    }

    fn serialize_request_only(value: &TestRequest) -> serde_json::Result<serde_json::Value> {
        if let serde_json::Value::Object(map) = serde_json::to_value(value)? {
            for (k, v) in map {
                if k == "request" {
                    return Ok(v);
                }
            }
        }
        Ok(serde_json::Value::Null)
    }

    #[derive(Clone, Debug, Deserialize, Serialize)]
    struct TestCase {
        description: String,
        allowed: bool,
        base: String,
        #[serde(default)]
        patch: Patch,
    }

    #[derive(Clone, Debug, Deserialize, Serialize)]
    struct TestCases {
        bases: BTreeMap<String, TestRequest>,
        cases: Vec<TestCase>,
    }

    /// Run tests from the given directory.
    /// The directory is searched under `src/tools/genpolicy/tests/policy/testdata`, and
    /// it must contain a `pod.yaml` file as well as a `testcases.json` file.
    /// The pod must produce a policy when fed into genpolicy, so there
    /// should be exactly one entry with a PodSpec. The test case file contains named base
    /// requests and ordered cases. Each case applies an RFC 6902 JSON Patch to the payload of
    /// its selected base request.
    /// An optional `settings-patch.json` file in the test case directory customizes
    /// the genpolicy settings used by that test.
    async fn runtests(test_case_dir: &str) {
        // Check if config_map.yaml exists.
        // If it does, we need to copy it to the workdir.
        let is_config_map_file_present = path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/policy/testdata")
            .join(test_case_dir)
            .join("config_map.yaml")
            .exists();

        let files_to_copy = if is_config_map_file_present {
            vec!["pod.yaml", "config_map.yaml"]
        } else {
            vec!["pod.yaml"]
        };

        // Prepare temp dir for running genpolicy.
        let (workdir, testdata_dir) = prepare_workdir(test_case_dir, &files_to_copy);

        prepare_settings(&workdir, &testdata_dir);

        let pod_yaml = fs::read(workdir.join("pod.yaml")).expect("should read pod YAML");
        let resources: Vec<serde_yaml::Value> = serde_yaml::Deserializer::from_slice(&pod_yaml)
            .map(serde_yaml::Value::deserialize)
            .collect::<Result<_, _>>()
            .expect("pod YAML should parse");
        assert_eq!(
            resources.iter().filter(|value| !value.is_null()).count(),
            1,
            "pod YAML should contain exactly one resource"
        );
        let output = genpolicy_succeeds_stdin(&pod_yaml, |cmd| {
            cmd.current_dir(&workdir)
                .arg("--base64-out")
                .arg("--rego-rules-path")
                .arg(workdir.join("rules.rego"))
                .arg("--json-settings-path")
                .arg(&workdir);
            if is_config_map_file_present {
                cmd.arg("--config-file")
                    .arg(workdir.join("config_map.yaml"));
            }
        })
        .expect("genpolicy should generate a policy");
        let initdata_anno =
            str::from_utf8(&output.stdout).expect("genpolicy output should be UTF-8");
        let policy = decode_policy(initdata_anno);

        // write policy to a file
        fs::write(workdir.join("policy.rego"), &policy).unwrap();

        // Write policy back to a file

        // Re-implement needed parts of AgentPolicy::initialize()
        let mut pol = AgentPolicy::new();
        pol.initialize(
            slog::Level::Debug.as_usize(),
            workdir.join("policy.rego").to_str().unwrap().to_string(),
            workdir.join("policy.log").to_str().map(|s| s.to_string()),
        )
        .await
        .unwrap();

        // Run through the test cases and evaluate the canned requests.

        let case_file =
            File::open(testdata_dir.join("testcases.json")).expect("test case file should open");
        let test_cases: TestCases =
            serde_json::from_reader(case_file).expect("test case file should parse");

        for test_case in test_cases.cases {
            println!("\n== case: {} ==\n", test_case.description);

            let base = test_cases
                .bases
                .get(&test_case.base)
                .unwrap_or_else(|| panic!("Unknown test case base: {}", test_case.base));
            let mut request = serialize_request_only(base).unwrap();
            patch(&mut request, &test_case.patch).unwrap_or_else(|e| {
                panic!(
                    "Failed to apply patch for test case {}: {}",
                    test_case.description, e
                )
            });

            let ep = base.to_string();
            let request = serde_json::to_string(&request).unwrap();
            let results = if base.is_stateful() {
                match pol.evaluate_request(&ep, &request).await {
                    Ok(decision) => {
                        let allowed = decision.allowed();
                        let prints = decision.prints().to_owned();
                        if allowed {
                            decision.commit(&mut pol).await.map(|_| (allowed, prints))
                        } else {
                            Ok((allowed, prints))
                        }
                    }
                    Err(err) => Err(err),
                }
            } else {
                pol.allow_request(&ep, &request).await
            };

            let logs = fs::read_to_string(workdir.join("policy.log")).unwrap();
            let results = results.unwrap();

            // TODO(burgerdev): better description of failure (left != right)
            assert_eq!(
                test_case.allowed, results.0,
                "logs: {}\npolicy: {}",
                logs, results.1
            );
        }
    }

    fn decode_policy(initdata_anno: &str) -> String {
        let initdata = kata_types::initdata::decode_initdata(initdata_anno)
            .expect("should decode initdata anno");
        initdata
            .get_coco_data("policy.rego")
            .expect("should read policy from initdata")
            .to_string()
    }

    #[test]
    fn successful_non_policy_output_is_not_retried() {
        let attempts = std::cell::Cell::new(0);
        let output = genpolicy_succeeds_stdin(b"", |cmd| {
            attempts.set(attempts.get() + 1);
            cmd.arg("--version");
        })
        .expect("genpolicy version command should succeed");

        let initdata_anno = str::from_utf8(&output.stdout).unwrap();
        assert!(std::panic::catch_unwind(|| decode_policy(initdata_anno)).is_err());
        assert_eq!(attempts.get(), 1);
    }

    fn prepare_settings(workdir: &path::Path, testdata_dir: &path::Path) {
        let settings_patch = testdata_dir.join("settings-patch.json");
        if settings_patch.exists() {
            let drop_in_dir = workdir.join("genpolicy-settings.d");
            fs::create_dir(&drop_in_dir).expect("settings drop-in directory should be created");
            fs::copy(&settings_patch, drop_in_dir.join("settings-patch.json"))
                .context(format!(
                    "{:?} --> {:?}",
                    settings_patch,
                    drop_in_dir.join("settings-patch.json")
                ))
                .expect("copying the settings patch should not fail");
        }
    }

    fn prepare_workdir(
        test_case_dir: &str,
        files_to_copy: &[&str],
    ) -> (path::PathBuf, path::PathBuf) {
        // Prepare temp dir for running genpolicy.
        let workdir = path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(test_case_dir);
        fs::create_dir_all(&workdir)
            .expect("should be able to create directories under CARGO_TARGET_TMPDIR");

        let testdata_dir = path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/policy/testdata")
            .join(test_case_dir);

        // Make sure that workdir is empty.
        for entry in fs::read_dir(&workdir).expect("should be able to read directories") {
            let entry = entry.expect("should be able to read directory entries");
            if entry
                .file_type()
                .expect("workdir entry should have a file type")
                .is_dir()
            {
                fs::remove_dir_all(entry.path()).expect("should be able to remove directories");
            } else {
                fs::remove_file(entry.path()).expect("should be able to remove files");
            }
        }

        for file in files_to_copy {
            fs::copy(testdata_dir.join(file), workdir.join(file))
                .context(format!(
                    "{:?} --> {:?}",
                    testdata_dir.join(file),
                    workdir.join(file)
                ))
                .expect("copying files around should not fail");
        }

        let genpolicy_dir = path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        for base in ["rules.rego", "genpolicy-settings.json"] {
            fs::copy(genpolicy_dir.join(base), workdir.join(base))
                .context(format!(
                    "{:?} --> {:?}",
                    genpolicy_dir.join(base),
                    workdir.join(base)
                ))
                .expect("copying files around should not fail");
        }

        (workdir, testdata_dir)
    }

    #[tokio::test]
    async fn test_copyfile() {
        runtests("copyfile").await;
    }

    #[tokio::test]
    async fn test_create_sandbox() {
        runtests("createsandbox").await;
    }

    #[tokio::test]
    async fn test_update_routes() {
        runtests("updateroutes").await;
    }

    #[tokio::test]
    async fn test_update_interface() {
        runtests("updateinterface").await;
    }

    #[tokio::test]
    async fn test_add_arp_neighbors() {
        runtests("addarpneighbors").await;
    }

    #[tokio::test]
    async fn test_create_container_network_namespace() {
        runtests("createcontainer/network_namespace").await;
    }

    #[tokio::test]
    async fn test_create_container_image_guest_pull_count() {
        runtests("createcontainer/image_guest_pull_count").await;
    }

    #[tokio::test]
    async fn test_create_container_image_host_pull() {
        runtests("createcontainer/image_host_pull").await;
    }

    #[tokio::test]
    async fn test_create_container_sysctls() {
        runtests("createcontainer/sysctls").await;
    }

    #[tokio::test]
    async fn test_create_container_generate_name() {
        runtests("createcontainer/generate_name").await;
    }

    #[tokio::test]
    async fn test_create_container_gid() {
        runtests("createcontainer/gid").await;
    }

    #[tokio::test]
    async fn test_create_container_cgroup_mount_extras() {
        runtests("createcontainer/cgroup_mount_extras").await;
    }

    #[tokio::test]
    async fn test_state_create_container() {
        runtests("state/createcontainer").await;
    }

    #[tokio::test]
    async fn test_state_exec_process() {
        runtests("state/execprocess").await;
    }

    #[tokio::test]
    async fn test_state_exec_process_deployment() {
        runtests("state/execprocessdeployment").await;
    }

    #[tokio::test]
    async fn test_create_container_security_context() {
        runtests("createcontainer/security_context/runas").await;
    }

    #[tokio::test]
    async fn test_create_container_security_context_supplemental_groups() {
        runtests("createcontainer/security_context/supplemental_groups").await;
    }

    #[tokio::test]
    async fn test_create_container_security_context_fsgroup() {
        runtests("createcontainer/security_context/fsgroup").await;
    }

    #[tokio::test]
    async fn test_create_container_volumes_empty_dir() {
        runtests("createcontainer/volumes/emptydir").await;
    }

    #[tokio::test]
    async fn test_create_container_volumes_config_map() {
        runtests("createcontainer/volumes/config_map").await;
    }

    #[tokio::test]
    async fn test_create_container_volumes_container_image() {
        runtests("createcontainer/volumes/container_image").await;
    }

    #[tokio::test]
    async fn test_create_container_image_guest_pull_storage_validation() {
        runtests("createcontainer/image_guest_pull_storage_validation").await;
    }

    #[tokio::test]
    async fn test_create_container_image_guest_pull_source_pinned() {
        runtests("createcontainer/image_guest_pull_source/pinned").await;
    }

    #[tokio::test]
    async fn test_create_container_image_guest_pull_source_unpinned() {
        runtests("createcontainer/image_guest_pull_source/unpinned").await;
    }

    #[tokio::test]
    async fn test_create_container_gpu_vfio_cdi() {
        runtests("createcontainer/gpu_vfio_cdi").await;
    }

    #[tokio::test]
    async fn test_create_container_ignored_fields() {
        runtests("createcontainer/ignored_fields").await;
    }

    #[tokio::test]
    async fn test_create_container_env_vars() {
        runtests("createcontainer/env_vars").await;
    }
}
