# Runtime Configuration


## Drop-in Files

When kata-deploy installs Kata Containers, the base configuration files at 
`/opt/kata/bin/share/defaults` should not be modified directly. Instead, use 
drop-in configuration files to customize settings. This approach ensures your 
customizations survive kata-deploy upgrades.

### How Drop-in Files Work

The Kata runtime reads the base configuration file and then applies any `.toml`
files found in the `config.d/` directory alongside it. Files are processed in
alphabetical order, with later files overriding earlier settings.

### Base Configuration Files

The base configuration references for the Go runtime can be found [here](https://github.com/kata-containers/kata-containers/tree/main/src/runtime/config), and for the Rust runtime [here](https://github.com/kata-containers/kata-containers/tree/main/src/runtime-rs/config).

!!! tip "What runtime implementation am I using?"

    By looking at the `/opt/kata/containerd/config.d/kata-deploy.toml` file, each runtimeClass (ex. `kata-qemu-nvidia-gpu`, `kata-qemu-nvidia-gpu-runtime-rs`) is configured with a specific `runtime_path`. If this path is set to `#!toml runtime_path = "/opt/kata/bin/containerd-shim-kata-v2"` you are using the Go runtime. Otherwise if it's `#!toml runtime_path = "/opt/kata/runtime-rs/bin/containerd-shim-kata-v2"`, it's the Rust runtime.

Note that Rust will be the default runtime in Kata v4.

### Creating Custom Drop-in Files

The recommended way to create custom drop-in files is to use the [helm chart](helm-configuration.md#drop-in-runtime-configuration).
Drop-in files may also be added directly to the filesystem.

To add custom settings, create a `.toml` file in the appropriate `config.d/`
directory. Use a numeric prefix to control the order of application.

**Reserved prefixes** (used by kata-deploy):

- `10-*`: Core kata-deploy settings
- `20-*`: Debug settings
- `30-*`: Kernel parameters
- `50-*`: Settings from the helm chart

**Recommended prefixes for custom settings**: `50-89`

### Drop-In Config Examples

#### Adding Custom Kernel Parameters

```bash
# SSH into the node or use kubectl exec
sudo mkdir -p /opt/kata/share/defaults/kata-containers/runtimes/qemu/config.d/
sudo cat > /opt/kata/share/defaults/kata-containers/runtimes/qemu/config.d/50-custom.toml << 'EOF'
[hypervisor.qemu]
kernel_params = "my_param=value"
EOF
```

#### Changing Default Memory Size

```bash
sudo cat > /opt/kata/share/defaults/kata-containers/runtimes/qemu/config.d/50-memory.toml << 'EOF'
[hypervisor.qemu]
default_memory = 4096
EOF
```
