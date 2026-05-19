# msft-preview Rebase Guide

This file is a reusable guide for rebasing `msft-preview` onto future upstream
releases.

Use it as an operating checklist, not as a one-time record for a specific
release.

## Rebase Setup

Before starting a rebase, record the exact upstream target and create a safety
branch.

- Working branch: `msft-preview`
- Upstream target: `<upstream-tag-or-commit>`
- Safety branch: `msft-preview-pre-rebase-<upstream-tag>-<timestamp>`

Suggested setup:

```bash
cd <kata-repo>
git checkout msft-preview
git branch "msft-preview-pre-rebase-<upstream-tag>-$(date +%Y%m%d-%H%M%S)"
```

## Behaviors That Must Survive Future Rebases

### 1. Runtime-go static resource behavior

Preserve all of the following in `src/runtime`:

- If `static_sandbox_resource_mgmt=true` and the workload does not specify limits, use the configured defaults:
  - `static_sandbox_default_workload_mem`
  - `static_sandbox_default_workload_vcpus`
- If workload limits are specified, do not add extra CPU or memory on top of those limits.
- Do not reintroduce `sandbox_workload_mem_min` enforcement in runtime-go.
  - The fork intentionally reverted the hard `128MiB` runtime-side minimum.
  - The webhook still clamps too-small limits separately.

Primary files:

- `src/runtime/pkg/oci/utils.go`
- `src/runtime/pkg/katautils/config.go`
- `src/runtime/config/configuration-clh.toml.in`
- `src/runtime/Makefile`

### 2. Runtime-rs static resource behavior

Preserve all of the following in `src/runtime-rs`:

- If static resource management is enabled and workload resources are unset, fall back to:
  - `static_sandbox_default_workload_mem`
  - `static_sandbox_default_workload_vcpus`
- If workload limits are specified, do not add extra CPU or memory beyond those workload limits.
- In the static-resource path, keep `default_maxvcpus` clamped to the effective static total.
- Every runtime-rs hypervisor config template that enables `static_sandbox_resource_mgmt` must
  also carry `static_sandbox_default_workload_mem` and `static_sandbox_default_workload_vcpus`.
  These are fork-added keys and default to `0` when absent (`#[serde(default)]`). Upstream rebases
  tend to keep them in `configuration-clh-runtime-rs.toml.in` but drop them from the other
  templates — most notably `configuration-dragonball.toml.in`. When missing, the fallback resolves
  `vcpus` to `0`, so `default_maxvcpus` becomes `0` and dragonball fails to boot with
  `vmm action error: MachineConfig(InvalidVcpuCount(0))` (seen on the
  `run-nerdctl-tests (dragonball)` CI job). Keep the keys in every static-resource template.

Primary files:

- `src/runtime-rs/crates/resource/src/cpu_mem/initial_size.rs`
- `src/runtime-rs/config/configuration-clh-runtime-rs.toml.in`
- `src/runtime-rs/config/configuration-dragonball.toml.in`
- `src/runtime-rs/Makefile`

### 3. Fork-only test and CI policy

Preserve the fork-specific CI and test exceptions unless the fork
infrastructure changes:

- `msft-preview` branch targeting in GitHub workflows.
- Disabled jobs for runners the fork does not have.
- Disabled `required-tests` entries for unsupported AKS, s390x, GPU, and GHCR-dependent jobs.
- Injected sandbox memory limits in some integration tests.
- Disabled dragonball runtime-rs tests that fail with `MachineConfig(InvalidVcpuCount(0))`.

Primary files:

- `.github/workflows/ci.yaml`
- `.github/workflows/docs.yaml`
- `.github/workflows/static-checks-self-hosted.yaml`
- `tools/testing/gatekeeper/required-tests.yaml`
- `tests/integration/cri-containerd/integration-tests.sh`
- `tests/integration/nydus/nydus-sandbox.yaml`

## Changes That Often Do Not Need Replaying

Do not mechanically replay old fork commits just because they existed on the
previous branch tip. Re-evaluate them against the new upstream target.

The following categories often drop cleanly or should be rechecked before being
reapplied:

- Obsolete Cloud Hypervisor pinning to fork-only repositories.
- Changes that were already upstreamed in a newer form.
- Formatting-only commits.
- Regenerated client code if upstream already regenerated the same surface.
- Local fixes that upstream later implemented differently but with equivalent behavior.

Rule: only replay a fork commit if the behavior is still required and missing
from the new upstream base.

## Conflict Hotspots And What To Do

### `.gitignore`

Keep both:

- newer upstream ignores
- Microsoft-specific osbuilder and IGVM ignores

### `versions.yaml`

Default to upstream Cloud Hypervisor references from the new release tag.

