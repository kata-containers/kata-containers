# How to Use Template in runtime-rs

## What is VM Templating

VM templating is a Kata Containers feature that enables new VM creation using a cloning technique. When enabled, new VMs are created by cloning from a pre-created template VM, and they will share the same initramfs, kernel and agent memory in readonly mode. It is very much like a process fork done by the kernel but here we *fork* VMs.

For more details on VM templating, refer to the [What is VM templating and how do I use it](./what-is-vm-templating-and-how-do-I-use-it.md) article.

## How to Enable VM Templating

VM templating can be enabled by changing your Kata Containers config file (`/opt/kata/share/defaults/kata-containers/runtime-rs/configuration.toml`, overridden by `/etc/kata-containers/configuration.toml` if provided) such that:

- `qemu` version `v4.1.0` or above is specified in `hypervisor.qemu`->`path` section
- `enable_template = true`
- `template_path = "/run/vc/vm/template"` (default value, can be customized as needed)
- `initrd =` is set
- `image =` option is commented out or removed
- `shared_fs =` option is commented out or removed
- `default_memory =` should be set to more than 256MB

Then you can create a VM template for later usage by calling:

### Initialize and create the VM template
The `factory init` command creates a VM template by launching a new VM, initializing the Kata Agent, then pausing and saving its state (memory and device snapshots) to the template directory. This saved template is used to rapidly clone new VMs using QEMU's memory sharing capabilities.

```bash
sudo kata-ctl factory init
```

### Check the status of the VM template

The `factory status` command checks whether a VM template currently exists by verifying the presence of template files (memory snapshot and device state). It will output "VM factory is on" if the template exists, or "VM factory is off" otherwise.

```bash
sudo kata-ctl factory status
```

### Destroy and clean up the VM template

The `factory destroy` command removes the VM template by remove the `tmpfs` filesystem and deleting the template directory along with all its contents.

```bash
sudo kata-ctl factory destroy
```

## How to Create a New VM from VM Template
In the Go version of Kata Containers, the VM templating mechanism is implemented using virtio-9p (9pfs). However, 9pfs is not supported in runtime-rs due to its poor performance, limited cache coherence, and security risks. Instead, runtime-rs adopts `VirtioFS` as the default mechanism to provide rootfs for containers and VMs.

Yet, when enabling the VM template mechanism, `VirtioFS` introduces conflicts in memory sharing because its DAX-based shared memory mapping overlaps with the template's page-sharing design. To resolve these conflicts and ensure strict isolation between cloned VMs, runtime-rs replaces `VirtioFS` with the snapshotter approach — specifically, the `blockfile` snapshotter.

The `blockfile` snapshotter is used in runtime-rs because it provides each VM with an independent block-based root filesystem, ensuring strong isolation and full compatibility with the VM templating mechanism.

### Configure Snapshotter

#### Check if `Blockfile` Snapshotter is Available
```bash
ctr plugins ls | grep blockfile
```

If not available, continue with the following steps:

#### Create Scratch File
```bash
dd if=/dev/zero of=/opt/containerd/blockfile bs=1M count=500
sudo mkfs.ext4 /opt/containerd/blockfile
```

#### Configure containerd
Edit the containerd configuration file:
```bash
sudo vim /etc/containerd/config.toml
```
Add or modify the following configuration for the `blockfile` snapshotter:
```toml
[plugins."io.containerd.snapshotter.v1.blockfile"]
  scratch_file = "/opt/containerd/blockfile"
  root_path = ""
  fs_type = "ext4"
  mount_options = []
  recreate_scratch = true
```

#### Restart containerd
After modifying the configuration, restart containerd to apply changes:

```bash
sudo systemctl restart containerd
```

### Run Container with `blockfile` Snapshotter
After the VM template is created, you can pull an image and run a container using the `blockfile` snapshotter:

```bash
ctr run --rm -t --snapshotter blockfile docker.io/library/busybox:latest template sh
```

We can verify whether a VM was launched from a template or started normally by checking the launch parameters — if the parameters contain `incoming`, it indicates that the VM was started from a template rather than created directly.

## Performance Test

The comparative experiment between **template-based VM** creation and **direct VM** creation showed that the template-based approach achieved a ≈ **73.2%** reduction in startup latency (average launch time of **0.6s** vs. **0.82s**) and a ≈ **79.8%** reduction in memory usage (average memory usage of **178.2 MiB** vs. **223.2 MiB**), demonstrating significant improvements in VM startup efficiency and resource utilization.

The test script is as follows:

```bash
# Clear the page cache, dentries, and inodes to free up memory
echo 3 | sudo tee /proc/sys/vm/drop_caches

# Display the current memory usage
free -h

# Create 100 normal VMs and template-based VMs, and track the time
time for I in $(seq 100); do
  echo -n " ${I}th"  # Display the iteration number
  ctr run -d --runtime io.containerd.kata.v2 --snapshotter blockfile docker.io/library/busybox:latest normal/template${I}
done

# Display the memory usage again after running the test
free -h