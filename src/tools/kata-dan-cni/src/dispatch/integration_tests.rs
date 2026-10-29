// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

//! Real kernel TAP/OVS tests, run only by tests/run-integration.sh.
//! Provider injection exercises the production attachment lifecycle without a
//! Kubernetes API. Only OVN's installation acknowledgement is simulated.

use std::fs;
use std::process::{Command, Output};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use backend_host_tap::HostTapBackend;
use dan_core::{DanEndpoint, HostPortBinding, LogicalPort};
use provider_fake::FakeProvider;
use serde_json::json;

use super::*;
use crate::test_support::{allocation, TestDir};

fn command(program: &str, args: &[&str]) -> Output {
    Command::new(program)
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("cannot run {program} {args:?}: {error}"))
}

fn stdout(output: Output) -> String {
    assert!(
        output.status.success(),
        "command failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("UTF-8 command output")
        .trim()
        .to_owned()
}

fn ovs(args: &[&str]) -> String {
    stdout(command(
        "ovs-vsctl",
        &[
            &["--timeout=5", "--db=unix:/run/openvswitch/db.sock"][..],
            args,
        ]
        .concat(),
    ))
}

fn link(name: &str) -> Option<Value> {
    let output = stdout(command("ip", &["-details", "-json", "link", "show"]));
    let links: Vec<Value> = serde_json::from_str(&output).expect("kernel link JSON");
    links.into_iter().find(|info| info["ifname"] == name)
}

fn field(table: &str, name: &str, column: &str) -> String {
    ovs(&["get", table, name, column])
        .trim_matches('"')
        .to_owned()
}

/// Real ovs-vswitchd assigns ofport. In place of an OVN controller, acknowledge
/// installation only for this fixture's interface after checking its ownership.
struct InstallationAck {
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl InstallationAck {
    fn start(tap: String, iface_id: String) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker = thread::spawn(move || {
            while !worker_stop.load(Ordering::Relaxed) {
                let output = command(
                    "ovs-vsctl",
                    &[
                        "--timeout=2",
                        "--db=unix:/run/openvswitch/db.sock",
                        "--if-exists",
                        "get",
                        "Interface",
                        &tap,
                        "ofport",
                    ],
                );
                let ready = output.status.success()
                    && String::from_utf8_lossy(&output.stdout)
                        .trim()
                        .parse::<i64>()
                        .is_ok_and(|port| port > 0);
                if ready {
                    let owner = field("Interface", &tap, "external_ids:kata-dan-owner");
                    assert!(!owner.is_empty() && owner != "[]", "missing owner");
                    assert_eq!(field("Port", &tap, "external_ids:kata-dan-owner"), owner);
                    assert_eq!(field("Interface", &tap, "external_ids:iface-id"), iface_id);
                    let info = link(&tap).expect("owned TAP exists before acknowledgement");
                    assert_eq!(info["ifalias"], format!("kata-dan:{owner}"));
                    ovs(&["set", "Interface", &tap, "external_ids:ovn-installed=true"]);
                    return;
                }
                thread::sleep(Duration::from_millis(25));
            }
        });
        Self {
            stop,
            worker: Some(worker),
        }
    }
}

impl Drop for InstallationAck {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let result = worker.join();
            if !thread::panicking() {
                result.expect("OVN acknowledgement helper");
            }
        }
    }
}

