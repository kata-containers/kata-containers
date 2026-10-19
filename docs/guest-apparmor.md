# Guest-side AppArmor

This feature applies workload AppArmor profiles inside a Kata guest. It follows
the composable-image direction discussed in
[issue #7586](https://github.com/kata-containers/kata-containers/issues/7586).
The Kubernetes profile-mapping contract is still under upstream discussion.

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

The shipped `kata-apparmor-test` profile is a test fixture, not a production
default or a guest-content isolation policy. No `kata-default` policy is shipped.
NVIDIA integration and a SELinux extension are outside this change.

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

At CreateContainer, absent, empty or `unconfined` values create an unconfined
binding. A valid name selects the corresponding extension profile. The agent
saves the resulting binding in its container state, not in runtime caller state.

| Exec request | Behavior |
| --- | --- |
| Absent or empty | Inherit the container's saved binding |
| Same normalized name | Verify and select the saved profile |
| Different profile, or `unconfined` for a confined container | Reject |
| Named profile for an unconfined container | Reject |

The helper receives the prepared identity over the private setup pipe and does
not load policy. Before mount namespace/rootfs setup it opens and verifies the
real procfs exec attribute, keeping that descriptor for the later selection.
Workload `maskedPaths` or rootfs files cannot turn the selection into a
successful write to a different filesystem. Failure to prepare or select it
aborts process creation; there is no unconfined retry. Container deletion clears
the binding, including when an ID is later reused.

`localhost/<name>` normalization and `runtime/default` to `kata-default` are
transitional compatibility handling, not an approved Kubernetes API. A requested
`kata-default` fails unless the selected extension actually supplies that policy.

## Failure layers and Kubernetes boundary

| Failure | Expected boundary |
| --- | --- |
| Configured extension path is invalid/unreadable | Runtime rejects before VM startup |
| Extension mount or dm-verity fails | Mount unit fails, agent dependency fails, VM powers off |
| Profile requested without an extension | Agent rejects container creation |
| Missing profile/dependency/parser or invalid policy | Agent rejects container creation |
| Unknown source, changed digest, missing policy or non-enforce mode | Agent rejects Create/Exec without replacement |
| AppArmor exec selection fails | Helper aborts before workload exec |

With no extension configured and no profile requested, normal Kata startup is
unchanged. Configuring an extension creates an agent `Requires`/`After`
dependency; ordering alone is not treated as mount success.

Containerd CRI can validate `Localhost` profiles on the host before producing an
OCI request. A host rejection never reaches Kata. Host and guest resolve their
own policy names independently; identical names do not prove identical policies.
Host placeholder profiles are not a supported deployment requirement. CI may
load the real test fixture on a disposable host to exercise this boundary, but
must separately establish enforcement inside the guest.

Upstream decisions are still needed for concrete OCI-name ownership,
`Localhost` host/guest semantics, and `RuntimeDefault` mapping/default policy.
No new Kubernetes annotation or AppArmor-specific runtime handler is introduced.

There is a concrete compatibility issue behind that open decision. In isolated
Kubernetes 1.37.1 / containerd 2.2.1 tests with both runtimes and the Noble guest,
omitting `appArmorProfile` and explicitly choosing `RuntimeDefault` both produced
the OCI name `cri-containerd.apparmor.d`. The agent treated that as a guest policy
name and rejected startup because the extension did not contain it. Separate
no-extension tests with both runtimes also rejected these requests, with the
specific error that the profile was requested but no AppArmor extension was
configured. Explicit `Unconfined` started with an unconfined guest label in both
configurations. These are observations of the current implementation, not
approved default-policy semantics. This is a default Kubernetes compatibility
blocker, including deployments that do not attach the extension. Do not silently
discard the request to make them start. Resolving the default-policy contract
is a merge gate, separate from proving an explicitly selected `Localhost` guest
profile.

## Validation and limitations

Unit tests cover identity registration/reuse, unknown preloaded profiles, changed
source/kernel state, exact mode parsing, include closure, concurrency and Exec
inheritance/downgrade rejection. Shell tests cover rootfs presets, extension
startup dependencies, policy validation and generated runtime configuration.
These tests do not replace a real guest boot.

The opt-in Kubernetes test requires `KATA_GUEST_APPARMOR_TEST=yes`. It must check
readiness, enforce mode, allowed/denied operations, sidecar behavior and
`kubectl exec`. Negative tests must match the expected security error rather than
accepting an arbitrary NotReady pod. Keep per-run workload, runtime, agent and
guest-console logs. Pending workflow approval is not a passing CI result.

Historical 5.15 guest experiments demonstrated a denied write and exposed host
CRI and extension failures. They do not establish the complete current Create/Exec
matrix, successful BATS execution, or compatibility of the current 6.18 kernel.
Those claims require the frozen candidate artifacts and isolated regression run.

### Isolated candidate results (2026-10-19)

The candidate was built from `6df017821ca77a789bddfcc68a5fbfcb5bebac5f`,
rebased on upstream `0008d6b9d7e2b6a586883aa05b894563efe86721`. Later changes
to the test oracle and this report do not change the kernel, agent, runtime or
image-building sources. The source checkpoint, toolchains, commands, pre/post-strip
hashes and unchanged ELF Build IDs are recorded with the candidate artifacts.
These are local acceptance results, not upstream CI results.

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
builds, other architectures, NVIDIA or agent-as-init guests. Upstream workflow
execution and the Kubernetes default-profile contract remain open gates.

Workload confinement is not a claim that all guest contents remain inaccessible
after every container escape. A least-privilege production profile needs review
against the actual guest tree, mounts, capabilities and profile transitions.
Follow-up testing should expose representative guest files through controlled
namespace/mount scenarios and verify both intended access and denial. A path
denylist alone is insufficient. SELinux domains and labels may better express
guest-system separation, but also need maintained labeling/policy and do not
protect against guest-kernel compromise.
