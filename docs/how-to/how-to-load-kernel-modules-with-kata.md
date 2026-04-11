# Loading kernel modules in Kata Containers

This document describes how to load kernel modules inside Kata Containers guest VM.

## Overview

The kernel modules feature allows you to load specific kernel modules into the guest VM kernel when a sandbox is created. This is useful when your containerized applications require specific kernel functionality that is not built into the guest kernel.

**How it works:**

1. You specify kernel modules and their parameters via configuration file or OCI annotations
2. The Kata runtime passes this information to the Kata Agent through gRPC during sandbox creation
3. The Kata Agent loads the modules using `modprobe(8)`, which automatically resolves module dependencies

**Failure conditions:**

The sandbox will fail to start if:

- A kernel module is specified but `modprobe(8)` is not installed in the guest, or it fails to load the module
- The module is not available in the guest or doesn't meet guest kernel requirements (architecture, version, etc.)

## Configuration Methods

- [Using Kata Configuration file](#using-kata-configuration-file)
- [Using annotations](#using-annotations)

## Using Kata Configuration file

> **Note**: Use this method when you need the kernel modules loaded for all containers. For per-pod configuration, use annotations instead.

The `kernel_modules` option accepts a list of kernel modules with their parameters. Each list element specifies a module name followed by space-separated parameters.

### Configuration Format

**For runtime-go** (`configuration-qemu.toml`, etc.):

```toml
[agent.kata]
kernel_modules = ["e1000e InterruptThrottleRate=3000,3000,3000 EEE=1", "i915"]
```

**For runtime-rs** (`configuration-qemu-runtime-rs.toml`, etc.):

```toml
[agent.kata]
kernel_modules = ["e1000e InterruptThrottleRate=3000,3000,3000 EEE=1", "i915"]
```

### Example

The following example loads two modules:

- `e1000e` with parameters `InterruptThrottleRate=3000,3000,3000` and `EEE=1`
- `i915` with no parameters

```toml
kernel_modules = ["e1000e InterruptThrottleRate=3000,3000,3000 EEE=1", "i915"]
```

### Limitations

- Write access to the Kata configuration file is required
- All containers will use the same module list, even if some containers don't need them
- Configuration changes require service restart to take effect

## Using annotations

Annotations provide a way to specify kernel modules per pod, which is more flexible than the configuration file approach.

### Annotation Key

```
io.katacontainers.config.agent.kernel_modules
```

### Format

The annotation value uses **semicolon (`;`)** as the separator between modules. Each module specification consists of:

- Module name (first word)
- Parameters (subsequent words, space-separated)

Example: `"e1000e EEE=1; i915 enable_ppgtt=0"`

### Kubernetes Example

The following example creates two pods, where only `pod1` will have the kernel modules `e1000e` and `i915` loaded:

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: pod1
  annotations:
    io.katacontainers.config.agent.kernel_modules: "e1000e EEE=1; i915"
spec:
  runtimeClassName: kata
  containers:
  - name: c1
    image: busybox
    command:
      - sh
    stdin: true
    tty: true

---
apiVersion: v1
kind: Pod
metadata:
  name: pod2
spec:
  runtimeClassName: kata
  containers:
  - name: c2
    image: busybox
    command:
      - sh
    stdin: true
    tty: true
```

> **Note**: To pass annotations to Kata containers, [CRI-O must be configured correctly](how-to-set-sandbox-config-kata.md#cri-o-configuration)

## Technical Details

### Data Flow

```
    Configuration File / Annotation
            │
            ▼
    SandboxConfig.AgentConfig.KernelModules
            │
            ▼
    Converted to gRPC KernelModule messages
            │
            ▼
    CreateSandboxRequest sent to Agent
            │
            ▼
    Agent executes modprobe in guest VM
```

### Implementation in Runtimes

**runtime-go:**

- Config parsing: `src/runtime/pkg/katautils/config.go`
- Annotation handling: `src/runtime/pkg/oci/utils.go` (`addAgentConfigOverrides()`)
- Module parsing: `src/runtime/virtcontainers/kata_agent.go` (`setupKernelModules()`)

**runtime-rs:**

- Config structure: `src/libs/kata-types/src/config/agent.rs`
- Annotation handling: `src/libs/kata-types/src/annotations/mod.rs` (`update_config_by_annotation()`)
- Module parsing: `src/runtime-rs/crates/agent/src/types.rs` (`KernelModule::set_kernel_modules()`)

## Debugging

To verify kernel modules are loaded in the guest VM:

```bash
# Inside the container, run:
lsmod | grep <module_name>

# Or check modprobe output in guest VM journal
```

If module loading fails, check:

1. Module is available in guest kernel modules directory (`/lib/modules/$(uname -r)`)
2. Module dependencies are satisfied
3. Guest kernel version matches module requirements
