# Guest-side AppArmor

This feature applies workload AppArmor profiles inside a Kata guest. It follows
the composable-image direction discussed in
[issue #7586](https://github.com/kata-containers/kata-containers/issues/7586).
The current local candidate uses an explicit guest opt-in and literal final OCI
profile names. This is the candidate contract, not a claim of upstream approval
or a passing CI run.

## Architecture and defaults

| Component | Responsibility |
| --- | --- |
| Ubuntu/Debian base image | `apparmor_parser` and its runtime libraries |
| AppArmor guest extension | Policy text and its complete include/ABI dependencies |
| Agent parent | Trusted first load, policy identity, container binding and reuse |
| Workload init/exec helper | Select the prepared profile before workload `execve()` |

The extension is attached at VM creation and mounted read-only at
`/run/kata-extensions/apparmor` before the agent starts. It contains no parser
or shared libraries. Neither the workload image nor the RPC request supplies
policy text. Changing policy requires a new extension and a new sandbox;
in-place policy updates are not supported.

Installing the parser does not attach an extension or confine workloads by
default. The distribution's `apparmor.service` is disabled by a preset, not
masked. Explicitly preloading a workload profile before the agent first loads
it will cause an unknown-provenance error, even if its name matches.

`[agent.kata] enable_guest_apparmor` defaults to `false`. Omission and explicit
`false` preserve existing Kata guest behavior and provide no Guest AppArmor:
the agent does not load, prepare or select guest profiles, regardless of the host
CRI default. Attaching an extension alone does not enable the feature. With
`true`, a configured `apparmor` extension is required, the agent must actually
load and verify policy, and errors fail closed without an unconfined retry.
Host-side validation remains independent in either mode.
`AgentDetails.guest_apparmor_enabled` reports the opt-in separately from the
kernel's AppArmor availability; a kernel capability bit alone is insufficient.
Both runtimes verify this reported mode before allowing workloads to start.

The production image `kata-containers-apparmor-extension.img` contains only the
`cri-containerd.apparmor.d` default profile and its dependencies/metadata. Its
policy comes from the vendored containerd **v2.2.1** template, rendered with fixed
inputs; the [source record](../tools/packaging/apparmor-extension/policy/metadata/default-profile-source.txt)
records inputs, hashes, modifications and license attribution. It is extension
content, never a base-image or agent-embedded fallback.

The separate `kata-containers-apparmor-test-extension.img` contains that same
default policy plus `kata-apparmor-test`. The latter is a permissive mechanism
test fixture with one denied write, not a production hardening policy. Production
packaging must exclude it. No `kata-default` alias is shipped. NVIDIA integration
and a SELinux extension are outside this change.

| Purpose | Build target | Image | Verity sidecar | Mount name |
| --- | --- | --- | --- | --- |
| Production, default only | `rootfs-image-apparmor-extension-tarball` | `kata-containers-apparmor-extension.img` | `root_hash_apparmor-extension.txt` | `apparmor` |
| Tests, default plus fixture | `rootfs-image-apparmor-test-extension-tarball` | `kata-containers-apparmor-test-extension.img` | `root_hash_apparmor-test-extension.txt` | `apparmor` |

The dedicated CI legs explicitly build the test target after kata-deploy, then
install only the new test image and sidecar with overwrite refusal. The test
tarball is excluded from production bundles. Configuration fails if either test
artifact is absent; it never substitutes the production extension or adds
fixtures to that image.

## Image and runtime requirements

Use a systemd-based guest (`AGENT_INIT=no`). Agent-as-init guests do not run the
extension mount units and are not supported for this feature. The kernel must
provide AppArmor, securityfs, and enabled AppArmor policy hashing. The agent
requires an exported kernel policy digest; it will not fall back to trusting
only a profile name or mode.

SELinux and AppArmor support may be compiled into the same kernel, but that does
not mean both major LSMs enforce policy simultaneously. Select and test separate
boot configurations using the existing `kernel_params` setting. For example:

```text
# AppArmor guest; disable_guest_selinux remains true
lsm=lockdown,yama,loadpin,safesetid,integrity,apparmor,bpf

# SELinux guest; disable_guest_selinux is false
lsm=lockdown,yama,loadpin,safesetid,integrity,selinux,bpf
```

These lists select compiled candidates; check `/sys/kernel/security/lsm` and
actual policy enforcement in the booted guest. A merged kernel configuration
alone is not compatibility evidence. Existing SELinux enablement and policy
remain separate from AppArmor.

Go and Rust QEMU configuration templates contain an opt-in example. The
generated `APPARMOREXTENSIONPATH` points to the extension, not to the base image.
Place extension array tables after all scalar keys in `[hypervisor.qemu]`:

```toml
[[hypervisor.qemu.guest_extension_images]]
name = "apparmor"
path = "/opt/kata/share/kata-containers/kata-containers-apparmor-extension.img"
# Set this to the verity parameters for this exact image when using dm-verity.
verity_params = ""

[agent.kata]
enable_guest_apparmor = true
```

An empty `verity_params` selects an unmeasured extension; it is not an integrity
guarantee. For measured images use the parameters produced with that image.
Content hashes recorded by the agent are identity checks, not signatures or
remote attestation. The guest kernel, agent, base image and administrator-selected
extension are trusted. Guest-kernel or privileged policy-administrator compromise
is outside this boundary.

## Policy root and loading

```text
/run/kata-extensions/apparmor/
├── profiles/<name>
├── abstractions/...
├── tunables/...
├── abi/...
└── metadata/manifest
```

Each requested file must declare exactly the corresponding profile. Includes
and ABI references must be literal paths relative to the policy root, with all
required dependencies present. Absolute paths, parent traversal, variable paths,
symbolic links and nested mounts are rejected. Optional includes must also stay
inside this root. The parser receives both its include and base directories
explicitly and does not use a previously compiled policy cache.
Both the build validator and agent use `--config-file=/dev/null`, so the
distribution's `parser.conf` cannot inject an external ABI or other options.

Under the Sandbox mutex the agent:

1. Verifies the configured, read-only extension and hashes its complete text tree.
2. Rejects an already loaded profile not registered by this agent.
3. Loads a new profile using the parser's add operation, never replace.
4. Requires explicit enforce mode and records the kernel policy digest separately
   from the source-tree SHA-256.
5. Reuses the profile only while its source, mount identity and kernel state match.

Missing or changed state fails closed. The agent does not reload a missing
profile or replace a changed one. Registry entries live for the sandbox lifetime;
deleting a container does not unload policy that another container may use.
An agent that loses the registry cannot adopt the surviving kernel profiles.

## Create and Exec behavior

These rules apply only with `enable_guest_apparmor = true`. At CreateContainer,
absent, empty or `unconfined` final OCI values create an unconfined binding. Every
other valid final OCI name selects **that literal name and policy** in the
extension. The agent saves the binding in container state, not runtime caller
state. Disabled mode keeps existing guest behavior without a Guest AppArmor
binding or policy-loading requirement.

| Exec request | Behavior |
| --- | --- |
| Absent or empty | Inherit the container's saved binding |
| Same literal name | Verify and select the saved profile |
| Different profile, or `unconfined` for a confined container | Reject |
| Named profile for an unconfined container | Reject |

The helper receives the prepared identity over the private setup pipe and does
not load policy. Before mount namespace/rootfs setup it opens and verifies the
real procfs exec attribute, keeping that descriptor for the later selection.
Workload `maskedPaths` or rootfs files cannot turn the selection into a
successful write to a different filesystem. Failure to prepare or select it
aborts process creation; there is no unconfined retry. Container deletion clears
the binding, including when an ID is later reused.

There is no `runtime/default` alias and no `localhost/` prefix stripping at the
OCI/agent boundary. Those strings are not valid literal guest profile names;
as final OCI names they fail validation in enabled mode. Kubernetes API values
and legacy annotations are interpreted by the existing CRI path before that
boundary. This change does not modify containerd, add an annotation, or add an
AppArmor runtime handler.

| Kubernetes request in the containerd 2.2.1 test environment | Final OCI name | Enabled guest behavior |
| --- | --- | --- |
| Omitted `appArmorProfile` | `cri-containerd.apparmor.d` | Load the extension default with that exact name |
| `RuntimeDefault` | `cri-containerd.apparmor.d` | Same name and same guest policy |
| `Unconfined` | Empty/unconfined | Unconfined binding |
| `Localhost`, `localhostProfile: kata-apparmor-test` | `kata-apparmor-test` | Load the test fixture from the test extension |
| `Localhost`, `localhostProfile: cri-containerd.apparmor.d` | `cri-containerd.apparmor.d` | Same default as omitted/RuntimeDefault; no collision override |
| `Localhost`, host accepts but guest policy is absent | Requested literal name | Agent rejects; no workload runs |

Host and guest resolve names in separate kernels. Matching names do not establish
cross-kernel identity; each must use its real policy, and the agent verifies the
guest source and loaded kernel identity. A missing guest default is rejected
just like any other missing guest policy.

### Guest SELinux identity alongside the AppArmor binding

At CreateContainer the agent also records the SELinux context of the process that
init really runs as. That value is read after guest-pull substitution rather than
from the inbound OCI spec: pulling the pause image inside the guest replaces the
whole process, and a recorded identity that does not match init would make Exec
compare itself against the wrong value.

| Registered container identity | Exec request | Behavior |
| --- | --- | --- |
| context | absent or empty | Run in the recorded context |
| context | the same context | Run in the recorded context |
| context | a different context | Reject; no process is created |
| empty (unlabeled container) | any value | Preserve existing behavior: the request is ignored and the child inherits the caller's domain |
| missing entry | any value | Reject as a state error, like a missing AppArmor binding |

`disable_guest_selinux` is decided before a request context is parsed, on the create
path and the exec path alike, so turning the feature off cannot produce an exec-only
validation failure that create never had. When the feature is on, the runtime
normalizes the exec request with the same rules create uses — the host-authored type
becomes the container domain unless `guest_selinux_label` is configured — only so the
agent has a comparable string; the agent owns the decision.

Scope: the unlabeled row keeps existing behavior rather than establishing a binding,
and the caller's domain is not necessarily the container init's domain. Deleting a
container clears its registration, so a reused ID cannot adopt a stale identity, and
an exec that targets a container with no registration fails closed. Out of scope
here: SELinux policy and domain transitions themselves, and privileged containers on
guests where device creation fails before Exec is reached. This adds no SELinux
policy source and is not an escape-prevention guarantee.

## Failure layers and Kubernetes boundary

| Failure | Expected boundary |
| --- | --- |
| Configured extension path is invalid/unreadable | Runtime rejects before VM startup |
| Extension mount or dm-verity fails | Mount unit fails, agent dependency fails, VM powers off |
| Flag enabled without an extension | Runtime/agent rejects the opt-in; no workload starts |
| Flag disabled, no extension, ordinary default Pod | Existing Kata guest behavior; no Guest AppArmor |
| Missing profile/dependency/parser or invalid policy | Agent rejects container creation |
| Unknown source, changed digest, missing policy or non-enforce mode | Agent rejects Create/Exec without replacement |
| AppArmor exec selection fails | Helper aborts before workload exec |

With the flag disabled and no extension configured, normal Kata startup keeps
its existing behavior, including ordinary Pods for which CRI emits its default
profile name. With the flag enabled, no extension is an error even for Unconfined.
Configuring an extension creates an agent `Requires`/`After` dependency; ordering
alone is not treated as mount success.

Containerd CRI can reject `Localhost` on the host before producing an OCI request.
That rejection is host evidence only. Host placeholder policies are not a product
deployment requirement. On disposable CI runners, configure loads the real test
fixture and a renamed copy used only for the guest-negative test. The renamed
copy is absent from the guest test extension. CI never replaces containerd's host
`cri-containerd.apparmor.d` policy.

The earlier default-Pod failure (CRI emitted `cri-containerd.apparmor.d` while
the extension lacked it, and no-extension guests also rejected it) belongs to the
old candidate. The new flag-false baseline and extension default are intended to
resolve that compatibility gap. Their real-guest verification is still pending;
old failures or passes cannot establish this candidate's behavior.

## Validation and limitations

The local checks require Bash and Python 3.11 or newer. They exercise temporary
files and mocked commands only. They
do not mount images, load host policy, contact a cluster or boot a guest:

```bash
bash tests/integration/kubernetes/guest-apparmor-ci-test.sh
bash tests/integration/kubernetes/k8s-apparmor-test.sh
```

The first script asserts both render modes and rejects local execution of every
host-changing CI entrypoint. The renderer checks installed paths, rejects
existing extensions or an existing/later drop-in, and never overwrites output.
Enabled mode explicitly sets `enable_guest_apparmor = true` and selects the
**test** image and `root_hash_apparmor-test-extension.txt`; a production image
cannot substitute. Disabled mode sets the flag to false, requires no extension
assets and preserves the deployed guest image and boot parameters.

Ordinary latest QEMU Go/Rust jobs keep their upstream test collections and run the
flag-false, no-extension default-Pod baseline. The two dedicated opt-in jobs keep
`k8s-apparmor.bats` and configure the test image. Their cases cover omitted,
RuntimeDefault, Unconfined, Localhost fixture, Localhost with the default name,
and separate host/guest missing-policy failures. Every positive case requires
Ready and exact real `/proc/self/attr/current` labels before and after probes,
for app and sidecar, at startup and through standard `kubectl exec`.

The fixture must allow a scratch write and deny its protected path. The default
must allow a scratch write and deny a tmpfs mount; the explicit Unconfined control
gets the same `SYS_ADMIN` capability and must successfully perform that exact
mount. Disabled baseline checks its real unconfined label and an allowed write;
it makes no confinement claim. An unrelated command error is not a denial pass.

Standard `kubectl exec` has no profile field and proves **inheritance only**.
Explicit same-profile, different-profile and downgrade Exec requests require
native OCI/RPC tests and cannot be inferred from Kubernetes results.

Negative tests require a created Pod, no Ready condition or workload container
ID, and an exact profile error from the intended layer in Pod status or
UID-scoped events. An API admission error, image/scheduling failure, arbitrary
NotReady state, wrong profile, host rejection in the guest case, absent parser,
or missing extension cannot pass the guest-missing-policy case. Each invocation
owns its Pod and evidence directory; snapshots and exec output are preserved.
Workflow cleanup verifies drop-in hashes and removes only its temporary host
profiles. Pending approval or configured jobs are not passing CI evidence.

### Current candidate status

The new local baseline is `938166d78`; the checkpoint before interface changes is
`114ee05af`. The public PR HEAD remains `506...` (abbreviated checkpoint supplied
for this work). These identifiers are distinct from the historical candidates
below. This document does not assert that the public PR contains the local work.

All new-candidate integration pass claims are **PENDING TESTING / NOT RUN** here:
flag-false default Pods, enabled default policy, the complete API/Exec matrix,
fail-closed errors, image startup and generated-artifact compatibility require
fresh matching builds and isolated execution. Local script assertions cannot
promote any of those gates to passed. No new upstream CI pass is claimed.

The rebase retains upstream `f7703ac92` synchronous `RemoveContainer` as its
baseline. The planned verification toolchain is official Go 1.26.10 and protoc
21.12 in a separate tool directory; obtaining tools does not establish a completed
build or test and does not replace server system tools.

The Rust SELinux regression gate remains **BLOCKED** on its ABI/toolchain
compatibility issue. **F7 is not-comparable** until compatible, matched artifacts
and evidence exist. This is separate from the historical kernel-only SELinux
mechanism result; neither that result nor an AppArmor pass clears this gate.

### Historical evidence boundaries

| Historical checkpoint | What may be retained | What it cannot prove |
| --- | --- | --- |
| `434...` / earlier 5.15 experiments | Historical mechanism observations only | New flag defaults, literal-name semantics, new images or current CI |
| `6df017821ca77a789bddfcc68a5fbfcb5bebac5f` | The isolated acceptance record reproduced below | Passing results for `938166d78` plus new interface work |
| `r2` | A separate historical run/checkpoint | Any new-candidate pass or a comparable F7 result |

Do not transfer counts, artifact identities or pass labels between these
checkpoints. The following section is a historical report, not a current
candidate acceptance statement.

### Historical isolated results for 6df (2026-10-19)

The historical candidate was built from
`6df017821ca77a789bddfcc68a5fbfcb5bebac5f`, rebased on upstream
`0008d6b9d7e2b6a586883aa05b894563efe86721`. Its source checkpoint, toolchains,
commands, pre/post-strip hashes and unchanged ELF Build IDs were recorded with
those artifacts. The source and interface changes in the new candidate require
new artifacts and new results. The counts below are historical local acceptance
results, not current-candidate or upstream CI results.

The disposable x86-64 node used nested KVM, Kubernetes 1.37.1, containerd 2.2.1
and a 6.18.56 Kata guest kernel. The guest bases were Ubuntu 24.04 Noble and
Debian 13 Trixie, both using systemd. The physical host kernel was unchanged.

| Check | Result and scope |
| --- | --- |
| Native OCI matrix | 20/20: Go/Rust × Noble/Trixie × confinement, masked exec attribute, no-extension unconfined baseline, missing guest policy and invalid host extension path |
| Measured extension corruption | 2/2: each base's console showed verity failure, extension mount failure, blocked agent and poweroff |
| Kubernetes matrix | 4/4: Go/Rust × Noble/Trixie, each including both original BATS tests, a separate host-accepted/guest-rejected policy case and two-container delete/recreate checks |
| SELinux kernel mechanism | Same kernel booted with SELinux selected, loaded a minimal policy, allowed one labeled read and denied another with an enforcing AVC; permissive control succeeded |

Native checks cover init and OCI Exec, omitted/same-profile reuse, rejected
unconfined/different-profile requests, and confinement after the rejected
requests. Kubernetes positive checks require Ready and verify the actual guest
profile, enforce mode and allowed/denied operations for both containers at
startup and through `kubectl exec`. Recreating the Pod must produce a new UID
and repeat those checks. All four suites completed their owned-resource cleanup;
the formal artifact hashes remained unchanged.

The BATS missing-profile test can stop at host CRI validation. The additional
guest-negative case therefore loaded a uniquely named copy of the real fixture
on the disposable host, but not in the extension. It required that exact
profile's rejection from the agent in UID-scoped Pod status/events, with no
workload execution. A generic NotReady state, image failure or host-only
rejection was not accepted as guest evidence. The temporary host policy was
removed after the owned namespace was deleted.

The SELinux check is deliberately narrower than a distro/runtime regression:
it does not validate CentOS `container-selinux`, agent mount/exec labeling or a
production policy. The acceptance agent was a GNU build requiring glibc 2.39;
these results do not cover older guest distributions, official static/musl
builds, other architectures, NVIDIA or agent-as-init guests. They do not clear
the current Rust SELinux ABI block or establish a comparable F7 result. Fresh
upstream workflow execution and verification of the new default-profile contract
remain open gates.

There is **no container-escape prevention guarantee**. Workload confinement is
not a claim that all guest contents remain inaccessible after every container
escape. A least-privilege production profile needs review
against the actual guest tree, mounts, capabilities and profile transitions.
Follow-up testing should expose representative guest files through controlled
namespace/mount scenarios and verify both intended access and denial. A path
denylist alone is insufficient. SELinux domains and labels may better express
guest-system separation, but also need maintained labeling/policy and do not
protect against guest-kernel compromise.
