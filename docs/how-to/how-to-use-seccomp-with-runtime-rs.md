## Introduction

To enhance security, Kata Containers supports using seccomp to restrict the hypervisor's system calls. Previously, this was only supported for a subset of hypervisors in runtime-go. Now, the runtime-rs also supports seccomp. This document describes how to enable/disable the seccomp feature for the corresponding hypervisor in runtime-rs.

## Pre-requisites

1. Ensure your system's kernel supports **seccomp**.
2. Confirm that each of the following virtual machines can run correctly on your system.

## Configure seccomp

With the exception of `qemu`, seccomp is enabled by default for all other supported hypervisors. Their corresponding built-in functionalities are also enabled by default.

### QEMU

As with runtime-go, you need to modify the following in your **configuration file**. These parameters will be passed directly to the `qemu` startup command line. For more details on the parameters, you can refer to: [https://www.qemu.org/docs/master/system/qemu-manpage.html](https://www.qemu.org/docs/master/system/qemu-manpage.html)

``` toml
# Qemu seccomp sandbox feature
# comma-separated list of seccomp sandbox features to control the syscall access.
# For example, `seccompsandbox= "on,obsolete=deny,spawn=deny,resourcecontrol=deny"`
# Note: "elevateprivileges=deny" doesn't work with daemonize option, so it's removed from the seccomp sandbox
# Another note: enabling this feature may reduce performance, you may enable
# /proc/sys/net/core/bpf_jit_enable to reduce the impact. see https://man7.org/linux/man-pages/man8/bpfc.8.html
seccompsandbox="on,obsolete=deny,spawn=deny,resourcecontrol=deny"
```
### Cloud Hypervisor, Firecracker and Dragonball

The **seccomp** functionality is enabled by default for the following three hypervisors: `cloud hypervisor`, `firecracker`, and `dragonball`.

The seccomp rules for `cloud hypervisor` and `firecracker` are built directly into their executable files. For `dragonball`, the relevant configuration is currently located at `src/runtime-rs/crates/hypervisor/src/dragonball/seccomp.rs`.

To disable this functionality for these hypervisors, you can modify the following configuration options in your **configuration file**.

``` toml
# Disable the 'seccomp' feature from Cloud Hypervisor, firecracker or dragonball, default false
disable_seccomp = true
```

## Implementation details

For `qemu`, `cloud hypervisor`, and `firecracker`, their **seccomp** functionality is built into the respective executable files you are using. **runtime-rs** simply provides command-line arguments for their launch based on the configuration file.

For `dragonball`, a set of allowed system calls is currently provided for the entire **runtime-rs** process, and the process is prevented from using any system calls outside of this whitelist. As mentioned above, this set is located at `src/runtime-rs/crates/hypervisor/src/dragonball/seccomp.rs`.
