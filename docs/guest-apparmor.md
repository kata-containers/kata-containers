# Guest-side AppArmor

This document describes how Kata applies AppArmor policy inside the guest, and
the boundary of what has been validated, for the upstream discussion in
[issue #7586](https://github.com/kata-containers/kata-containers/issues/7586).
It intentionally does not define a final Kubernetes API or Kata runtime
configuration format.

## Where the pieces live

Three components are involved, and keeping them separate is the point:

| Component | Owns |
| --- | --- |
| Base guest image | `apparmor_parser` and its runtime dependencies |
| AppArmor guest extension | All workload policy text, including anything profiles include |
| Guest agent | Loading a requested profile and verifying it is enforced |

Policy is not built into the base image. Policy changes far more often than the
guest operating system does, and putting it in the base image would mean
rebuilding and redistributing a rootfs for every policy change. It ships in a
guest extension image instead, which is attached to the VM at start and mounted
read-only before the agent runs.

The extension carries policy text only. It contains no parser and no shared
libraries, because those belong to the base image.

## Current execution model

The OCI `process.apparmorProfile` value is preserved by `runtime-rs` and passed
to the guest agent as an opaque value. The agent performs two separate
operations:

1. Before container namespace and rootfs setup, it resolves the value against
   the mounted AppArmor guest extension, verifies `securityfs`, loads the
   profile with `apparmor_parser` when it is not already loaded, and verifies
   from the kernel that the profile is present and in enforce mode.
2. After the container rootfs is prepared, but before credentials and
   capabilities are dropped, it writes `exec <profile>` to
   `/proc/self/attr/exec`. The following workload `execve()` then enters the
   selected profile.

The loader never reads the workload container rootfs and never accepts profile
text from the host at runtime.

Profile preparation is serialized inside the guest with a short-lived lock
around the loaded-profile check, parser invocation, and post-load verification.
Concurrent containers requesting the same profile therefore reuse one loaded
kernel profile instead of racing to load it.

## The extension root is the policy root

The contents of the extension are mounted at `/run/kata-extensions/apparmor`,
and that directory is also the only include root given to `apparmor_parser`:

```text
/run/kata-extensions/apparmor/
├── profiles/         profiles/<name>, requested by name
├── abstractions/     #include <abstractions/...>
├── tunables/         #include <tunables/...>
├── local/            site-specific includes
└── metadata/manifest version and provenance
```

Everything a profile can include must live under this root. If profiles could
include the distribution's `/etc/apparmor.d`, their meaning would change
whenever the base image's packages changed, which is exactly the coupling this
layout exists to avoid. The include root is passed explicitly because
`apparmor_parser` adds to its search path rather than replacing it.

A profile's file name is what the agent resolves against, and the agent then
verifies from the kernel that a profile with that name is loaded. A file named
`example` that declares `profile something-else` therefore does not satisfy a
request for `example`; the built-in build check rejects that combination.

## Profile behavior

| Input | Behavior |
| --- | --- |
| Empty or absent | No AppArmor operation; the extension may be absent entirely |
| `unconfined` | No AppArmor operation, following OCI semantics |
| A validated profile name | Load or reuse the guest profile, verify it is enforced, then select it before `execve()` |
| `localhost/<name>` | Temporary OCI-boundary compatibility normalization |
| `runtime/default` | Temporary compatibility mapping to `kata-default` |
| Missing or invalid profile | Container startup fails |
| Parser failure, or profile not in enforce mode | Container startup fails |

A profile that is loaded in **complain** mode is refused rather than accepted.
Complain mode reports violations without preventing them, so treating it as
success would leave the workload effectively unconstrained while appearing to
have applied policy.

The `localhost/` and `runtime/default` handling is an implementation aid for
values observed at the OCI boundary. Containerd may already have resolved
`RuntimeDefault` into a concrete profile name before the request reaches the
guest, and where that mapping should live remains an open design question for
issue #7586.

## Failure modes

Failures happen at different layers, which matters when diagnosing them:

| Situation | Where it fails |
| --- | --- |
| Extension image missing, dm-verity failure, or mount failure | Guest boot. The mount unit runs before the agent and powers the VM off on failure, so the agent never starts |
| No extension configured, but a profile is requested | Container creation, with an error saying the extension is not configured |
| Extension configured but not mounted | Container creation, with an error naming the missing mount point |
| Profile absent from a mounted extension | Container creation, with the resolved path in the error |
| Parser missing, or parser failure | Container creation |
| Profile loaded but not enforced | Container creation |
| Writing `/proc/self/attr/exec` fails | Workload `execve()` in the container init process |

A workload that requests no profile is unaffected by the extension being absent
or unmounted.

### Known limitation: `AGENT_INIT=yes`

Only guests running systemd mount guest extension images, because the mount is
performed by a systemd unit ordered before the agent. A guest built with
`AGENT_INIT=yes` therefore cannot use this feature today. The agent detects
this case and says so explicitly rather than reporting a missing profile, so
the two situations are not confused.

### Known limitation: exec processes

`exec_process()` in `runtime-rs` no longer clears the container's AppArmor
profile, so the value reaches the agent on the exec path as well. Whether an
exec is allowed to request a profile different from its container's, and
whether clearing it should remain the default, are open upstream questions.
What is not acceptable is an exec silently running unconfined inside a confined
container.

## Guest image requirements

The base image provides the parser:

```bash
PACKAGES+=" apparmor"     # in the ubuntu and debian rootfs configs
```

`apparmor-utils` is deliberately not installed: it is a command line tool the
agent never invokes, and it pulls in a python3 dependency chain.

The guest kernel must provide AppArmor, and `CONFIG_SECURITYFS` must be set;
the agent mounts `securityfs` if it is not already mounted.

The distribution's `apparmor.service` loads every profile in `/etc/apparmor.d`
at boot. If it ran, workload profiles could already be in the kernel before the
agent was asked to apply one, and the agent could not tell where a loaded
profile came from. The base image therefore installs a systemd preset that
disables it. This is weaker than masking the unit on purpose: the requirement
is that the base image must not preload workload profiles, not that the service
is forbidden, so it remains available for anyone who enables it explicitly.

## Host and CRI boundary

Host-side AppArmor state must be recorded separately from guest enforcement.
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
`securityContext.appArmorProfile` form on a newer Kubernetes release.

## Earlier prototype validation (historical)

!!! warning "This describes the superseded prototype"
    The results below were obtained while policy was delivered by unpacking a
    profile tarball into `/etc/apparmor.d` at rootfs build time, using the
    prototype's `GUEST_APPARMOR` and `GUEST_APPARMOR_PROFILE_TARBALL` build
    options and a test profile named `kata-default`.

    Those build options and that lookup path no longer exist. Policy now comes
    from the guest extension described above, and the test profile is named
    `kata-apparmor-test`. The results are kept because they are the only
    end-to-end evidence collected so far, and because the failure modes they
    record are still informative, but they **do not** validate the current
    implementation.

    Re-running these checks against the extension-based path is outstanding
    work.

### Validation on a separate Kubernetes cluster

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
