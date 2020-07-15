# Per-Pod Kata Configurations

Kata Containers gives users freedom to customize at per-pod level, by setting
a wide range of Kata specific annotations in the pod specification.

# Kata Configuration Annotations
There are several kinds of Kata configurations and they are listed below.

## Global Options
| Key | Value Type | Comments |
|-------| ----- | ----- |
| `io.katacontainers.config_path` | string | Kata config file location that overrides the default config paths |
| `io.katacontainers.pkg.oci.bundle_path` | string | OCI bundle path |
| `io.katacontainers.pkg.oci.container_type`| string | OCI container type. Only accepts `pod_container` and `pod_sandbox` |

## Runtime Options
| Key | Value Type | Comments |
|-------| ----- | ----- |
| `io.katacontainers.config.runtime.experimental` | `boolean` | determines if experimental features enabled |
| `io.katacontainers.config.runtime.disable_guest_seccomp`| `boolean` | determines if `seccomp` should be applied inside guest |
| `io.katacontainers.config.runtime.disable_new_netns` | `boolean` | determines if a new netns is created for the hypervisor process |
| `io.katacontainers.config.runtime.internetworking_model` | string| determines how the VM should be connected to the container network interface. Valid values are `macvtap`, `tcfilter` and `none` |
| `io.katacontainers.config.runtime.sandbox_cgroup_only`| `boolean` | determines if Kata processes are managed only in sandbox cgroup |

## Agent Options
| Key | Value Type | Comments |
|-------| ----- | ----- |
| `io.katacontainers.config.agent.enable_tracing` | `boolean` | enable tracing for the agent |
| `io.katacontainers.config.agent.kernel_modules` | string | the list of kernel modules and their parameters that will be loaded in the guest kernel. Semicolon separated list of kernel modules and their parameters. These modules will be loaded in the guest kernel using `modprobe`(8). E.g., `e1000e InterruptThrottleRate=3000,3000,3000 EEE=1; i915 enable_ppgtt=0` |
| `io.katacontainers.config.agent.trace_mode` | string | the trace mode for the agent |
| `io.katacontainers.config.agent.trace_type` | string | the trace type for the agent |

