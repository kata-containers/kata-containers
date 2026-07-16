# Migrating configuration from the Go runtime to `runtime-rs`

## Go runtime deprecation

Starting with the **4.0.0 release**, the **Rust runtime (`runtime-rs`)** is the
**default** runtime shipped by `kata-deploy` on every architecture that has a
`runtime-rs` build (x86_64, aarch64 and s390x). The default RuntimeClass
therefore resolves to `qemu-runtime-rs` rather than the Go runtime's
`kata-qemu`. ppc64le has no `runtime-rs` build yet and stays on the Go
runtime.

**The Go runtime is deprecated, but it is not removed.** It remains supported
(no new features are being added) and selectable — for example via the
`kata-qemu` RuntimeClass. No fixed removal date has been set: support may end
earlier if maintainers decide it is necessary (for example in response to an
architectural or security concern), or continue longer if there is sustained
interest and maintainer capacity. Migrate any configuration that depends on
Go-runtime-only options (catalogued below) to `runtime-rs`.

## Scope

Kata Containers currently ships two runtime implementations:

- The **Go runtime** (`src/runtime`, historically referred to as `runtime-go`).
- The **Rust runtime** (`src/runtime-rs`, referred to as `runtime-rs`).

Both runtimes are configured through a TOML configuration file, but the set of
options each one understands is **not** identical. The configuration option set
diverged as `runtime-rs` was built, so a configuration file written for the Go
runtime is not guaranteed to behave identically (or even be fully honoured) when
used with `runtime-rs`, and vice versa.

This document catalogues those discrepancies. It currently focuses on the
**QEMU** hypervisor, which is supported by both runtimes and is therefore the
most directly comparable. Other hypervisors and configuration flavors will be
added over time.

The information below was derived from the configuration parsing code, which is
the authoritative source for what each runtime actually reads:

