# Guest AppArmor policy extension

This directory holds the production AppArmor policy in `policy/` and the
separate integration fixture in `test-policy/`. Both extension images carry
policy text only. Production ships `cri-containerd.apparmor.d`; the opt-in
test image adds `kata-apparmor-test` to the same default policy.

The kata-agent loads workload AppArmor profiles from this image inside the
guest. The profile *parser* is not here: `apparmor_parser` and its runtime
dependencies belong to the base image. See
[Guest-side AppArmor](../../../docs/guest-apparmor.md) for the lifecycle,
configuration, security boundary and validation requirements.

## Why an extension and not the base image

Profiles are policy, and policy changes far more often than the base
operating system does. Shipping them in the base image would mean rebuilding
and redistributing a whole rootfs for every policy change. A guest extension
image is attached at VM start and mounted read-only before the agent runs, so
it can be versioned, reviewed and replaced independently.

## Layout: the extension root is a policy root

The contents of `policy/` become the root of the mounted extension, and the
agent passes that root to `apparmor_parser` as its include search path:

```text
policy/                        ->  /run/kata-extensions/apparmor/
├── profiles/                      profiles/<name>     requested by name
├── abstractions/                  abstractions/...    #include <abstractions/...>
├── tunables/                      tunables/global     #include <tunables/...>
├── local/                         local/...           site specific includes
└── metadata/manifest              version and provenance
```

Everything a profile can include must live under this root. Sharing
`abstractions/` or `tunables/` with the base image would make the meaning of a
profile depend on whichever distribution release the base image happens to
carry, which is the coupling this layout exists to avoid.

## Profile files

A file under `profiles/` is named after the profile it is expected to define.
The agent requests a profile by name, loads `profiles/<name>`, and then
verifies from the kernel that a profile with that name is loaded and in
enforce mode. It also records source and kernel policy digests, refusing
unknown preloaded policies or a changed identity. A file
named `kata-apparmor-test` that declares `profile something-else` will not
satisfy a request for `kata-apparmor-test`.

## What belongs here

Only reviewed policy text. Not the parser, not shared libraries, not
distribution packages.

