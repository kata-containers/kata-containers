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
        AddARPNeighborsRequest, CopyFileRequest, CreateContainerRequest, CreateSandboxRequest,
        ExecProcessRequest, RemoveContainerRequest, UpdateInterfaceRequest, UpdateRoutesRequest,
    };
    use serde::{Deserialize, Serialize};

    use kata_agent_policy::policy::AgentPolicy;

    // Translate each test case in testcases.json
    // to one request type.
    #[derive(Clone, Debug, Deserialize, Serialize)]
    #[serde(tag = "type")]
    enum TestRequest {
        CopyFile(CopyFileRequest),
        CreateContainer(CreateContainerRequest),
        CreateSandbox(CreateSandboxRequest),
        ExecProcess(ExecProcessRequest),
        RemoveContainer(RemoveContainerRequest),
        UpdateInterface(UpdateInterfaceRequest),
        UpdateRoutes(UpdateRoutesRequest),
        AddARPNeighbors(AddARPNeighborsRequest),
    }

    impl Display for TestRequest {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                TestRequest::CopyFile(_) => write!(f, "CopyFileRequest"),
                TestRequest::CreateContainer(_) => write!(f, "CreateContainerRequest"),
                TestRequest::CreateSandbox(_) => write!(f, "CreateSandboxRequest"),
                TestRequest::ExecProcess(_) => write!(f, "ExecProcessRequest"),
                TestRequest::RemoveContainer(_) => write!(f, "RemoveContainerRequest"),
                TestRequest::UpdateInterface(_) => write!(f, "UpdateInterfaceRequest"),
                TestRequest::UpdateRoutes(_) => write!(f, "UpdateRoutesRequest"),
                TestRequest::AddARPNeighbors(_) => write!(f, "AddARPNeighborsRequest"),
            }
        }
    }

    #[derive(Clone, Debug, Deserialize, Serialize)]
    struct TestCase {
        description: String,
        allowed: bool,
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
            base64_out: true,
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
                        println!("Retrying to generate policy: {}", e);
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

            let v = serde_json::to_value(&test_case.request).unwrap();

            let results = pol
                .allow_request(
                    &test_case.request.to_string(),
                    &serde_json::to_string(&v).unwrap(),
                )
                .await;

            let logs = fs::read_to_string(workdir.join("policy.log")).unwrap();
            let results = results.unwrap();

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
}