Do not re-pin to `microsoft/cloud-hypervisor` unless the fork explicitly needs
it again.

### Workflow branch conflicts

When upstream changes workflow structure and the fork changes branch names, keep
both:

- upstream workflow structure
- fork branch target `msft-preview`

### `tools/testing/gatekeeper/required-tests.yaml`

This file changes frequently upstream. Re-express the fork skips against the
current upstream job names instead of mechanically replaying old lines.

The fork currently expects these skip categories:

- unsupported runners
- unsupported k8s environments
- GHCR-dependent jobs
- low-memory test incompatibilities
- dragonball runtime-rs `InvalidVcpuCount(0)` failures

### `src/runtime/pkg/katautils/config.go`

Two common pitfalls:

- keep newer upstream fields added to the `runtime` struct
- do not reintroduce `SandboxWorkloadMemMin`

### `src/runtime-rs/Makefile`

Keep both:

- the explicit `cargo build -p runtime-rs`
- the fork's `RELEASE_CARGO_PROFILE_ENV` release-profile overrides

Also do a final manual check for stray conflict markers in this file after the
rebase completes. This file has been a repeat conflict hotspot.

### `tools/osbuilder/node-builder/azure-linux/common.sh`, `package_build.sh`, `package_install.sh`

Keep the Azure Linux node-builder shim config names aligned with the runtime
Makefiles and config directories:

- runtime-go release config: `configuration-clh.toml`
- runtime-go debug config: `configuration-clh-debug.toml`
- runtime-rs release config: `configuration-clh-runtime-rs.toml`
- runtime-rs debug config: `configuration-clh-runtime-rs-debug.toml`

Notes:

- `src/runtime-rs/Makefile` is the source of truth for the release config name.
- the runtime-rs debug config is created by `tools/osbuilder/node-builder/azure-linux/package_build.sh` by copying the release config and enabling debug options.

Quick verification:

```bash
cd <kata-repo>
grep -n 'CONFIG_FILE_CLH = configuration-clh-runtime-rs.toml' src/runtime-rs/Makefile
grep -n 'SHIM_CONFIG_FILE_NAME_RUNTIME_GO\|SHIM_CONFIG_FILE_NAME_RUNTIME_RS' tools/osbuilder/node-builder/azure-linux/common.sh
bash -n tools/osbuilder/node-builder/azure-linux/common.sh tools/osbuilder/node-builder/azure-linux/package_build.sh tools/osbuilder/node-builder/azure-linux/package_install.sh
```

### Static build scripts

In `tools/packaging/static-build/cloud-hypervisor/build-static-clh.sh`, keep:

- `rm -rf cloud-hypervisor`

If upstream changes the copy command, keep the newer upstream structure and only
preserve the cleanup behavior if still needed.

## Focused Validation Commands

Run these after the rebase:

```bash
cd <kata-repo>
cargo test -p resource --manifest-path src/runtime-rs/Cargo.toml --lib initial_size -- --nocapture
```

```bash
cd <kata-repo>/src/runtime
go test ./pkg/oci ./pkg/katautils
```

Useful inspection commands:

```bash
cd <kata-repo>
git log --oneline --reverse <upstream-tag>..msft-preview
git grep -n 'static_sandbox_default_workload_mem\|static_sandbox_default_workload_vcpus\|InvalidVcpuCount\|sandbox_workload_mem_min' -- src/runtime src/runtime-rs tools/testing/gatekeeper/required-tests.yaml tests/integration
```

Before pushing, scan tracked files for unresolved rebase markers:

```bash
cd <kata-repo>
git grep -nE '^(<<<<<<<|=======|>>>>>>>)'
```

If the only hits are banner lines inside scripts or help text, they are safe.
Any true start-of-line conflict marker in source or config files must be
removed before committing or pushing.

## Rebase Review Checklist

Use this checklist before you push:

1. Confirm the branch still preserves the fork-specific resource-sizing behavior in runtime-go and runtime-rs.
2. Confirm CI still targets `msft-preview` and still skips unsupported jobs.
3. Confirm `required-tests.yaml` skips match current upstream job names.
4. Confirm node-builder config filenames still match the runtime Makefiles.
5. Confirm there are no stray conflict markers.
6. Confirm the validation commands above pass, or document any known failures.

## Practical Rule For Future Rebases

Prefer this order of decisions:

1. Keep upstream if the fork change was already upstreamed.
2. Keep fork behavior for resource sizing in runtime-go and runtime-rs.
3. Keep fork CI and test skips when the limitation still exists.
4. Avoid replaying stale Cloud Hypervisor pinning commits.
5. Re-express `required-tests` skips against current job names instead of preserving old labels verbatim.
