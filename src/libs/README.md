# Kata Containers Library Crates

The `src/libs` directory hosts library crates shared by multiple Kata Containers components. These libraries provide common utilities, data types, and protocol definitions to facilitate development and maintain consistency across the project.

## Library Crates

| Library | Description |
|---------|-------------|
| [kata-types](kata-types/) | Constants, data types, and configuration structures shared by Kata Containers components |
| [kata-sys-util](kata-sys-util/) | System utilities: CPU, device, filesystem, hooks, K8s, mount, netns, NUMA, PCI, protection, spec validation |
| [protocols](protocols/) | ttrpc protocol definitions for agent, health, remote, CSI, OCI, confidential data hub |
| [runtime-spec](runtime-spec/) | OCI runtime spec data structures and constants |
| [shim-interface](shim-interface/) | Shim management interface with RESTful API over Unix domain socket |
| [logging](logging/) | Slog-based logging with JSON output and systemd journal support |
| [safe-path](safe-path/) | Safe path resolution to prevent symlink and TOCTOU attacks |
| [mem-agent](mem-agent/) | Memory management agent: memcg, compact, PSI monitoring |
| [test-utils](test-utils/) | Test macros for root/non-root privileges and KVM accessibility |

## Details

### kata-types

Core types and configurations including:

- Annotations for CRI-containerd, CRI-O, dockershim
- Hypervisor configurations (QEMU, Cloud Hypervisor, Firecracker, Dragonball)
- Agent and runtime configurations
- Kubernetes-specific utilities

### kata-sys-util

System-level utilities:

- `cpu`: CPU information and affinity
- `device`: Device management
- `fs`: Filesystem operations
- `hooks`: Hook execution
- `k8s`: Kubernetes utilities
- `mount`: Mount operations
- `netns`: Network namespace handling
- `numa`: NUMA topology
- `pcilibs`: PCI device access
- `protection`: Hardware protection features
- `spec`: OCI spec loading
- `validate`: Input validation

### protocols

Generated ttrpc protocol bindings:

- `agent`: Kata agent API
- `health`: Health check service
- `remote`: Remote hypervisor API
- `csi`: Container storage interface
- `oci`: OCI specifications
- `confidential_data_hub`: Confidential computing support

Features: `async` for async ttrpc, `with-serde` for serde support.

### runtime-spec

OCI runtime specification types:

- `ContainerState`: Creating, Created, Running, Stopped, Paused
- `State`: Container state with version, id, status, pid, bundle, annotations
- Namespace constants: pid, network, mount, ipc, user, uts, cgroup

### shim-interface

Shim management service interface:

- RESTful API over Unix domain socket (`/run/kata/<sid>/shim-monitor.sock`)
- `MgmtClient` for HTTP requests to shim management server
- Sandbox ID resolution with prefix matching

### logging

Slog-based logging framework:

- JSON output to file or stdout
- systemd journal support
- Runtime log level filtering per component/subsystem
- Async drain for thread safety

### safe-path

Secure filesystem path handling:

- `scoped_join()`: Safely join paths under a root directory
- `scoped_resolve()`: Resolve paths constrained by root
- `PinnedPathBuf`: TOCTOU-safe path reference
- `ScopedDirBuilder`: Safe directory creation

### mem-agent

Memory management for containers:

- `memcg`: Memory cgroup configuration and monitoring
- `compact`: Memory compaction control
- `psi`: Pressure stall information monitoring
- Async runtime with configurable policies

### test-utils

Testing utilities:

- `skip_if_root!`: Skip test if running as root
- `skip_if_not_root!`: Skip test if not running as root
- `skip_if_kvm_unaccessable!`: Skip test if KVM is unavailable
- `assert_result!`: Assert expected vs actual results

## License

All crates are licensed under Apache-2.0.