The production `policy/profiles/cri-containerd.apparmor.d` is derived from the
`defaultTemplate` in [containerd v2.2.1](https://github.com/containerd/containerd/blob/v2.2.1/contrib/apparmor/template.go).
The vendored template literal and original Apache-2.0 license live in
`upstream/`. Copyright notices for the Docker, Moby and containerd authors,
source URLs, snapshot SHA256s and the modification description ship in
`policy/metadata/default-profile-source.txt`. The full license ships as
`policy/metadata/LICENSE.containerd`, prefixed with AppArmor comments so the
entire policy tree can pass the same include validator.

The render parameters are fixed:

| Parameter | Value |
| --- | --- |
| Name | `cri-containerd.apparmor.d` |
| PROC | `/proc/` (explicit `@{PROC}=/proc/` preamble) |
| Imports / InnerImports | Empty |
| DaemonProfile | `unconfined` |
| RootlessKit | Empty |

`render-default-profile.py` reads only the vendored literal and writes UTF-8
policy to stdout. It does not call containerd's host-dependent `loadData()`,
inspect host policies or process state, search for RootlessKit, or read
environment variables. Reproduce the checked-in profile offline:

```bash
python3 tools/packaging/apparmor-extension/render-default-profile.py > /tmp/cri-containerd.apparmor.d
cmp /tmp/cri-containerd.apparmor.d tools/packaging/apparmor-extension/policy/profiles/cri-containerd.apparmor.d
```

All upstream baseline rules are retained, including the proc/sys restrictions,
mount denial, signal peer rules and same-profile ptrace permissions. This
baseline grants broad network, capability and file access subject to explicit
denials. It is not a complete container escape defense. Placing it in the
extension makes it available by name; selecting and applying it is the
agent/runtime's responsibility.

## Build and release interfaces

Run from the repository root:

```bash
make rootfs-image-apparmor-extension-tarball
make rootfs-image-apparmor-test-extension-tarball
```

| Artifact | Production | Test only |
| --- | --- | --- |
| Image | `kata-containers-apparmor-extension.img` | `kata-containers-apparmor-test-extension.img` |
| Hash | `root_hash_apparmor-extension.txt` | `root_hash_apparmor-test-extension.txt` |
| Tarball | `kata-static-rootfs-image-apparmor-extension.tar.zst` | `kata-static-rootfs-image-apparmor-test-extension.tar.zst` |
| Profiles | `cri-containerd.apparmor.d` | Same default plus `kata-apparmor-test` |

The Docker wrapper writes component tarballs under
`tools/packaging/kata-deploy/local-build/build/`. Inside each tarball, images
and hashes are under `opt/kata/share/kata-containers/`. Production build and
release allowlists select the production extension on x86_64 and aarch64.
The test target is explicit and excluded from the production bundles.
The Base remains `kata-static-rootfs-image.tar.zst` with `root_hash_base.txt`;
neither AppArmor image replaces or embeds that Base.

A custom policy root containing `profiles/` and all required includes remains
supported. Pass it explicitly through Make, using a directory inside the
repository so it is visible through the existing Docker mount:

```bash
make rootfs-image-apparmor-extension-tarball APPARMOR_POLICY_DIR="$PWD/my-policy"
```

Direct `kata-deploy-binaries.sh` callers can use
`--build=rootfs-image-apparmor-extension --apparmor-policy-dir=/absolute/policy/root`
or `APPARMOR_POLICY_DIR`. Shell callers may also pass the root as the first
argument to `install_image_apparmor_extension`. Custom roots replace the
shipped production policy tree; they must supply any default policy their
workloads need. The production target rejects `profiles/kata-apparmor-test`.
The test target adds the tracked fixture without overwriting an existing one.

Every source root and the final assembled tree pass the same closure and
declaration checks. Validation uses explicit parser options, compiles rules
before reading names, and keeps stderr warnings separate from name stdout.
A cold parser builder gets an isolated context with the complete COPY inputs.
The image builder receives `FS_TYPE=erofs`, `SKIP_DAX_HEADER=yes` and
`MEASURED_ROOTFS=yes` (the existing s390x exception remains nonmeasured).
Fresh image output and variant-specific verity metadata are required before
installing artifacts. Temporary trees are removed on success and failure.

## Interface for the Kubernetes test agent

Build and install the **test** tarball above for fixture-based tests. Configure
the extension with logical name `apparmor`, image path
`/opt/kata/share/kata-containers/kata-containers-apparmor-test-extension.img`,
and, when required, the verity parameters from
`root_hash_apparmor-test-extension.txt`. Do not select the production image
for tests requesting `kata-apparmor-test`.

The guest policy root remains `/run/kata-extensions/apparmor/`; its `profiles/`
directory has both `cri-containerd.apparmor.d` and `kata-apparmor-test`.
The fixture source is now `test-policy/profiles/kata-apparmor-test`, while the
default source is `policy/profiles/cri-containerd.apparmor.d`. The fixture
retains its real denial of writes/locks to `/tmp/kata-apparmor-deny`. Test
assembly copies the default policy bytes directly from the production tree.

For a real image check, verify its data/hash partitions using the test hash
file with `veritysetup verify`, mount the EROFS data read-only, compare both
profiles with these sources and compile them with the Base image's parser.
The offline packaging tests use a fake image builder and do not establish
real dm-verity or guest enforcement results.

## Offline checks

```bash
bash tools/packaging/kata-deploy/local-build/tests/apparmor-policy-test.sh
bash tools/packaging/kata-deploy/local-build/tests/apparmor-extension-test.sh
bash tools/packaging/kata-deploy/local-build/tests/apparmor-build-entry-test.sh
```

These checks cover parser declarations and include closure, deterministic
default rendering, full license retention, production/test separation,
custom roots, clean parser builder inputs, failure cleanup, fresh output and
verity metadata, and Make/release manifest selection. A locally installed
`apparmor_parser` additionally compiles the real policies without loading
anything into the host kernel.
