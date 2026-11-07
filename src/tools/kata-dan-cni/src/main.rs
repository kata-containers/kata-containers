// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

fn main() {
    std::process::exit(kata_dan_cni::run_process());
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::process::{Command, Output, Stdio};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::OnceLock;

    use serde_json::{json, Value};

    const ANNOTATIONS: &str = "io.kubernetes.cri.pod-annotations";
    const DAN: &str = "networking.katacontainers.io/dan";
    const ARGS: &str = "K8S_POD_NAMESPACE=default;K8S_POD_NAME=demo;K8S_POD_UID=pod-uid";
    static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

    // Source-local tests do not receive CARGO_BIN_EXE. Build once and use Cargo's
    // executable path so these process tests also work from a clean target directory.
    fn binary() -> &'static PathBuf {
        static BINARY: OnceLock<PathBuf> = OnceLock::new();
        BINARY.get_or_init(|| {
            let output = Command::new(env!("CARGO"))
                .args([
                    "build",
                    "--locked",
                    "--offline",
                    "--package",
                    "kata-dan-cni",
                    "--bin",
                    "kata-dan-cni",
                    "--message-format=json",
                ])
                .current_dir(env!("CARGO_MANIFEST_DIR"))
                .output()
                .expect("build the real kata-dan-cni binary");
            assert!(
                output.status.success(),
                "CNI binary build failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout)
                .expect("Cargo JSON output is UTF-8")
                .lines()
                .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                .find_map(|message| {
                    (message["reason"] == "compiler-artifact"
                        && message["target"]["name"] == "kata-dan-cni"
                        && message["target"]["kind"]
                            .as_array()
                            .is_some_and(|kinds| kinds.contains(&json!("bin"))))
                    .then(|| message["executable"].as_str().map(PathBuf::from))
                    .flatten()
                })
                .expect("Cargo emitted the CNI executable path")
        })
    }

    struct Fixture {
        directory: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let directory = std::env::temp_dir().join(format!(
                "kata-dan-cli-{}-{}",
                std::process::id(),
                NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&directory).expect("create isolated CLI directory");
            let delegate = directory.join("delegate-stub");
            fs::write(&delegate, r#"#!/bin/sh
set -eu
/bin/cat > "$CLI_TEST_DIR/$CNI_COMMAND.stdin"
printf '%s\n' "$CNI_COMMAND" "$CNI_CONTAINERID" "${CNI_NETNS-}" "$CNI_IFNAME" "${CNI_ARGS-}" "$CNI_PATH" > "$CLI_TEST_DIR/$CNI_COMMAND.env"
: > "$CLI_TEST_DIR/delegate-called"
case "$CNI_COMMAND" in
  ADD) /bin/cat "$CLI_TEST_DIR/result.json" ;;
  CHECK|DEL) ;;
  *) exit 90 ;;
esac
"#).expect("write delegate fixture");
            fs::set_permissions(&delegate, fs::Permissions::from_mode(0o755))
                .expect("make delegate executable");
            let fixture = Self { directory };
            fs::write(
                fixture.directory.join("result.json"),
                fixture.result().to_string(),
            )
            .expect("write delegate Result");
            fixture
        }

        fn result(&self) -> Value {
            json!({
                "cniVersion":"1.0.0",
                "interfaces":[{"name":"eth0", "sandbox":"/proc/example/netns"}],
                "ips":[{"address":"192.0.2.10/24", "gateway":"192.0.2.1", "interface":0}],
                "routes":[{"dst":"0.0.0.0/0", "gw":"192.0.2.1"}],
                "dns":{"nameservers":["192.0.2.53"], "search":["default.svc.cluster.local"]}
            })
        }

        fn config(&self) -> Value {
            json!({
                "cniVersion":"1.0.0", "name":"outer-network", "type":"kata-dan-cni",
                "danRuntimeClass":"kata-qemu-runtime-rs", "backend":{"type":"host-tap"},
                "danConfDir":self.directory.join("dans"),
                "kubeconfig":self.directory.join("does-not-exist.kubeconfig"), "nodeName":"node-a",
                "delegate": {
                    "name":"original-kube-ovn-network",
                    "plugins":[{
                        "type":"delegate-stub", "server_socket":"/run/openvswitch/kube-ovn-daemon.sock",
                        "provider":"blue.default.ovn", "capabilities":{"portMappings":true}
                    }]
                },
                "runtimeConfig":{"portMappings":[{"hostPort":8080,"containerPort":80,"protocol":"tcp"}]}
            })
        }

        fn invoke(&self, command: &str, input: &[u8], attachment_environment: bool) -> Output {
            let mut process = Command::new(binary());
            process
                .env_clear()
                .env("PATH", "/usr/bin:/bin")
                .env("CNI_COMMAND", command)
                .env("CLI_TEST_DIR", &self.directory)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            if attachment_environment {
                process
                    .env("CNI_CONTAINERID", "sandbox-id")
                    .env("CNI_IFNAME", "eth0")
                    .env(
                        "CNI_NETNS",
                        if command == "DEL" {
                            ""
                        } else {
                            "/proc/example/netns"
                        },
                    )
                    .env("CNI_ARGS", ARGS)
                    .env("CNI_PATH", &self.directory);
            }
            let mut child = process.spawn().expect("start real kata-dan-cni binary");
            child
                .stdin
                .take()
                .expect("piped stdin")
                .write_all(input)
                .expect("send CNI input");
            child.wait_with_output().expect("collect CNI output")
        }

        fn call(&self, command: &str, input: &Value) -> Output {
            self.invoke(command, input.to_string().as_bytes(), true)
        }

        fn was_delegated(&self) -> bool {
            self.directory.join("delegate-called").exists()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }

    fn json_stdout(output: &Output) -> Value {
        serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!(
                "stdout must be one JSON response: {error}; stdout={:?}; stderr={:?}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
        })
    }

    #[test]
    fn version_minimal_input() {
        let fixture = Fixture::new();
        let output = fixture.invoke("VERSION", br#"{"cniVersion":"1.0.0"}"#, false);
        assert!(output.status.success(), "{:?}", output);
        assert!(output.stderr.is_empty());
        let response = json_stdout(&output);
        assert_eq!(response["cniVersion"], "1.0.0");
        let versions = response["supportedVersions"]
            .as_array()
            .expect("supported CNI versions");
        for version in ["0.3.1", "0.4.0", "1.0.0"] {
            assert!(versions.contains(&json!(version)));
        }
        assert!(!fixture.was_delegated());
    }

    #[test]
    fn pod_delegation() {
        let fixture = Fixture::new();
        let mut config = fixture.config();
        let output = fixture.call("ADD", &config);
        assert!(output.status.success(), "{:?}", output);
        assert!(output.stderr.is_empty());
        assert_eq!(json_stdout(&output), fixture.result());
        assert!(fixture.was_delegated());
        assert!(!fixture.directory.join("dans").exists());

        config["prevResult"] = fixture.result();
        for command in ["CHECK", "DEL"] {
            let output = fixture.call(command, &config);
            assert!(output.status.success(), "{command}: {output:?}");
            assert!(output.stdout.is_empty(), "{command} must not emit a Result");
            assert!(output.stderr.is_empty());
        }
        for command in ["ADD", "CHECK", "DEL"] {
            let request: Value = serde_json::from_slice(
                &fs::read(fixture.directory.join(format!("{command}.stdin")))
                    .expect("delegate received stdin"),
            )
            .expect("delegate received JSON");
            assert_eq!(request["type"], "delegate-stub");
            assert_eq!(request["name"], "original-kube-ovn-network");
            assert_eq!(request["cniVersion"], "1.0.0");
            assert_eq!(
                request["server_socket"],
                "/run/openvswitch/kube-ovn-daemon.sock"
            );
            assert_eq!(request["provider"], "blue.default.ovn");
            assert_eq!(
                request["runtimeConfig"]["portMappings"],
                config["runtimeConfig"]["portMappings"]
            );
            assert!(request.get("backend").is_none());
            assert!(request.get("kubeconfig").is_none());
            if command == "ADD" {
                assert!(request.get("prevResult").is_none());
            } else {
                assert_eq!(request["prevResult"], fixture.result());
            }
            let received_environment =
                fs::read_to_string(fixture.directory.join(format!("{command}.env")))
                    .expect("delegate environment record");
            let fields: Vec<_> = received_environment.lines().collect();
            assert_eq!(
                fields,
                vec![
                    command,
                    "sandbox-id",
                    if command == "DEL" {
                        ""
                    } else {
                        "/proc/example/netns"
                    },
                    "eth0",
                    ARGS,
                    fixture.directory.to_str().expect("UTF-8 fixture path")
                ]
            );
        }
    }

    #[test]
    fn dan_no_fallback() {
        let fixture = Fixture::new();
        let mut config = fixture.config();
        config["runtimeConfig"][ANNOTATIONS] = json!({DAN:"true"});
        let output = fixture.call("ADD", &config);
        assert!(!output.status.success());
        let error = json_stdout(&output);
        assert_eq!(error["code"], 5);
        assert_eq!(error["cniVersion"], "1.0.0");
        let message = error["msg"].as_str().expect("CNI error message");
        assert!(message.contains("kubeconfig"));
        assert!(String::from_utf8_lossy(&output.stderr).contains(message));
        assert!(
            !fixture.was_delegated(),
            "explicit DAN failure must preserve path selection"
        );
        assert!(!fixture.directory.join("dans").exists());
    }
}