#[test]
#[ignore = "requires the isolated environment prepared by make test-integration"]
fn fake_provider_lifecycle() {
    assert_eq!(
        std::env::var("KATA_DAN_INTEGRATION").as_deref(),
        Ok("1"),
        "use make test-integration to prepare isolated TAP/OVS namespaces"
    );
    let directory = TestDir::new();
    let identity = AttachmentIdentity {
        container_id: "fake-provider-sandbox".into(),
        if_name: "eth0".into(),
    };
    let allocation = allocation();
    let port = LogicalPort {
        name: "alternative-provider.port".into(),
        host_binding: HostPortBinding::default(),
    };
    let provider: Box<dyn Provider> = Box::new(FakeProvider::new(allocation.clone(), port.clone()));
    let backend = HostTapBackend::new();
    let store = DanFile::new(directory.0.join("dans"));
    let mut config_json = json!({
        "cniVersion": "1.0.0",
        "name": "fake-provider-integration",
        "type": "kata-dan-cni",
        "backend": {"type": "host-tap"},
        "danConfDir": directory.0.join("dans"),
        "runtimeConfig": {
            "io.kubernetes.cri.pod-annotations": {
                "networking.katacontainers.io/dan": "true"
            }
        },
        // CHECK and DEL must use the existing lifecycle without a concrete
        // Provider selector, Kubernetes credentials or delegate execution.
        "delegate": {"name": "unused", "plugins": [{"type": "unused-delegate"}]}
    });
    let config = NetConf::parse(&serde_json::to_vec(&config_json).expect("CNI config"))
        .expect("valid config");
    let device = backend
        .prepare(&NetworkContext {
            container_id: identity.container_id.clone(),
            selection: NetworkSelection {
                network_name: config.cni.name.clone(),
                if_name: identity.if_name.clone(),
            },
            allocation: allocation.clone(),
            logical_port: port.clone(),
        })
        .expect("plan real host-tap endpoint");
    let DanEndpoint::HostTap { tap_name } = &device.endpoint;
    let ack = InstallationAck::start(tap_name.clone(), port.name.clone());

    let result = runtime()
        .expect("test runtime")
        .block_on(add_dan(
            &identity,
            &config,
            provider.as_ref(),
            PodIdentity {
                namespace: "integration".into(),
                name: "fake-provider".into(),
                uid: "fake-provider-pod".into(),
            },
            &backend,
            &store,
        ))
        .expect("ADD with FakeProvider and unchanged host-tap backend");
    drop(ack);

    let path = directory.0.join("dans/fake-provider-sandbox.json");
    let saved: Value = serde_json::from_slice(&fs::read(&path).expect("sandbox DAN JSON"))
        .expect("runtime-readable DAN JSON");
    assert_eq!(store.read(&identity).expect("DAN device"), device);
    assert_eq!(saved["devices"].as_array().expect("devices").len(), 1);
    let saved_device = &saved["devices"][0];
    assert_eq!(saved_device["guest_mac"], "02:00:00:00:00:42");
    assert_eq!(saved_device["device"]["type"], "host-tap");
    assert_eq!(saved_device["device"]["tap_name"], tap_name.as_str());
    assert_eq!(
        saved_device["network_info"]["interface"]["ip_addresses"],
        json!(["10.42.0.9/24", "fd42::9/64"])
    );
    assert_eq!(
        saved_device["network_info"]["interface"]["mtu"],
        allocation.mtu
    );
    assert_eq!(
        saved_device["network_info"]["routes"],
        json!([
            {"dest": "0.0.0.0/0", "gateway": "10.42.0.1"},
            {"dest": "::/0", "gateway": "fd42::1"},
            {"dest": "10.43.0.0/16", "gateway": ""}
        ])
    );
    let tap = link(tap_name).expect("real kernel TAP");
    assert_eq!(tap["mtu"], allocation.mtu);
    assert_eq!(ovs(&["port-to-br", tap_name]), "br-int");
    assert_eq!(
        field("Interface", tap_name, "external_ids:iface-id"),
        port.name
    );
    assert_eq!(result["interfaces"][0]["name"], tap_name.as_str());
    let result_addresses: Vec<_> = result["ips"]
        .as_array()
        .expect("CNI IPs")
        .iter()
        .map(|ip| ip["address"].as_str().expect("CNI IP address"))
        .collect();
    assert_eq!(result_addresses, ["10.42.0.9/24", "fd42::9/64"]);

    config_json["prevResult"] = result;
    let mut environment = CniEnvironment {
        command: CniCommand::Check,
        container_id: Some(identity.container_id.clone()),
        netns: Some("/proc/self/ns/net".into()),
        if_name: Some(identity.if_name.clone()),
        cni_args: None,
        cni_path: vec![],
    };
    assert_eq!(
        dispatch(
            environment.clone(),
            &serde_json::to_vec(&config_json).expect("CHECK config"),
        )
        .expect("CHECK through the unchanged CNI dispatcher"),
        CommandOutput::Empty
    );
    environment.command = CniCommand::Del;
    environment.netns = None;
    assert_eq!(
        dispatch(
            environment,
            &serde_json::to_vec(&config_json).expect("DEL config"),
        )
        .expect("DEL through the unchanged CNI dispatcher"),
        CommandOutput::Empty
    );

    assert!(link(tap_name).is_none(), "TAP remains after DEL");
    for table in ["Port", "Interface"] {
        assert_eq!(
            ovs(&["--if-exists", "get", table, tap_name, "_uuid"]),
            "",
            "{table} remains after DEL"
        );
    }
    assert!(!path.exists(), "DAN JSON remains after DEL");
}
