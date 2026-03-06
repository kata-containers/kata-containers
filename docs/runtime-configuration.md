# Runtime Configuration

The containerd shims (both the Rust and Go implementations) take configuration files to control their behavior. These files are in `/opt/kata/share/defaults/kata-containers/`. An example excerpt:

```toml title="/opt/kata/share/defaults/kata-containers/configuration.toml"
[hypervisor.qemu]
path = "/opt/kata/bin/qemu-system-x86_64"
kernel = "/opt/kata/share/kata-containers/vmlinux.container"
image = "/opt/kata/share/kata-containers/kata-containers.img"
machine_type = "q35"

# rootfs filesystem type:
#   - ext4 (default)
#   - xfs
#   - erofs
rootfs_type = "ext4"

# Enable running QEMU VMM as a non-root user.
# By default QEMU VMM run as root. When this is set to true, QEMU VMM process runs as
# a non-root random user. See documentation for the limitations of this mode.
rootless = false

# List of valid annotation names for the hypervisor
# Each member of the list is a regular expression, which is the base name
# of the annotation, e.g. "path" for io.katacontainers.config.hypervisor.path"
enable_annotations = ["enable_iommu", "virtio_fs_extra_args", "kernel_params"]
```

These files should never be modified directly. If you wish to create a modified version of these files, you may create your own [custom runtime](helm-configuration.md#custom-runtimes). For example, to modify the image path, we provide these values to helm:

```yaml title="values.yaml"
customRuntimes:
  enabled: true
  runtimes:
    my-gpu-runtime:
      baseConfig: "qemu-nvidia-gpu"
      dropIn: |
        [hypervisor.qemu]
        image = "/path/to/custom-image.img"
      runtimeClass: |
        kind: RuntimeClass
        apiVersion: node.k8s.io/v1
        metadata:
          name: kata-my-gpu-runtime
          labels:
            app.kubernetes.io/managed-by: kata-deploy
        handler: kata-my-gpu-runtime
        overhead:
          podFixed:
            memory: "640Mi"
            cpu: "500m"
        scheduling:
          nodeSelector:
            katacontainers.io/kata-runtime: "true"
```

