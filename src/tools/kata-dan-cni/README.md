# kata-dan-cni

## Overview

kata-dan-cni is a Rust CNI plugin that provides Directly Attachable Network (DAN) support for Kata Containers. It reuses network allocations from Kube-OVN to let Kata virtual machines use host network devices directly.

The plugin handles both ordinary Pods and DAN-enabled Pods through a single CNI entry point. It delegates ordinary Pods to the existing Kube-OVN CNI plugin and creates network devices and DAN JSON files for DAN-enabled Pods. Deployment uses a CNI configuration and a DaemonSet, without requiring changes to the Kube-OVN source code.

## Features

- Supports CNI `ADD`, `DEL`, basic `CHECK`, and `VERSION` commands.
- Supports the `host-tap` backend.
- Parses IPv4, IPv6, and dual-stack network information and generates DAN JSON files.
- Includes scripts for DaemonSet installation, credential synchronization, and uninstallation.

## Platform support

- Supports x86_64 (amd64) Linux systems with the GNU C library.

## Quick start

Complete the [prerequisites](docs/prepare.md), then follow the [installation and configuration guide](docs/setup.md) to deploy the DaemonSet.

To enable DAN networking for a Pod, add the `networking.katacontainers.io/dan: "true"` annotation and select the RuntimeClass specified in the deployment configuration.

## Tests

Run formatting, Clippy and the ordinary tests from the repository root:

```bash
make -C src/tools/kata-dan-cni check
```

Run the real TAP/OVS integration tests on a Linux development VM or CI runner:

```bash
sudo apt-get install -y iproute2 jq openvswitch-switch util-linux
sudo modprobe tun
sudo modprobe openvswitch
sudo -v
make -C src/tools/kata-dan-cni test-integration
```

The integration target compiles the Rust library tests as the invoking user, then
runs them with root privileges in private network, mount and PID namespaces. It
starts its own OVSDB, ovs-vswitchd and system-datapath `br-int`, and writes DAN
files to a temporary directory. Missing dependencies, unavailable isolation and
an empty test selection cause the target to fail.

The integration test injects `FakeProvider` through the existing `Provider` trait into `add_dan`,
using the production `HostTapBackend` and DAN file lifecycle. CHECK and DEL use
the existing CNI command dispatcher, without Kubernetes credentials. This checks
the attachment lifecycle's independence from a concrete Provider; production
Provider selection and Kubernetes API access are outside this test's scope.

Real ovs-vswitchd must assign a positive `ofport`. A test helper simulates only
the OVN controller's `external_ids:ovn-installed=true` acknowledgement, after
checking the test port's ownership. The test verifies the fake allocation's
Guest MAC, IPv4/IPv6 addresses, routes and MTU in the DAN JSON, and the IP addresses
in the CNI result. It also checks the real TAP's MTU and the logical-port name in
the OVS `iface-id`. It then runs CHECK and DEL and verifies that the TAP, OVS rows
and DAN JSON are removed.

This test is ignored by ordinary `make test` and explicitly selected by
`test-integration`. Logs are saved under
`target/kata-dan-integration-logs/run.*`; set `KATA_DAN_TEST_LOG_DIR` to change the
parent directory, or `OVS_SCHEMA` to select an installed OVS schema. Failed tests
also capture interface and OVS database state. The `Kata DAN CNI tests` GitHub
Actions workflow runs this target and its result is required by Gatekeeper for
the static check category.
