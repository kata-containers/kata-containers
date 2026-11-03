# Substrate microVM tests

The normal `ci.yaml` pipeline builds the Substrate agent, rootfs and standard
Kata kernel, then calls `run-substrate-tests.yaml` with their artifact suffix.
This uses the same test gate as the other integration tests, through the
existing PR and nightly entry points.

The `substrate-microvm-e2e` job checks out upstream `main` and runs the Kind
microVM e2e suites against the candidate guest. Substrate provisions its normal
assets, including Cloud Hypervisor and virtiofsd. The Kind job uploads only the
candidate rootfs and standard Kata kernel, then clones Substrate's default
`SandboxConfig` version and replaces those two references. Other assets and
architectures retain their upstream values. The upstream source and asset
cache are unchanged. Test results include both configurations and candidate
SHA-256 hashes.

Before and after the suite, CI checks the live `SandboxConfig` against the
candidate rootfs and kernel hashes. It computes Substrate's bundle hash and
compares it with the default runtime reported by active microVM workers through
`kubectl ate get workers`. Missing workers or a mismatched configuration,
architecture or runtime hash fail the job. The log prints the artifact and
bundle hashes, and the results include the observed configuration, registered
worker runtimes and verification summary for both checks.

The existing amd64 and arm64 build workflows build `agent-substrate` and
`rootfs-image-substrate` in their normal component matrices. Their existing
archive step selects only the Substrate rootfs and standard kernel for
`kata-static-substrate.tar.zst`. Other release archive allowlists exclude
Substrate components. Substrate components use separate artifact names so
kata-deploy's artifact downloads do not collect them.

The normal release workflow publishes
`kata-static-substrate-VERSION-ARCH.tar.zst` alongside Kata's other archives.
Host binaries and other kernel variants are excluded.

The e2e job also runs the guest regression checks before creating the cluster:

```bash
bash tests/integration/substrate/test-artifacts.sh /path/to/candidate-substrate
```

These exercise release archive isolation, all static archive upload commands
with mocked GitHub calls, candidate staging and worker artifact verification.
They run without a cluster or KVM. The staging test uses the extracted
candidate files but mocks the upload and Kubernetes commands. The image build
also runs the userspace ext4 packing checks inside its builder container, using
the same e2fsprogs version as the real image build.

The e2e lane currently runs on x86_64. ARM64 artifacts are built, but runtime
coverage needs a KVM-capable runner with enough available capacity. Substrate
`main` is used to detect upstream compatibility changes, and this e2e job is not
listed among Gatekeeper's required checks.

For local setup and testing, see [Substrate's microVM guide](https://github.com/agent-substrate/substrate/blob/main/docs/dev/microvm-local.md).
The standalone guest boot test is provided by [Substrate PR #1967](https://github.com/agent-substrate/substrate/pull/1967).
