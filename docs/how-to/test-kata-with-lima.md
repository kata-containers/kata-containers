# Test Kata Containers with Lima

The [Lima lab](../../tools/testing/lima/README.md) creates a disposable Linux VM,
installs Kubernetes and Kata through this checkout's CI entry points, and checks
that a pod completes using a Kata RuntimeClass. It can also use an existing Linux
host locally or over SSH. The default environment is Ubuntu 26.04, 4 vCPUs, 16 GiB RAM,
50 GiB disk, kubeadm, containerd, and `qemu-runtime-rs` from the latest CI build
(`quay.io/kata-containers/kata-deploy-ci:kata-containers-latest`).

## Host requirements

Install Python 3.9 or newer, OpenSSH, Lima 2.2.0 or newer, and
[just](https://github.com/casey/just). The optional Nix shell supplies these tools
and the controller's additional utilities. The host needs room for the Ubuntu
image, VM disk, shared checkout, container images, and temporary boot files.
The checkout is shared with the VM through a writable virtio-fs mount on both
macOS and Linux. Source edits are visible immediately.
Linux hosts also need Rust virtiofsd and QEMU configured to discover it; the
optional Nix shell supplies this configuration.

### Linux

The local VM uses QEMU/KVM and the same architecture as the host. The default
target is x86_64; an ARM64 target is also provided. Enable virtualization in the
host firmware and expose nested virtualization to the Lima VM. If the Linux
host is itself a VM, its hypervisor must expose nested virtualization too.

The user running Lima must be able to open `/dev/kvm`. On a host where that
device belongs to group `kvm`:

```sh
ls -l /dev/kvm
sudo usermod -aG kvm "$USER"
```

Start a new login session after changing group membership. `just doctor` checks
the KVM API and creates a KVM VM; VM provisioning also checks KVM inside the guest.

### macOS: supported Apple chips

The local target uses Apple's Virtualization.framework (`vmType: vz`) with an
ARM64 Linux guest and `nestedVirtualization: true`. It requires **macOS 15 or
newer** and an **Apple M3 or later** chip, following Apple's
[nested virtualization capability](https://developer.apple.com/documentation/virtualization/vzgenericplatformconfiguration/isnestedvirtualizationsupported)
and Lima's [VZ requirements](https://lima-vm.io/docs/config/vmtype/vz/).

| Mac chip | Local nested Kata VM |
| --- | --- |
| M1 family, including Pro, Max, Ultra | Unsupported |
| M2 family, including Pro, Max, Ultra | Unsupported |
| M3 family | Eligible; requires macOS 15+ and a successful capability check |
| M4 family and later Apple silicon | Eligible; requires macOS 15+ and a successful capability check |
| Intel | Unsupported by this local lab profile |

`just doctor` checks `VZGenericPlatformConfiguration.isNestedVirtualizationSupported`;
the framework's result is authoritative for the current machine. Install Apple's
Command Line Tools so that `swift` and the Virtualization SDK are available.
Unsupported Macs can use an [existing remote Linux host](#install-on-a-linux-node-locally-or-over-ssh).

Full pod execution has been validated on an M4 Pro with macOS 26.7.1, Lima 2.2.0,
and Kata 4.3.0 for both `qemu-runtime-rs` and `qemu-nvidia-cpu-runtime-rs`.
This hardware result does not imply that every eligible chip has been tested.

## Start the lab

Use a Kata checkout or Git worktree. The lab shares both the selected source
and any external Git metadata with the VM.

From the Kata repository root:

```sh
cd tools/testing/lima
nix develop
just plan
just doctor
just up
```

If the tools are already installed, omit `nix develop`. Nix is optional; `just`
itself does not install Lima. For Nix installations requiring explicit feature
flags, enter the shell with:

```sh
nix --extra-experimental-features 'nix-command flakes' develop
```

`just up` creates or starts the owned VM, waits for cloud-init, checks the shared
checkout (including local edits), prepares Ubuntu prerequisites, runs upstream
`deploy-k8s` and `deploy-kata`, and runs a pinned BusyBox workload. Success ends
with:

```text
Kata guest kernel: <version>
READY: Kubernetes pod completed using the Kata RuntimeClass
```

The controller shares CI scripts and source; it installs the CI-generated Kata
binaries selected by `DOCKER_TAG`. It does not build modified runtime code.
To test locally built artifacts, see
[building and deploying local artifacts](how-to-build-and-deploy-local-artifacts.md).

On Linux ARM64, select the matching target before creating a VM:

```sh
export JUST_KATA_TARGET=targets/linux-arm64.json
just doctor
just up
```

Without just, the Python controller offers the same operations. From the lab
directory, its target defaults to macOS or Linux as appropriate:

```sh
python3 scripts/lab.py --dry-run up
python3 scripts/lab.py doctor
python3 scripts/lab.py up
```

## Compare the latest CI build with a release

The default uses CI's `kata-deploy-ci:kata-containers-latest` image. Select a
published release with `release`, which switches to `kata-deploy:<release>`:

```sh
just up
just release=4.3.0 up
```

These commands create two independent Lima VMs for the same hardware target.
VM names and ownership records include a hash of the source path and full image
reference (registry, repository, and tag). A different checkout, release, or image reference selects
another VM automatically, so it cannot replace the default CI lab's installation.
Changing only `runtime` reuses the selected build's VM.

Use the same selection for every operation on that comparison VM:

```sh
just test
just release=4.3.0 test
just release=4.3.0 ssh
just release=4.3.0 logs
just release=4.3.0 down
```

`JUST_KATA_RELEASE=4.3.0` and Python's `--release 4.3.0` are equivalent selectors.
For example:

```sh
python3 scripts/lab.py --release 4.3.0 --dry-run up
```

The release selector changes the installed artifacts. Both VMs use the shared
checkout's provisioning scripts and chart. The moving CI tag reuses its own VM;
run `just ci deploy-kata` to refresh that installation when a newer CI image is
published. `up` reuses an already advertised installation. The lab does not
resolve a moving tag to a new VM for every image digest.

## Inspect and repeat

```sh
just test
just logs
just ssh
```

Inside the VM:

```sh
kubectl get nodes -o wide
kubectl get pods -A
kubectl get runtimeclass
exit
```

`just logs` saves diagnostics below the lab's ignored `results/` directory.
`just test` removes its temporary pod after completion or failure. Source edits
are already visible through the shared mount. To reapply Kata deployment:

```sh
just ci deploy-kata
just test
```

Use the upstream suites or another repository script through the same transport:

```sh
just ci run-tests
just ci-script tests/integration/kubernetes/gha-run.sh run-tests
```

## Test the NVIDIA CPU runtime-rs profile

Select the runtime directly; no extra configuration file is needed:

```sh
just runtime=qemu-nvidia-cpu-runtime-rs up
just runtime=qemu-nvidia-cpu-runtime-rs test
```

`qemu-runtime-rs` is the default. `runtime` accepts a Kata shim name and forwards
it to CI as `KATA_HYPERVISOR`; CI handles the corresponding profile and supported
architectures. The smoke test uses the same selection for its RuntimeClass.
For application pods, set `spec.runtimeClassName` to the installed class, such
as `kata-qemu-nvidia-cpu-runtime-rs`.

The tested NVIDIA CPU profile uses the EROFS snapshotter, memory-backed writable
layers, dm-verity, and `shared_fs = "none"`. It requires no GPU passthrough.
The CI helper selects the upstream
[NVIDIA CPU example](../../tools/packaging/kata-deploy/helm-chart/kata-deploy/try-kata-nvidia-cpu.values.yaml)
automatically. On macOS, one Apple override file applies the nesting workarounds
to both runtime-rs handlers; storage configuration stays in the upstream profile.

To explicitly reapply this runtime to an existing build's VM:

```sh
just runtime=qemu-nvidia-cpu-runtime-rs ci deploy-kata
just runtime=qemu-nvidia-cpu-runtime-rs test
just runtime=qemu-nvidia-cpu-runtime-rs logs
```

The smoke test selects `kata-qemu-nvidia-cpu-runtime-rs` and reports the NVIDIA
guest kernel. On macOS, generic QEMU remains installed too:

```sh
just runtime=qemu-runtime-rs test
just runtime=qemu-nvidia-cpu-runtime-rs test
```

For a release comparison, retain the runtime selection and select the release's
separate VM:

```sh
just runtime=qemu-nvidia-cpu-runtime-rs release=4.3.0 up
just runtime=qemu-nvidia-cpu-runtime-rs release=4.3.0 test
```

Persist a runtime selection in the current shell with `JUST_KATA_RUNTIME`:

```sh
export JUST_KATA_RUNTIME=qemu-nvidia-cpu-runtime-rs
just up
just test
unset JUST_KATA_RUNTIME
```

The Python equivalent is `python3 scripts/lab.py --runtime
qemu-nvidia-cpu-runtime-rs up`. An explicit runtime option takes precedence over
`KATA_HYPERVISOR` in an advanced CI configuration file.

## Shared CI implementation

The lab reuses these existing entry points and helpers from the selected checkout:

| Operation | Shared implementation |
| --- | --- |
| Kubernetes and container engine installation | `tests/integration/kubernetes/gha-run.sh deploy-k8s` |
| Kata installation, modules, runtime classes, snapshotters | `tests/integration/kubernetes/gha-run.sh deploy-kata` |
| NVIDIA and other specialized profile defaults | `helm_helper` in `tests/gha-run-k8s-common.sh` |
| Helm bootstrap and deployment diagnostics | `ensure_helm` and `dump_kata_deploy_diagnostics` in the same helper |
| yq bootstrap | `ci/install_yq.sh` |
| Upstream workload suites | `tests/integration/kubernetes/gha-run.sh run-tests` |

The lab-specific code handles Lima ownership and lifecycle, virtio-fs sharing,
SSH transport, fresh-VM bootstrap, and KVM capability checks. Its small smoke test uses a pinned
workload and can run repeatedly without a Helm upgrade. CI's Helm verification
hook remains available for deployment verification; it does not replace this
independent `just test` operation.

## Customize targets and Kubernetes

Copy a target to an ignored `*.local.json` file before changing CPU, memory,
disk size, or its name. Once created, a VM's target signature must remain
unchanged; changing it is rejected to avoid acting on the wrong VM.

```sh
cp targets/local.json targets/experiment.local.json
export JUST_KATA_TARGET=targets/experiment.local.json
# Edit this file before creating the VM.
just plan
just up
```

The macOS default is `targets/macos.json`; the Linux x86_64 default is
`targets/local.json`. `JUST_KATA_SOURCE` selects another checkout or worktree.
`ci/default.json` contains the upstream CI environment, including the release
image. Copy it to `ci/custom.local.json` and select it with
`JUST_KATA_CI_CONFIG` to change containerd versions, runtimes, or test filters.

The CI adapter also exposes `vanilla`, `k3s`, `k0s`, `rke2`, and `microk8s`.
Select a Kubernetes distribution for a fresh VM; an installed distribution
cannot be changed in place:

```sh
just target=targets/experiment.local.json kubernetes=k3s up
```

## Select a checkout, branch, or pull request

Use the current checkout, including local edits, or select another path:

```sh
just kata_source=/path/to/kata-containers up
```

The selected checkout is mounted writable at the same absolute path in the Lima
VM and is available through `/home/ubuntu/just-kata/src`. Git metadata outside a
worktree is also shared so Git commands work inside the VM. Edits from either
side affect the same files.

To keep the original checkout's edits separate, select a branch or PR:

```sh
just branch=topic/my-change up
just pr=14100 up
just pr=14100 test
just pr=14100 logs
just pr=14100 down
```

These selections create detached Git worktrees below `.state/worktrees/`.
The branch or commit must already exist in the base repository; fetch it first
if necessary. PR selection fetches the PR head from the Kata repository and
selects `ghcr.io/kata-containers/kata-deploy-ci:<PR>-<head-SHA>-<arch>`.
The controller checks that the PR image is available before provisioning.
Wait for the architecture's CI build if it has not been published yet, or
explicitly choose another image with `image=<registry>/<repository>:<tag>`
or a published release with `release=<version>`.

Worktrees are created by `up`, `install-kata`, or `install-cluster` and reused
by later commands. Their initial revision and local edits are preserved;
subsequent invocations do not fetch or reset them. To update a cached worktree,
fetch and check out the desired revision there, then repeat the install command.
For PRs, the image selection follows the worktree's current HEAD.
Use either `pr` or `branch` and repeat the same selectors for later operations.
Branch selection uses the default CI image unless an image or release is selected;
sharing source edits does not rebuild Kata binaries.

## Install on a Linux node locally or over SSH

The bare-metal backend uses a Linux node with usable KVM and passwordless sudo.
Run it directly on that node, or configure passwordless SSH authentication in
your normal SSH configuration, including `ProxyJump` if needed. A Kata checkout
must already be present on the execution node.

For an existing Kubernetes cluster, install Kata and run the smoke test:

```sh
# Run on the Linux node, from this lab directory.
just backend=baremetal install-kata
just backend=baremetal test

# Run from a controller, using the checkout already on the SSH node.
just backend=baremetal host=user@node kata_source=/path/on/node/kata-containers install-kata
just backend=baremetal host=user@node kata_source=/path/on/node/kata-containers test
```

`install-kata` uses the existing Kubernetes installation. Set
`kubeconfig=/absolute/path/to/config` when needed; the path refers to the execution
node. The selected Kubernetes distribution must match the existing cluster.

For a fresh Ubuntu 26.04 node, use `install-cluster` with the same selectors.
It installs host prerequisites, Kubernetes, containerd, and Kata, then runs the
smoke test. Fresh-host checks reject an unmanaged existing installation.

Runtime, release, image, PR, and branch selectors work with either mode. On SSH
nodes, worktrees are created there under `~/.cache/kata-lab/worktrees/`, and the
architecture is detected remotely. Set `JUST_KATA_ARCH=amd64` or `arm64` to
override detection. `logs` collects diagnostics through the same transport.
`up` and `down` manage Lima VMs; bare-metal commands do not provide an uninstall.

## Networking and ARM boot diagnostics

Local profiles use Lima's default user-mode networking and virtio-fs source
mounts, with no extra vmnet networks. SSH uses Lima's generated SSH configuration, so
the controller does not depend on guest DHCP addresses. The guest account has
passwordless sudo for CI provisioning. VPN routing and ICMP behavior can differ
from Apple vmnet networking; check actual registry access as well as ping.

On macOS, the default Helm drop-in removes the unsupported `pmu=off` CPU
property, disables virtual ITS (`its=off`), and allows 60 seconds for the agent
connection. Runtime, hypervisor, and agent debug logging remain enabled.
In the tested nested ARM configuration, ITS caused a boot stall after root-disk
detection; increasing the timeout alone did not fix it. Both settings were
needed for the validated workload. The underlying failing layer has not been
established, so keep these overrides scoped to the Mac profile.

If a workload fails:

```sh
just logs
just probe
just ssh
```

Inside the VM, inspect the relevant kernel components:

```sh
uname -a
lsmod | sort
ls -l /dev/kvm /dev/vhost-vsock /dev/vsock /dev/vhost-net
sudo journalctl -u containerd -u kubelet --no-pager -n 200
```

ARM KVM can be built into the kernel, so it need not appear in `lsmod`.

If Lima reports `disk quota exceeded` while writing a temporary boot image,
check host free space and quotas. If the system temporary filesystem is full,
select another filesystem with space before retrying:

```sh
mkdir -p "$HOME/.cache/lima-tmp"
export TMPDIR="$HOME/.cache/lima-tmp"
just up
```

## Delete the VM

```sh
just down
```

The controller verifies the target signature and a per-VM ownership token before
stopping and deleting its recorded Lima VM. Records live in `.state/`; preserve
them until teardown. Deleting the VM removes its cluster and guest data. Existing
unrelated Lima instances are not selected by this operation.
