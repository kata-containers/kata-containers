// Copyright (c) 2024 Edgeless Systems GmbH
//
// SPDX-License-Identifier: Apache-2.0
//

#[cfg(test)]
mod tests {
    use anyhow::Context;
    use std::fmt::{self, Display};
    use std::fs::{self, File};
    use std::path;
    use std::str;

    use protocols::agent::{
        AddARPNeighborsRequest, CreateContainerRequest, CreateSandboxRequest, ExecProcessRequest,
        RemoveContainerRequest, UpdateInterfaceRequest, UpdateRoutesRequest,
    };
    use serde::{Deserialize, Serialize};

    use kata_agent_policy::policy::{AgentPolicy, PolicyCopyFileRequest};

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
        #[serde(flatten)]
        request: TestRequest,
    }

    /// Run tests from the given directory.
    /// The directory is searched under `src/tools/genpolicy/tests/testdata`, and
    /// it must contain a `resources.yaml` file as well as a `testcases.json` file.
    /// The resources must produce a policy when fed into genpolicy, so there
    /// should be exactly one entry with a PodSpec. The test case file must contain
    /// a JSON list of [TestCase] instances. Each instance will be of type enum TestRequest,
    /// with the tag `type` listing the exact type of request.
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

        let config_files = if is_config_map_file_present {
            Some(vec![workdir
                .join("config_map.yaml")
                .to_str()
                .unwrap()
                .to_string()])
        } else {
            None
        };

        let config = genpolicy::utils::Config {
            base64_out: false,
            config_files,
            containerd_socket_path: None, // Some(String::from("/var/run/containerd/containerd.sock")),
            insecure_registries: Vec::new(),
            layers_cache: genpolicy::layers_cache::ImageLayersCache::new(&None),
            raw_out: false,
            rego_rules_path: workdir.join("rules.rego").to_str().unwrap().to_string(),
            runtime_class_names: Vec::new(),
            settings: genpolicy::settings::Settings::new(
                workdir.join("genpolicy-settings.json").to_str().unwrap(),
            ),
            silent_unsupported_fields: false,
            use_cache: false,
            version: false,
            yaml_file: workdir.join("pod.yaml").to_str().map(|s| s.to_string()),
            initdata: kata_types::initdata::InitData::new("sha256", "0.1.0"),
        };

        // The container repos/network calls can be unreliable, so retry
        // a few times before giving up.
        let mut initdata_anno = String::new();
        for i in 0..6 {
            initdata_anno = match genpolicy::policy::AgentPolicy::from_files(&config).await {
                Ok(policy) => {
                    assert_eq!(policy.resources.len(), 1);
                    policy.resources[0].generate_initdata_anno(&policy)
                }
                Err(e) => {
                    if i == 5 {
                        panic!("Failed to generate policy after 6 attempts");
                    } else {
                        println!("Retrying to generate policy: {e}");
                        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                        continue;
                    }
                }
            };
            break;
        }
        let policy = decode_policy(&initdata_anno);

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
        let test_cases: Vec<TestCase> =
            serde_json::from_reader(case_file).expect("test case file should parse");

        for test_case in test_cases {
            println!("\n== case: {} ==\n", test_case.description);

            let v = serialize_request_only(&test_case.request).unwrap();

            let results = pol
                .allow_request(
                    &test_case.request.to_string(),
                    &serde_json::to_string(&v).unwrap(),
                )
                .await;

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

    #[tokio::test]
    async fn test_confidential_storage_policy_exact_contract() {
        let rules_path = path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rules.rego");
        let mut rules = fs::read_to_string(rules_path).expect("rules.rego should open");
        rules.push_str(
            r#"
policy_data := {}
default ConfidentialStorageTest := false
ConfidentialStorageTest if {
    allow_storages([], input.storages, "", "")
    allow_confidential_volumes(input.policy, input.mounts, input.storages)
}
"#,
        );

        let manifest_uri = "kbs:///tenant/storage-manifests/workspace-v1";
        let mount_name = kata_types::mount::confidential_storage_mount_name(manifest_uri).unwrap();
        let mount_point =
            format!("/run/kata-containers/shared/containers/passthrough/{mount_name}");
        let policy = serde_json::json!({
            "volume_name": "workspace",
            "manifest_uri": manifest_uri,
            "requested_access": 2,
            "mount_destination": "/workspace",
            "mount_source": format!("^{mount_point}$"),
            "mount_type": "bind",
            "mount_options": ["rbind", "rprivate", "rw"],
            "storage_fstype": "confidential-storage",
            "filesystem_type": "ext4",
            "filesystem_options": ["nodev", "nosuid", "rw"],
            "fs_group": {
                "group_id": 3000,
                "group_change_policy": 1
            }
        });
        let mount = serde_json::json!({
            "destination": "/workspace",
            "source": mount_point,
            "type_": "bind",
            "options": ["rbind", "rprivate", "rw"]
        });
        let storage = serde_json::json!({
            "driver": "blk",
            "driver_options": [],
            "fs_group": {
                "group_id": 3000,
                "group_change_policy": 1
            },
            "fstype": "confidential-storage",
            "mount_point": mount_point,
            "options": [],
            "source": "00/00",
            "shared": false,
            "confidential_storage": {
                "manifest_uri": manifest_uri,
                "requested_access": 2
            }
        });

        async fn allowed(rules: &str, input: &serde_json::Value) -> bool {
            let mut policy = AgentPolicy::new();
            policy.set_policy(rules).await.unwrap();
            policy
                .allow_request("ConfidentialStorageTest", &input.to_string())
                .await
                .unwrap()
                .0
        }

        let valid = serde_json::json!({
            "policy": [policy],
            "mounts": [mount],
            "storages": [storage]
        });
        assert!(allowed(&rules, &valid).await);

        let mut second_policy = valid["policy"][0].clone();
        second_policy["mount_destination"] = serde_json::json!("/home/codewire");
        let mut second_mount = valid["mounts"][0].clone();
        second_mount["destination"] = serde_json::json!("/home/codewire");
        let multiple_mounts = serde_json::json!({
            "policy": [valid["policy"][0].clone(), second_policy],
            "mounts": [valid["mounts"][0].clone(), second_mount],
            "storages": [valid["storages"][0].clone()]
        });
        assert!(
            allowed(&rules, &multiple_mounts).await,
            "one confidential Storage may back several exactly authorized mounts"
        );

        let mut explicit_root_group = valid.clone();
        explicit_root_group["policy"][0]["fs_group"]["group_id"] = serde_json::json!(0);
        explicit_root_group["storages"][0]["fs_group"]["group_id"] = serde_json::json!(0);
        assert!(allowed(&rules, &explicit_root_group).await);

        let mut invalid_cases = Vec::new();

        let mut manifest_substitution = valid.clone();
        manifest_substitution["storages"][0]["confidential_storage"]["manifest_uri"] =
            serde_json::json!("kbs:///tenant/storage-manifests/other-v1");
        invalid_cases.push(("manifest substitution", manifest_substitution));

        let mut access_downgrade = valid.clone();
        access_downgrade["storages"][0]["confidential_storage"]["requested_access"] =
            serde_json::json!(1);
        invalid_cases.push(("access downgrade", access_downgrade));

        let mut target_substitution = valid.clone();
        target_substitution["mounts"][0]["destination"] = serde_json::json!("/other");
        invalid_cases.push(("target substitution", target_substitution));

        let mut plaintext_downgrade = valid.clone();
        plaintext_downgrade["storages"][0]["fstype"] = serde_json::json!("ext4");
        invalid_cases.push(("plaintext downgrade", plaintext_downgrade));

        let mut wrong_driver = valid.clone();
        wrong_driver["storages"][0]["driver"] = serde_json::json!("local");
        invalid_cases.push(("wrong storage driver", wrong_driver));

        let mut wrong_device_source = valid.clone();
        wrong_device_source["storages"][0]["source"] = serde_json::json!("/dev/vda");
        invalid_cases.push(("wrong device source", wrong_device_source));

        let mut mount_correlation_substitution = valid.clone();
        mount_correlation_substitution["mounts"][0]["source"] = serde_json::json!(
            "/run/kata-containers/shared/containers/passthrough/confidential-0000000000000000000000000000000000000000000000000000000000000000"
        );
        invalid_cases.push((
            "mount and storage correlation substitution",
            mount_correlation_substitution,
        ));

        let mut fs_group_substitution = valid.clone();
        fs_group_substitution["storages"][0]["fs_group"]["group_id"] = serde_json::json!(3001);
        invalid_cases.push(("fsGroup substitution", fs_group_substitution));

        let mut invalid_fs_group = valid.clone();
        invalid_fs_group["storages"][0]["fs_group"]["group_id"] = serde_json::json!(-1);
        invalid_cases.push(("invalid fsGroup", invalid_fs_group));

        let mut filesystem_contract_substitution = valid.clone();
        filesystem_contract_substitution["policy"][0]["filesystem_options"] =
            serde_json::json!(["rw"]);
        invalid_cases.push((
            "filesystem contract substitution",
            filesystem_contract_substitution,
        ));

        let mut extra_storage = valid.clone();
        extra_storage["storages"]
            .as_array_mut()
            .unwrap()
            .push(valid["storages"][0].clone());
        invalid_cases.push(("extra confidential storage", extra_storage));

        let mut missing_authorization = valid.clone();
        missing_authorization["policy"] = serde_json::json!([]);
        invalid_cases.push(("missing policy authorization", missing_authorization));

        for (description, invalid) in invalid_cases {
            assert!(
                !allowed(&rules, &invalid).await,
                "unexpectedly allowed {description}"
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
            fs::remove_file(entry.path()).expect("should be able to remove files");
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
