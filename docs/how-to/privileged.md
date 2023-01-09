# Privileged Kata Containers

Kata Containers supports creation of containers that are "privileged" (i.e. have additional capabilities and access
that is not normally granted).

## Warnings

**Warning:** Whilst this functionality is supported, it can decrease the security of Kata Containers if not configured 
correctly.

### Host Devices

By default, when privileged is enabled for a container, all the `/dev/*` block devices from the host are mounted
into the guest. This will allow the privileged container inside the Kata guest to gain access to mount any block device 
from the host, a potentially undesirable side-effect that decreases the security of Kata.

The following sections document how to configure this behavior in different container runtimes.

#### Containerd

The Containerd allows configuring the privileged host devices behavior for each runtime in the containerd config. This is
done with the `privileged_without_host_devices` option. Setting this to `true` will disable hot plugging of the host 
devices into the guest, even when privileged is enabled.

Support for configuring privileged host devices behaviour was added in containerd `1.3.0` version.

See below example config:

```toml
[plugins]
  [plugins.cri]
    [plugins.cri.containerd]
       [plugins.cri.containerd.runtimes.runc]
         runtime_type = "io.containerd.runc.v2"
         privileged_without_host_devices = false
       [plugins.cri.containerd.runtimes.kata]
         runtime_type = "io.containerd.kata.v2"
         privileged_without_host_devices = true
         [plugins.cri.containerd.runtimes.kata.options]
           ConfigPath = "/opt/kata/share/defaults/kata-containers/configuration.toml"
```

 - [How to use Kata Containers and containerd with Kubernetes](how-to-use-k8s-with-containerd-and-kata.md)
 - [Containerd CRI config documentation](https://github.com/containerd/containerd/blob/main/docs/cri/config.md)

#### CRI-O

Similar to containerd, CRI-O allows configuring the privileged host devices
behavior for each runtime in the CRI config. This is done with the 
`privileged_without_host_devices` option. Setting this to `true` will disable
 hot plugging of the host devices into the guest, even when privileged is enabled.

Support for configuring privileged host devices behaviour was added in CRI-O `1.16.0` version.

See below example config:

```toml
[crio.runtime.runtimes.runc]
  runtime_path = "/usr/local/bin/crio-runc"
  runtime_type = "oci"
  runtime_root = "/run/runc"
  privileged_without_host_devices = false
[crio.runtime.runtimes.kata]
  runtime_path = "/usr/bin/kata-runtime"
  runtime_type = "oci"
  privileged_without_host_devices = true
[crio.runtime.runtimes.kata-shim2]
  runtime_path = "/usr/local/bin/containerd-shim-kata-v2"
  runtime_type = "vm"
  privileged_without_host_devices = true
```

 - [Kata Containers with CRI-O](../how-to/run-kata-with-k8s.md#cri-o)
  
