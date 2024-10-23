# Per-Pod Kata Configurations

Kata Containers gives users freedom to customize at per-pod level, by setting
a wide range of Kata specific annotations in the pod specification.

Some annotations may be [restricted](#restricted-annotations) by the
configuration file for security reasons, notably annotations that could lead the
runtime to execute programs on the host. Such annotations are marked with _(R)_ in
the tables below.

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
| `io.katacontainers.config.runtime.enable_pprof` | `boolean` | enables Golang `pprof` for `containerd-shim-kata-v2` process |
| `io.katacontainers.config.runtime.create_container_timeout` | `uint64` | the timeout for create a container in `seconds`, default is `60` |

## Agent Options
| Key | Value Type | Comments |
|-------| ----- | ----- |
| `io.katacontainers.config.agent.enable_tracing` | `boolean` | enable tracing for the agent |
| `io.katacontainers.config.agent.container_pipe_size` | uint32 | specify the size of the std(in/out) pipes created for containers |
| `io.katacontainers.config.agent.kernel_modules` | string | the list of kernel modules and their parameters that will be loaded in the guest kernel. Semicolon separated list of kernel modules and their parameters. These modules will be loaded in the guest kernel using `modprobe`(8). E.g., `e1000e InterruptThrottleRate=3000,3000,3000 EEE=1; i915 enable_ppgtt=0` |
| `io.katacontainers.config.agent.cdh_api_timeout` | uint32 | timeout in second for Confidential Data Hub (CDH) API service, default is `50` |

## Hypervisor Options
| Key | Value Type | Comments |
|-------| ----- | ----- |
| `io.katacontainers.config.hypervisor.asset_hash_type` | string | the hash type used for assets verification, default is `sha512` |
| `io.katacontainers.config.hypervisor.block_device_cache_direct` | `boolean` | Denotes whether use of `O_DIRECT` (bypass the host page cache) is enabled |
| `io.katacontainers.config.hypervisor.block_device_cache_noflush` | `boolean` | Denotes whether flush requests for the device are ignored |
| `io.katacontainers.config.hypervisor.block_device_cache_set` | `boolean` | cache-related options will be set to block devices or not |
| `io.katacontainers.config.hypervisor.block_device_driver` | string | the driver to be used for block device, valid values are `virtio-blk`, `virtio-scsi`, `nvdimm`|
| `io.katacontainers.config.hypervisor.cpu_features` | `string` | Comma-separated list of CPU features to pass to the CPU (QEMU) |
| `io.katacontainers.config.hypervisor.default_max_vcpus` | uint32| the maximum number of vCPUs allocated for the VM by the hypervisor |
| `io.katacontainers.config.hypervisor.default_memory` | uint32| the memory assigned for a VM by the hypervisor in `MiB` |
| `io.katacontainers.config.hypervisor.default_vcpus` | float32| the default vCPUs assigned for a VM by the hypervisor |
| `io.katacontainers.config.hypervisor.disable_block_device_use` | `boolean` | disallow a block device from being used |
| `io.katacontainers.config.hypervisor.disable_image_nvdimm` | `boolean` | specify if a `nvdimm` device should be used as rootfs for the guest (QEMU) |
| `io.katacontainers.config.hypervisor.disable_vhost_net` | `boolean` | specify if `vhost-net` is not available on the host |
| `io.katacontainers.config.hypervisor.enable_hugepages` | `boolean` | if the memory should be `pre-allocated` from huge pages |
| `io.katacontainers.config.hypervisor.enable_iommu_platform` | `boolean` | enable `iommu` on CCW devices (QEMU s390x) |
| `io.katacontainers.config.hypervisor.enable_iommu` | `boolean` | enable `iommu` on Q35 (QEMU x86_64) |
| `io.katacontainers.config.hypervisor.enable_iothreads` | `boolean`| enable IO to be processed in a separate thread. Supported currently for virtio-`scsi` driver |
| `io.katacontainers.config.hypervisor.enable_mem_prealloc` | `boolean` | the memory space used for `nvdimm` device by the hypervisor |
| `io.katacontainers.config.hypervisor.enable_vhost_user_store` | `boolean` | enable vhost-user storage device (QEMU) |
| `io.katacontainers.config.hypervisor.vhost_user_reconnect_timeout_sec` | `string`| the timeout for reconnecting vhost user socket (QEMU) 
| `io.katacontainers.config.hypervisor.enable_virtio_mem` | `boolean` | enable virtio-mem (QEMU) |
| `io.katacontainers.config.hypervisor.entropy_source` (R) | string| the path to a host source of entropy (`/dev/random`, `/dev/urandom` or real hardware RNG device) |
| `io.katacontainers.config.hypervisor.file_mem_backend` (R) | string | file based memory backend root directory |
| `io.katacontainers.config.hypervisor.firmware_hash` | string | container firmware SHA-512 hash value |
| `io.katacontainers.config.hypervisor.firmware` | string | the guest firmware that will run the container VM |
| `io.katacontainers.config.hypervisor.firmware_volume_hash` | string | container firmware volume SHA-512 hash value |
| `io.katacontainers.config.hypervisor.firmware_volume` | string | the guest firmware volume that will be passed to the container VM |
| `io.katacontainers.config.hypervisor.guest_hook_path` | string | the path within the VM that will be used for drop in hooks |
| `io.katacontainers.config.hypervisor.hotplug_vfio_on_root_bus` | `boolean` | indicate if devices need to be hotplugged on the root bus instead of a bridge|
| `io.katacontainers.config.hypervisor.hypervisor_hash` | string | container hypervisor binary SHA-512 hash value |
| `io.katacontainers.config.hypervisor.image_hash` | string | container guest image SHA-512 hash value |
| `io.katacontainers.config.hypervisor.image` | string | the guest image that will run in the container VM |
| `io.katacontainers.config.hypervisor.initrd_hash` | string | container guest initrd SHA-512 hash value |
| `io.katacontainers.config.hypervisor.initrd` | string | the guest initrd image that will run in the container VM |
| `io.katacontainers.config.hypervisor.jailer_hash` | string | container jailer SHA-512 hash value |
| `io.katacontainers.config.hypervisor.jailer_path` (R) | string | the jailer that will constrain the container VM |
| `io.katacontainers.config.hypervisor.kernel_hash` | string | container kernel image SHA-512 hash value |
| `io.katacontainers.config.hypervisor.kernel_params` | string | additional guest kernel parameters |
| `io.katacontainers.config.hypervisor.kernel` | string | the kernel used to boot the container VM |
| `io.katacontainers.config.hypervisor.machine_accelerators` | string | machine specific accelerators for the hypervisor |
| `io.katacontainers.config.hypervisor.machine_type` | string | the type of machine being emulated by the hypervisor |
| `io.katacontainers.config.hypervisor.memory_offset` | uint64| the memory space used for `nvdimm` device by the hypervisor |
| `io.katacontainers.config.hypervisor.memory_slots` | uint32| the memory slots assigned to the VM by the hypervisor |
| `io.katacontainers.config.hypervisor.msize_9p` | uint32 | the `msize` for 9p shares |
| `io.katacontainers.config.hypervisor.path` | string | the hypervisor that will run the container VM |
| `io.katacontainers.config.hypervisor.pcie_root_port` | specify the number of PCIe Root Port devices. The PCIe Root Port device is used to hot-plug a PCIe device (QEMU) |
| `io.katacontainers.config.hypervisor.shared_fs` | string | the shared file system type, either `virtio-9p` or `virtio-fs` |
| `io.katacontainers.config.hypervisor.use_vsock` | `boolean` | specify use of `vsock` for agent communication |
| `io.katacontainers.config.hypervisor.vhost_user_store_path` (R) | `string` | specify the directory path where vhost-user devices related folders, sockets and device nodes should be (QEMU) |
| `io.katacontainers.config.hypervisor.virtio_fs_cache_size` | uint32 | virtio-fs DAX cache size in `MiB` |
| `io.katacontainers.config.hypervisor.virtio_fs_cache` | string | the cache mode for virtio-fs, valid values are `always`, `auto` and `never` |
| `io.katacontainers.config.hypervisor.virtio_fs_daemon` | string | virtio-fs `vhost-user` daemon path |
| `io.katacontainers.config.hypervisor.virtio_fs_extra_args` | string | extra options passed to `virtiofs` daemon |
| `io.katacontainers.config.hypervisor.enable_guest_swap` | `boolean` | enable swap in the guest |
| `io.katacontainers.config.hypervisor.use_legacy_serial` | `boolean` | uses legacy serial device for guest's console (QEMU) |
| `io.katacontainers.config.hypervisor.default_gpus` | uint32 | the minimum number of GPUs required for the VM. Only used by remote hypervisor to help with instance selection |
| `io.katacontainers.config.hypervisor.default_gpu_model` | string | the GPU model required for the VM. Only used by remote hypervisor to help with instance selection |

## Container Options
| Key | Value Type | Comments |
|-------| ----- | ----- |
| `io.katacontainers.container.resource.swappiness"` | `uint64` | specify the `Resources.Memory.Swappiness` |
| `io.katacontainers.container.resource.swap_in_bytes"` | `uint64` | specify the `Resources.Memory.Swap` |

# CRI-O Configuration

In case of CRI-O, all annotations specified in the pod spec are passed down to Kata.

# containerd Configuration

For containerd, annotations specified in the pod spec are passed down to Kata
starting with version `1.3.0` of containerd. Additionally, extra configuration is
needed for containerd, by providing `pod_annotations` field and
`container_annotations` field in the containerd config
file.  The `pod_annotations` field and `container_annotations` field are two lists of
annotations that can be passed down to Kata as OCI annotations. They support golang match
patterns. Since annotations supported by Kata follow the pattern `io.katacontainers.*`,
the following configuration would work for passing annotations to Kata from containerd:

```
$ cat /etc/containerd/config
....

         [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.kata]
           runtime_type = "io.containerd.kata.v2"
           pod_annotations = ["io.katacontainers.*"]
           container_annotations = ["io.katacontainers.*"]
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
    io.katacontainers.config.runtime.disable_guest_seccomp: "false"
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

# Restricted annotations

Some annotations are _restricted_, meaning that the configuration file specifies
the acceptable values. Currently, only hypervisor annotations are restricted,
for security reason, with the intent to control which binaries the Kata
Containers runtime will launch on your behalf.

The configuration file validates the annotation _name_ as well as the annotation
_value_.

The acceptable annotation names are defined by the `enable_annotations` entry in
the configuration file.

For restricted annotations, an additional configuration entry provides a list of
acceptable values. Since most restricted annotations are intended to control
which binaries the runtime can execute, the valid value is generally provided by
a shell pattern, as defined by `glob(3)`. The table below provides the name of
the configuration entry:

| Key | Config file entry | Comments |
|-------| ----- | ----- |
| `entropy_source` | `valid_entropy_sources` | Valid entropy sources, e.g. `/dev/random` |
| `file_mem_backend`  | `valid_file_mem_backends` | Valid locations for the file-based memory backend root directory |
| `jailer_path`  | `valid_jailer_paths`| Valid paths for the jailer constraining the container VM (Firecracker) |
| `path`  | `valid_hypervisor_paths` | Valid hypervisors to run the container VM |
| `vhost_user_store_path`  | `valid_vhost_user_store_paths` | Valid paths for vhost-user related files|
| `virtio_fs_daemon`  | `valid_virtio_fs_daemon_paths` | Valid paths for the `virtiofsd` daemon |