## Hypervisor Options
| Key | Value Type | Comments |
|-------| ----- | ----- |
| `io.katacontainers.config.hypervisor.asset_hash_type` | string | the hash type used for assets verification, default is `sha512` |
| `io.katacontainers.config.hypervisor.block_device_cache_direct` | `boolean` | Denotes whether use of `O_DIRECT` (bypass the host page cache) is enabled |
| `io.katacontainers.config.hypervisor.block_device_cache_noflush` | `boolean` | Denotes whether flush requests for the device are ignored |
| `io.katacontainers.config.hypervisor.block_device_cache_set` | `boolean` | cache-related options will be set to block devices or not |
| `io.katacontainers.config.hypervisor.block_device_driver` | string | the driver to be used for block device, valid values are `virtio-blk`, `virtio-scsi`, `nvdimm`|
| `io.katacontainers.config.hypervisor.default_max_vcpus` | uint32| the maximum number of vCPUs allocated for the VM by the hypervisor |
| `io.katacontainers.config.hypervisor.default_memory` | uint32| the memory assigned for a VM by the hypervisor in `MiB` |
| `io.katacontainers.config.hypervisor.default_vcpus` | uint32| the default vCPUs assigned for a VM by the hypervisor |
| `io.katacontainers.config.hypervisor.disable_block_device_use` | `boolean` | disallow a block device from being used |
| `io.katacontainers.config.hypervisor.disable_vhost_net` | `boolean` | specify if `vhost-net` is not available on the host |
| `io.katacontainers.config.hypervisor.enable_hugepages` | `boolean` | if the memory should be `pre-allocated` from huge pages |
| `io.katacontainers.config.hypervisor.enable_iothreads` | `boolean`| enable IO to be processed in a separate thread. Supported currently for virtio-`scsi` driver |
| `io.katacontainers.config.hypervisor.enable_mem_prealloc` | `boolean` | the memory space used for `nvdimm` device by the hypervisor |
| `io.katacontainers.config.hypervisor.enable_swap` | `boolean` | enable swap of VM memory |
| `io.katacontainers.config.hypervisor.entropy_source` | string| the path to a host source of entropy (`/dev/random`, `/dev/urandom` or real hardware RNG device) |
| `io.katacontainers.config.hypervisor.file_mem_backend` | string | file based memory backend root directory |
| `io.katacontainers.config.hypervisor.firmware_hash` | string | container firmware SHA-512 hash value |
| `io.katacontainers.config.hypervisor.firmware` | string | the guest firmware that will run the container VM |
| `io.katacontainers.config.hypervisor.guest_hook_path` | string | the path within the VM that will be used for drop in hooks |
| `io.katacontainers.config.hypervisor.hotplug_vfio_on_root_bus` | `boolean` | indicate if devices need to be hotplugged on the root bus instead of a bridge|
| `io.katacontainers.config.hypervisor.hypervisor_hash` | string | container hypervisor binary SHA-512 hash value |
| `io.katacontainers.config.hypervisor.image_hash` | string | container guest image SHA-512 hash value |
| `io.katacontainers.config.hypervisor.image` | string | the guest image that will run in the container VM |
| `io.katacontainers.config.hypervisor.initrd_hash` | string | container guest initrd SHA-512 hash value |
| `io.katacontainers.config.hypervisor.initrd` | string | the guest initrd image that will run in the container VM |
| `io.katacontainers.config.hypervisor.jailer_hash` | string | container jailer SHA-512 hash value |
| `io.katacontainers.config.hypervisor.jailer_path` | string | the jailer that will constrain the container VM |
| `io.katacontainers.config.hypervisor.kernel_hash` | string | container kernel image SHA-512 hash value |
| `io.katacontainers.config.hypervisor.kernel_params` | string | additional guest kernel parameters |
| `io.katacontainers.config.hypervisor.kernel` | string | the kernel used to boot the container VM |
| `io.katacontainers.config.hypervisor.machine_accelerators` | string | machine specific accelerators for the hypervisor |
| `io.katacontainers.config.hypervisor.machine_type` | string | the type of machine being emulated by the hypervisor |
| `io.katacontainers.config.hypervisor.memory_offset` | uint32| the memory space used for `nvdimm` device by the hypervisor |
| `io.katacontainers.config.hypervisor.memory_slots` | uint32| the memory slots assigned to the VM by the hypervisor |
| `io.katacontainers.config.hypervisor.msize_9p` | uint32 | the `msize` for 9p shares |
| `io.katacontainers.config.hypervisor.path` | string | the hypervisor that will run the container VM |
| `io.katacontainers.config.hypervisor.shared_fs` | string | the shared file system type, either `virtio-9p` or `virtio-fs` |
| `io.katacontainers.config.hypervisor.use_vsock` | `boolean` | specify use of `vsock` for agent communication |
| `io.katacontainers.config.hypervisor.virtio_fs_cache_size` | uint32 | virtio-fs DAX cache size in `MiB` |
| `io.katacontainers.config.hypervisor.virtio_fs_cache` | string | the cache mode for virtio-fs, valid values are `always`, `auto` and `none` |
| `io.katacontainers.config.hypervisor.virtio_fs_daemon` | string | virtio-fs `vhost-user` daemon path |
| `io.katacontainers.config.hypervisor.virtio_fs_extra_args` | string | extra options passed to `virtiofs` daemon |

# CRI Configuration

In case of CRI-O, all annotations specified in the pod spec are passed down to Kata.

For containerd, annotations specified in the pod spec are passed down to Kata
starting with version `1.3.0` of containerd. Additionally, extra configuration is
needed for containerd, by providing a `pod_annotations` field in the containerd config
file.  The `pod_annotations` field is a list of annotations that can be passed down to
Kata as OCI annotations. It supports golang match patterns. Since annotations supported
by Kata follow the pattern `io.katacontainers.*`, the following configuration would work
for passing annotations to Kata from containerd:

```
$ cat /etc/containerd/config
....

[plugins.cri.containerd.runtimes.kata]
           runtime_type = "io.containerd.runc.v1"
           pod_annotations = ["io.katacontainers.*"]
           [plugins.cri.containerd.runtimes.kata.options]
             BinaryName = "/usr/bin/kata-runtime"
....

```

Additional documentation on the above configuration can be found in the 
[containerd docs](https://github.com/containerd/cri/blob/8d5a8355d07783ba2f8f451209f6bdcc7c412346/docs/config.md).

# Example - Using annotations

As mentioned above, not all containers need the same modules, therefore using
the configuration file for specifying the list of kernel modules per POD can
be a pain. Unlike the configuration file, annotations provide a way to specify
custom configurations per POD.

The list of kernel modules and parameters can be set using the annotation
`io.katacontainers.config.agent.kernel_modules` as a semicolon separated
list, where the first word of each element is considered as the module name and
the rest as its parameters.

Also users might want to enable guest `seccomp` to provide better isolation with a
little performance sacrifice. The annotation
`io.katacontainers.config.runtime.disable_guest_seccomp` can used for such purpose.

In the following example two PODs are created, but the kernel modules `e1000e`
and `i915` are inserted only in the POD `pod1`. Also guest `seccomp` is only enabled
in the POD `pod2`.


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
  annotations:
    io.katacontainers.config.runtime.disable_guest_seccomp: false
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
