# Guest-side AppArmor

This document records the current opt-in Guest AppArmor prototype and the
validation boundary for the upstream discussion in
[issue #7586](https://github.com/kata-containers/kata-containers/issues/7586).
It intentionally does not define a final Kubernetes API or Kata runtime
configuration format.

## Current execution model

The OCI `process.apparmorProfile` value is preserved by `runtime-rs` and passed
to the Guest agent as an opaque value. The Guest agent performs two separate
operations:

1. Before container namespace and rootfs setup, it resolves the value against
   trusted files already present in the Guest rootfs, verifies `securityfs`,
   loads the profile with `apparmor_parser` when required, and verifies that
   the profile is visible in the Guest kernel.
2. After the container rootfs is prepared, but before credentials and
   capabilities are dropped, it writes `exec <profile>` to
   `/proc/self/attr/exec`. The following workload `execve()` then enters the
   selected profile.

The profile loader never reads the workload container rootfs and never accepts
profile text from the Host at runtime.

Profile preparation is serialized inside the Guest with a short-lived lock
around the loaded-profile check, parser invocation, and post-load verification.
Concurrent containers requesting the same profile therefore reuse one loaded
kernel profile instead of racing to load it.

## Profile behavior

The following behavior is currently implemented:

| Input | Behavior |
| --- | --- |
| Empty or absent | No AppArmor operation |
| `unconfined` | No AppArmor operation, following OCI semantics |
| A validated profile name | Load or verify the Guest profile, then select it before `execve()` |
| `localhost/<name>` | Temporary OCI-boundary compatibility normalization |
| `runtime/default` | Temporary compatibility mapping to `kata-default` |
| Missing or invalid profile | Container startup fails |
| Parser failure or missing kernel profile | Container startup fails |

The `localhost/` and `runtime/default` handling is an implementation aid for
values observed at the OCI boundary. Ownership of Kubernetes semantic mapping,
including the final location of `runtime/default -> kata-default`, remains an
open design question for issue #7586.

## Guest image requirements

Ubuntu and Debian rootfs builds support the following existing opt-in settings:

```bash
GUEST_APPARMOR=yes
GUEST_APPARMOR_PROFILE_TARBALL=/path/to/profiles.tar.zst
```

The build installs the AppArmor userspace packages and unpacks trusted profile
files into `/etc/apparmor.d`. The Guest kernel must provide AppArmor and the
Guest must mount `securityfs`.

The current rootfs path also links the distribution AppArmor service for
systemd-based Guests. This remains a prototype validation path. It is not yet
the final preload contract and does not cover `AGENT_INIT=yes` Guests.

## Host and CRI boundary

Host-side AppArmor state must be recorded separately from Guest enforcement.
For Kubernetes `Localhost` profiles, containerd CRI may require the profile to
already be loaded on the Host before it creates the OCI specification. If that
check fails, the request never reaches `runtime-rs` or the Guest agent. This is
an expected baseline failure and must not be reported as a Guest loading bug.

The integration test therefore records the stage at which a request fails:

```text
Host CRI validation
runtime-rs OCI profile preservation
Guest profile discovery or parser load
Guest AppArmor exec transition
Workload enforcement
```

The existing compatibility test uses the annotation form by default for the
older Kubernetes 1.29 validation environment. Set
`KATA_K8S_APPARMOR_API=field` when testing the
`securityContext.appArmorProfile` form on a newer Kubernetes release. The
separate new-cluster validation described below uses Kubernetes 1.28.2 and
does not imply that Guest AppArmor is available there.

## Validation on the new Kubernetes cluster

The implementation was checked against a separate three-node Kubernetes
1.28.2 cluster. The control-plane node is ARM64 and the two worker nodes are
AMD64. The `kata` RuntimeClass is restricted to AMD64 nodes, so the Guest
workload was scheduled to an AMD64 worker. The worker used Kata Containers
3.28.0 with a 5.15.167 Guest kernel.

The baseline Kata workload reached `1/1 Running` and reported the expected
Guest kernel version. This confirms RuntimeClass scheduling, containerd,
Kata, the hypervisor and the Guest image can start a workload on the new
cluster.

The production Guest kernel is not an AppArmor-enabled kernel. Its
configuration reports:

```text
# CONFIG_SECURITY_APPARMOR is not set
CONFIG_LSM="lockdown,yama,loadpin,safesetid,integrity,bpf"
```

The existing production Guest image was also inspected read-only. It contains
an `/etc/apparmor.d` directory with a distribution profile, but it does not
contain `apparmor_parser` in `/sbin` or `/usr/sbin`. It was not replaced.

For the opt-in validation path, a separate AppArmor-enabled Guest kernel and
rootfs were built. The candidate rootfs contains `apparmor_parser`, the
trusted profile set, and the profile name emitted by the Kubernetes 1.28
containerd CRI path (`cri-containerd.apparmor.d`).

Inside the running Guest, `/sys/kernel/security/lsm` was unavailable and the
container reported `Seccomp: 0` because no seccomp profile was requested by
the baseline Pod. Therefore the current result is:

| Check | Result | Interpretation |
| --- | --- | --- |
| Kata RuntimeClass scheduling | Pass | Workload runs on the intended AMD64 worker |
| Guest kernel boot | Pass | Guest kernel 5.15.167 boots successfully |
| Guest AppArmor kernel support | Blocked | `CONFIG_SECURITY_APPARMOR` is not enabled |
| Guest BPF LSM activation | Not covered by this test | Requires a separate BPF policy test |
| Guest AppArmor deny enforcement | Pass in isolated handler | Guest profile is enforced on `rd350x` |

The production `kata` RuntimeClass still uses the original kernel, image and
shim. The validation uses a separate `kata-apparmor` RuntimeClass and a
separate `kata-apparmor` containerd handler, so the result does not claim that
the production handler has been changed.

## Candidate Guest validation

Before changing the Kubernetes containerd handler, the candidate kernel and
rootfs were tested in an isolated QEMU/KVM Guest. The kernel was built from
Linux 5.15.167 with AppArmor, BPF LSM and BTF enabled. The rootfs was derived
from the existing Ubuntu Noble Guest image and extended with `apparmor_parser`
and the `kata-default` test profile.

The Guest boot log showed:

```text
AppArmor: AppArmor initialized
LSM support for eBPF active
LSM=capability,apparmor,bpf
```

The following Guest-side sequence then completed successfully:

```text
securityfs mounted
apparmor_parser -r /etc/apparmor.d/kata-default -> 0
kata-default (enforce)
write exec kata-default to /proc/self/attr/exec -> 0
write /tmp/kata-apparmor-deny -> Permission denied
```

This is positive evidence for the Guest kernel, rootfs, parser, profile
loading and exec transition design. The modified `kata-agent` and
`containerd-shim-kata-v2` also compiled successfully as amd64 GNU release
artifacts in an isolated build environment, with the agent built with its
Seccomp feature enabled. These artifacts have not been installed over the
production binaries.

The candidate kernel, image, agent and shim were then registered through an
isolated `kata-apparmor` containerd handler on `rd350x`. The existing
production `kata` handler was left unchanged. A Kubernetes Pod scheduled to
`rd350x` reached `1/1 Running`, reported the Guest profile
`cri-containerd.apparmor.d (enforce)`, and received `Permission denied` when
writing `/tmp/kata-apparmor-deny`.

The first Kubernetes attempt intentionally failed closed because the CRI had
provided `cri-containerd.apparmor.d` and the candidate Guest rootfs did not
yet contain that trusted profile. After adding the profile to the candidate
rootfs and rebuilding only the isolated image, the same Pod started
successfully. This demonstrates both the missing-profile failure path and the
successful Guest enforcement path.

An additional Pod using the `unconfined` OCI value started successfully,
reported `unconfined`, and was able to create the protected-path test file.
This confirms that an explicit unconfined request is not silently converted to
the Guest AppArmor profile.

The Host/CRI boundary was also exercised with
`localhost/kata-default`. Because `kata-default` was not loaded in the Host
AppArmor namespace, containerd rejected the request with
`apparmor profile not found kata-default` before the candidate Guest was
started. This is recorded as a Host CRI rejection, not as a Guest profile
loading failure.

## Tests

Rust unit tests cover profile normalization, no-op inputs, name validation and
the separation between preparation and selection. The Kubernetes test is
opt-in because it requires a Kata Guest kernel with AppArmor, a Guest rootfs
containing the requested profile, and an execution path accepted by the Host
CRI:

```bash
KATA_GUEST_APPARMOR_TEST=yes \
KATA_GUEST_APPARMOR_PROFILE=kata-apparmor-test \
KATA_GUEST_APPARMOR_NODE=rd350x \
KATA_GUEST_APPARMOR_RUNTIME_CLASS=kata-apparmor \
KATA_GUEST_APPARMOR_IMAGE=registry.cn-hangzhou.aliyuncs.com/acs/ubuntu:22.04 \
KATA_GUEST_APPARMOR_REQUEST=runtime/default \
KATA_GUEST_APPARMOR_EXPECTED_PROFILE=cri-containerd.apparmor.d \
KATA_GUEST_APPARMOR_SKIP_NODE_DIAGNOSTICS=yes \
KATA_GUEST_APPARMOR_PROTECTED_PATH=/tmp/kata-apparmor-deny \
KATA_GUEST_APPARMOR_EVIDENCE_DIR=/tmp/guest-apparmor-evidence \
bats tests/integration/kubernetes/k8s-apparmor.bats
```

The test requires the workload to report its current AppArmor profile and to
receive `Permission denied` or `Operation not permitted` when writing the
protected path. `runtime/default` is the default request because containerd
CRI may convert it to a concrete OCI profile such as
`cri-containerd.apparmor.d`; set `KATA_GUEST_APPARMOR_EXPECTED_PROFILE` to the
profile name present in the candidate Guest rootfs. Set
`KATA_GUEST_APPARMOR_REQUEST=localhost/<profile>` only when the Host CRI
profile is intentionally prepared and the Host-side validation is part of the
test. When `KATA_GUEST_APPARMOR_EVIDENCE_DIR` is set, the test also saves the
Pod YAML, Pod description, Kubernetes events and, when available, a
`crictl inspect` result for Host/CRI review. A missing or Host-unavailable
profile must cause workload startup to fail rather than silently running
unconfined. On an existing cluster, set `KATA_GUEST_APPARMOR_NODE` and use a
dedicated kubeconfig whose current namespace is reserved for the test. Set
`KATA_GUEST_APPARMOR_SKIP_NODE_DIAGNOSTICS=yes` unless creating a temporary
node-debugger Pod in `kube-system` is explicitly acceptable.

The current validation run passed the isolated Guest AppArmor rootfs checks,
the six focused Rust AppArmor unit tests, isolated release compilation of the
modified agent and runtime, and the opt-in Kubernetes test on the new cluster.
The BATS 1.2.1 run on the new cluster used a dedicated kubeconfig namespace,
explicitly targeted `rd350x`, and completed with both cases passing:

```text
1..2
ok 1 Kata guest AppArmor denies a protected write
ok 2 missing or Host-unavailable AppArmor profile never reaches Ready
```

The Kubernetes evidence includes:

```text
cluster: Kubernetes 1.28.2
node: rd350x
runtime class: kata-apparmor
profile: cri-containerd.apparmor.d (enforce)
protected write: Permission denied
unconfined profile: unconfined
unconfined protected write: exit code 0
```

The production Kata binaries and the production `kata` handler configuration
were left unchanged. The isolated handler and candidate artifacts remain
available for further opt-in testing. A full workspace format check and the
complete CI matrix still require the repository's generated files, pinned Git
dependencies and supported Linux runners; those remain follow-up work.

## Open upstream questions

The following items remain intentionally undecided until maintainer feedback:

- Whether `runtime/default` mapping belongs to CRI, runtime-rs, or the Guest
  agent.
- Whether custom Guest profiles should be represented by Kata runtime
  configuration.
- Whether boot-time preload should use systemd, agent-init, or a common Guest
  boot abstraction.
- How profile versioning, update, rollback and multi-container concurrency
  should be represented.

## Current limitations

The prototype does not modify Host kernel parameters or GRUB. The new-cluster
validation does add an explicitly named, opt-in containerd handler and
RuntimeClass for the candidate artifacts; it does not replace the production
handler. Full compatibility coverage still requires real tests across
supported Guest distributions, init modes, hypervisors and CI runners with
AppArmor-enabled Guest kernels.