- Go runtime: [`src/runtime/pkg/katautils/config.go`](https://github.com/kata-containers/kata-containers/blob/main/src/runtime/pkg/katautils/config.go)
- `runtime-rs`: [`src/libs/kata-types/src/config`](https://github.com/kata-containers/kata-containers/tree/main/src/libs/kata-types/src/config)
  (shared by all `runtime-rs` hypervisors)

The accompanying configuration templates are:

- Go runtime: [`src/runtime/config/configuration-qemu.toml.in`](https://github.com/kata-containers/kata-containers/blob/main/src/runtime/config/configuration-qemu.toml.in)
- `runtime-rs`: [`src/runtime-rs/config/configuration-qemu-runtime-rs.toml.in`](https://github.com/kata-containers/kata-containers/blob/main/src/runtime-rs/config/configuration-qemu-runtime-rs.toml.in)

## Dropped configuration options

These options are read by the Go runtime but are **not** honoured by
`runtime-rs`. They fall into two groups: options that are **not yet
implemented** (and are expected to be added later), and options that are
**dropped without a replacement**.

### Not yet implemented in `runtime-rs`

These options have no `runtime-rs` equivalent **yet**. Supporting them requires
adding code to `runtime-rs`; they are not intentional removals.

| Option (`[hypervisor.qemu]`) | Purpose in the Go runtime |
| --- | --- |
| `enable_numa` | Expose the host NUMA topology to the guest (1:1 mapping, vCPU binding). |
| `numa_mapping` | Custom mapping of VM NUMA nodes to host NUMA nodes. |
| `net_rate_limiter_bw_max_rate` | Network bandwidth rate limiter (bits/sec). |
| `net_rate_limiter_bw_one_time_burst` | Network bandwidth rate limiter initial burst. |
| `net_rate_limiter_ops_max_rate` | Network operations rate limiter (ops/sec). |
| `net_rate_limiter_ops_one_time_burst` | Network operations rate limiter initial burst. |

!!! note "`disk_rate_limiter_*` is supported in both runtimes"
    Unlike the network rate limiter, the `disk_rate_limiter_*` options
    (`disk_rate_limiter_bw_max_rate`, `disk_rate_limiter_bw_one_time_burst`,
    `disk_rate_limiter_ops_max_rate`, `disk_rate_limiter_ops_one_time_burst`)
    are present in **both** runtimes.

### Dropped without a replacement

| Option (`[hypervisor.qemu]`) | Purpose in the Go runtime |
| --- | --- |
| `firmware_volume` | Path to a split firmware volume (`FIRMWARE_VARS.fd` / `FIRMWARE_CODE.fd`). |
| `measurement_algo` | Measurement algorithm used for SEV-SNP attestation. |
| `vhost_user_reconnect_timeout_sec` | Reconnect timeout for non-server SPDK vhost-user sockets. |
| `use_legacy_serial` | Use a legacy serial device for the guest console. |

The **VMCache** feature is **deprecated** and is not implemented in
`runtime-rs`, so its `[factory]` options have no equivalent:

| Option (`[factory]`) | Purpose in the Go runtime |
| --- | --- |
| `vm_cache_number` | Number of cached VMs created by the VMCache server. |
| `vm_cache_endpoint` | Unix socket address used by VMCache. |

VM templating (`enable_template` / `template_path`) **is** supported by
`runtime-rs`, but lives in a different table — see
[Options that are different but carry the same meaning](#options-that-are-different-but-carry-the-same-meaning).

## Options that are different but carry the same meaning

These options exist in both runtimes but were **renamed**, **moved to a
different table**, or had their **type/unit changed**. They express the same
intent, so they need to be translated when porting a configuration file.

| Go runtime | `runtime-rs` | Difference |
| --- | --- | --- |
| `[hypervisor.qemu] seccompsandbox` | `[hypervisor.qemu] seccomp_sandbox` | Renamed (underscore added). |
| `[hypervisor.qemu] enable_debug` + `hypervisor_loglevel` (numeric `uint32`) | `[hypervisor.qemu] enable_debug` + `log_level` (string: `trace`, `debug`, `info`, `warn`, `error`, `critical`) | Both runtimes support `enable_debug`. `runtime-rs` replaces the numeric `hypervisor_loglevel` with a string `log_level`. |
| `[hypervisor.qemu] hot_plug_vfio` (string port type: `no-port`, `bridge-port`, `root-port`, `switch-port`) | `[hypervisor.qemu] hotplug_vfio_on_root_bus` (`bool`) | Different model for selecting where VFIO devices are hot-plugged. `runtime-rs` pairs `hotplug_vfio_on_root_bus` with `pcie_root_port` / `pcie_switch_port`. |
| `[hypervisor.qemu] block_device_driver` values `virtio-blk`, `virtio-scsi`, `nvdimm` | `[hypervisor.qemu] block_device_driver` values `virtio-blk-pci`, `virtio-blk-ccw`, `virtio-blk-mmio`, `virtio-scsi`, `virtio-pmem` | Same option name, but the accepted driver value strings differ. |
| `[runtime] guest_selinux_label` | `[hypervisor.qemu] selinux_label` | Renamed and moved from the `[runtime]` table to the hypervisor table. |
| `[runtime] create_container_timeout` (seconds) | `[agent.kata] create_container_timeout` (seconds in the file, stored internally in milliseconds) | Moved from the `[runtime]` table to the `[agent]` table. |
| `[agent.kata] dial_timeout` (seconds) | `[agent.kata] dial_timeout_ms` (milliseconds) | Renamed and the unit changed from seconds to milliseconds. |
| `[agent.kata] cdh_api_timeout` (seconds) | `[agent.kata] cdh_api_timeout_ms` (milliseconds) | Renamed and the unit changed from seconds to milliseconds. |
| `[factory]` (top-level table) | `[hypervisor.qemu.factory]` | VM templating moved under the hypervisor table. Only `enable_template` and `template_path` are carried over (the deprecated VMCache fields are dropped — see [Dropped without a replacement](#dropped-without-a-replacement)). |
| `[runtime] experimental_force_guest_pull` (`bool`) | `[runtime] experimental = ["force_guest_pull"]` | Force guest-side image pull is selected through the `experimental` feature list rather than a dedicated boolean. |
| Annotation `io.katacontainers.config.agent.policy` | `[agent.kata] policy` | The Go runtime only accepts an agent policy through the OCI annotation; `runtime-rs` additionally exposes it as a configuration-file option. |
| Annotation `io.katacontainers.config.hypervisor.cc_init_data` (`initdata`) | `[hypervisor.qemu] initdata` | The Go runtime only accepts confidential-computing init data through the annotation; `runtime-rs` additionally exposes it as a configuration-file option. |
| `[runtime] enable_debug` (`bool`) | `[runtime] enable_debug` (`bool`) + `log_level` (string) | Both runtimes support `enable_debug`. `runtime-rs` additionally accepts a string `log_level` for finer-grained runtime logging. |
| `[agent.kata] enable_debug` (`bool`) | `[agent.kata] enable_debug` (`bool`) + `log_level` (string) | Both runtimes support `enable_debug`. `runtime-rs` additionally accepts a string `log_level` for finer-grained agent logging. |

## Options that are `runtime-rs` specific

These options are parsed by `runtime-rs` but have **no equivalent
configuration-file option** in the Go runtime.

### `[hypervisor.qemu]`

| Option | Purpose in `runtime-rs` |
| --- | --- |
| `vm_rootfs_driver` | Dedicated block driver for the VM rootfs (`virtio-pmem`, `virtio-blk-pci`, `virtio-blk-mmio`), separate from `block_device_driver`. |
| `queue_size` | virtio queue size, in bytes, for block devices. |
| `num_queues` | Block device multi-queue count. |
| `network_queues` | Number of `virtio-net` RX/TX queue pairs exposed to the guest. |
| `ctlpath` / `valid_ctlpaths` | Path (and validation list) for the hypervisor control binary. |
| `prefetch_list_path` | Host path to a `prefetch_files.list` for image lazy-loading. |
| `hugepage_type` | Huge page backend type (`hugetlbfs` or `thp`). |
| `virtio_fs_is_dax` | Explicit toggle for the `virtio-fs` DAX window. The Go runtime infers DAX usage from `virtio_fs_cache_size`. |
| `guest_swap_path` | Path of the guest swap device file. |
| `guest_swap_size_percent` | Swap size as a percentage of total guest memory. |
| `guest_swap_create_threshold_secs` | Delay, in seconds, before creating the guest swap device. |
| `rootless_user` (`uid`, `gid`, `groups`, `user_name`) | Structured description of the non-root user used to run the VMM. The Go runtime only exposes the `rootless` boolean. |
| `boot_to_be_template`, `boot_from_template`, `memory_path`, `device_state_path` | Fine-grained VM templating controls. |

!!! warning "Guest swap is rejected by the QEMU plugin"
    `enable_guest_swap` exists in both runtimes, and the `guest_swap_*` tuning
    options above are parsed by `runtime-rs`. However, the QEMU plugin
    currently **rejects** `enable_guest_swap = true` during validation, so
    guest swap is unsupported under QEMU in `runtime-rs` today. Since they
    have no effect for QEMU, the `guest_swap_*` options can be dropped from
    the `runtime-rs` QEMU templates (they remain relevant for hypervisors
    that do support guest swap).

### `[runtime]`

| Option | Purpose in `runtime-rs` |
| --- | --- |
| `name` | Selects the runtime implementation (e.g. `virt_container`). |
| `hypervisor_name` | Selects the hypervisor plugin (e.g. `qemu`). |
| `agent_name` | Selects the agent (e.g. `kata`). |
| `keep_abnormal` | Skip cleanup and keep the sandbox alive on abnormal exit / failed health check, for debugging. |
| `shared_mounts` | Declarations of mounts shared between containers in a sandbox. |
| `use_passfd_io` | Use file-descriptor passthrough for container process I/O. |
| `passfd_listener_port` | Port used by the fd-passthrough I/O feature. |

!!! note "Component selection is `runtime-rs` only"
    The `name` / `hypervisor_name` / `agent_name` selection mechanism is
    specific to `runtime-rs`, which uses a single configuration file to pick
    the runtime, hypervisor and agent components. The Go runtime instead
    selects the hypervisor implicitly from the `[hypervisor.<name>]` table
    that is present.

### `[agent.kata]`

| Option | Purpose in `runtime-rs` |
| --- | --- |
| `server_port` | Agent vsock server port. |
| `log_port` | Agent log vsock port. |
| `passfd_listener_port` | Agent-side port for fd-passthrough I/O. |
| `reconnect_timeout_ms` | Agent reconnect timeout in milliseconds. |
| `health_check_request_timeout_ms` | Timeout for agent health-check requests. |
| `container_pipe_size` | Size of the container I/O pipe. |

### `[agent.kata.mem_agent]`

The entire memory-agent configuration table is specific to `runtime-rs`. It
includes (non-exhaustive):

- `mem_agent_enable` (alias `enable`)
- `memcg_disable`, `memcg_swap`, `memcg_swappiness_max`, `memcg_period_secs`,
  `memcg_period_psi_percent_limit`, `memcg_eviction_psi_percent_limit`,
  `memcg_eviction_run_aging_count_min`
- `compact_disable`, `compact_period_secs`, `compact_period_psi_percent_limit`,
  `compact_psi_percent_limit`, `compact_sec_max`, `compact_order`,
  `compact_threshold`, `compact_force_times`
