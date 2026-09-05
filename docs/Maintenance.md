# Support & Maintenance Levels for Kata Containers Features

## Introduction

Maintainers and committers are vital to any open-source project's vitality and the same is true for Kata Containers.
Within the project, we have code for many components, features, and platforms, but not all of them have the
same level of support. This document aims to outline the maintenance levels of components in Kata Containers, to
help users understand the classification of each components.

## Levels of Support

We have five categories of maintenance/support:

1. Supported: Actively maintained by assigned or paid contributor(s), has updated documentation, upstream CI
  to test behaviour and ensure it stays stable
1. Best effort: Either limited availability from maintainers, lack of support on certain environments,
  missing/outdated documentation, or lacking CI tests to ensure its stability
1. Not maintained: No current maintainers, but the community is open to receive them. May be moved into deprecated
1. Experimental: A new(ish) feature that hasn't reached maturity yet. Not recommended for use
1. Deprecated: Deprecated and unsupported. Recommended to move away from it. Might have a date/release target for removal

## Becoming a maintainer

To become a maintainer, first become a [committer](https://github.com/kata-containers/community/blob/main/README.md#committer)
and then volunteer to join the maintainer team for one, or more features. Reach out to the Kata Containers
Architecture Committee, or other admins to be added to, or removed from teams.

## Support classification

| "Feature" | Support State | Maintainers | Notes |
| --- | --- | --- | --- |
| **Architectures** | |  | |
| amd64 | Supported | [@kata-containers/arch-amd64](https://github.com/orgs/kata-containers/teams/arch-amd64) | |
| s390x | Supported | [@kata-containers/arch-s390x](https://github.com/orgs/kata-containers/teams/arch-s390x) | s390x only works with the qemu hypervisor and not all features are supported on this platform. |
| arm64 (aarch64) | Best effort | [@kata-containers/arch-aarch64](https://github.com/orgs/kata-containers/teams/arch-aarch64) | arm64 only works with the qemu hypervisor and not all features are supported on this platform. Limit CI runners. |
| ppc64le | Best effort | [@kata-containers/arch-ppc64le](https://github.com/orgs/kata-containers/teams/arch-ppc64le) | ppc64le only works with the qemu hypervisor and not all features are supported on this platform. ppc64le doesn't currently support the runtime-rs runtime. Limit CI runners for e2e tests. |
| risc-v | Experimental | [@kata-containers/arch-riscv](https://github.com/orgs/kata-containers/teams/arch-riscv) | Still WIP, not all components buildable, No reliable CI |
| darwin | Best effort | [@kata-containers/arch-darwin](https://github.com/orgs/kata-containers/teams/arch-darwin) | Only for developer tools (`genpolicy`), the Darwin runtime is unmaintained. |
| | | | |
| **TEE** | | | (Trusted Execution environment) |
| IBM Secure Execution for Linux (SEL) | Supported | [@kata-containers/arch-s390x](https://github.com/orgs/kata-containers/teams/arch-s390x) | There is no public SEL (the s390x TEE) runner, so the CI is run downstream and the results publish upstream [nightly](https://github.com/kata-containers/kata-containers/actions/workflows/ci-nightly-s390x.yaml).|
| Intel TDX | Supported | [@kata-containers/intel-tdx](https://github.com/orgs/kata-containers/teams/intel-tdx) | The project has CI and active maintainers, but limited CI runners. |
| AMD SEV-SNP | Best effort | [@kata-containers/amd-snp](https://github.com/orgs/kata-containers/teams/amd-snp) | The project has CI and active development, but no active maintainers. |
| ARM CCA | Experimental | [@kata-containers/arch-aarch64](https://github.com/orgs/kata-containers/teams/arch-aarch64) | Very limited code currently |
| | | | |
| **Hypervisors** | | | |
| qemu | Supported | [@kata-containers/qemu](https://github.com/orgs/kata-containers/teams/qemu) | Widely supported across all architectures and multiple TEEs |
| cloud-hypervisor | Supported | [@kata-containers/cloud-hypervisor](https://github.com/orgs/kata-containers/teams/cloud-hypervisor) | Only supported on amd64 architecture |
| openVMM | Supported | [@kata-containers/openvmm](https://github.com/orgs/kata-containers/teams/openvmm) | Only supported with runtime-rs on amd64 architecture |
| dragonball | Supported | [@kata-containers/dragonball](https://github.com/orgs/kata-containers/teams/dragonball) | Only supported with runtime-rs on amd64 architecture |
| firecracker | Not maintained | [@kata-containers/firecracker](https://github.com/orgs/kata-containers/teams/firecracker) | Some development, but limited to the go runtime. No CI testing |
| remote | Not maintained | | Remote hypervisor with an active implementation in [cloud-api-adaptor](https://github.com/confidential-containers/cloud-api-adaptor). No Kata CI, no testing for runtime-rs |
| | | | |
| **Tools** | | | |
| genpolicy | Supported | [@kata-containers/genpolicy](https://github.com/orgs/kata-containers/teams/genpolicy) | Tested in the CI on multiple platforms |
| kata-deploy | Supported | [@kata-containers/kata-deploy](https://github.com/orgs/kata-containers/teams/kata-deploy) | Tested in the CI on all supported platforms |
| agent-ctl | Not maintained | | Some CI testing. No maintainers |
| kata-ctl | Not maintained | | Meant as a replacement of kata-runtime. No CI testing or maintainers |
| kata-manager | Deprecated | | No CI testing. Better to re-write using "deploy-rs" code if we want to support non-k8s deployments |
| kata-monitor | Not maintained | | |
| kata-runtime | Deprecated | | No CI testing. To be replaced by kata-ctl. |
| log-parser | Deprecated | | No CI testing. To be replaced by kata-ctl's log-parse implementation. |
| trace-forwarder | Deprecated | | No CI testing. Long term CVEs |
| vsock-exporter | Deprecated | | No maintainers, no testing? Has CVEs |
| | | | |
| **Rootfs base Operating System** | | | |
| ubuntu | Supported | [@kata-containers/builder](https://github.com/orgs/kata-containers/teams/builder) | Tested in the CI on all platforms |
| cbl-mariner | Supported | [@kata-containers/cbl-mariner](https://github.com/orgs/kata-containers/teams/cbl-mariner) | Tested in the CI on all x86 |
| alpine | Deprecated | | No CI testing |
| centos | Deprecated | | No CI testing |
| debian | Deprecated | | No CI testing |
| | | | |
| **Tests** | | | |
| Build checks | Supported | [@kata-containers/tests](https://github.com/orgs/kata-containers/teams/tests) | Tested in the CI on all platforms |
| cri-containerd tests | Supported | [@kata-containers/tests](https://github.com/orgs/kata-containers/teams/tests) | Tested in the CI on multiple platforms |
| K8s tests | Supported | [@kata-containers/tests](https://github.com/orgs/kata-containers/teams/tests) | Tested in the CI on multiple platforms |
| nydus | Not maintained | [@kata-containers/nydus](https://github.com/orgs/kata-containers/teams/nydus) | No maintainers. CI testing on multiple platforms, but not updated |
| Containerd-sandbox API | Not maintained | | No identified maintainers |
| Containerd stability | Not maintained | | CI testing on multiple platforms, but not updated |
| nerdctl | Not maintained | | No identified maintainers |
| darwin tests | Not maintained | | Not maintained |
| kata-monitor tests | Not maintained | | Not maintained |
| docker tests | Not maintained | | No identified maintainers |
| static checks | Not maintained | | Not maintained, a huge file with many tests, some of which have value. Should be reviewed and split up? |
| Basic-CI | Deprecated | | No CI testing |
| | | | |
| **Runtime Variants** | | | |
| Golang Runtime | Supported | [@kata-containers/runtime](https://github.com/orgs/kata-containers/teams/runtime) | Legacy runtime with complete feature set. Extensive CI coverage across all architectures. Planned for deprecation around Q4 2026, with removal in Kata Containers 5.0 |
| runtime-rs (Rust) | Supported | [@kata-containers/runtime-rs](https://github.com/orgs/kata-containers/teams/runtime-rs) | |
| | | | |
| **Container Runtime Integrations** | | | |
| Containerd | Supported | [@kata-containers/tests](https://github.com/orgs/kata-containers/teams/tests) | Extensive CI across multiple platforms and with two containerd versions (LTS, Active). |
| Kubeadm | Supported | [@kata-containers/tests](https://github.com/orgs/kata-containers/teams/tests) | Extensive CI across multiple platforms |
| AKS | Supported |  | Extensive CI across x86 |
| k0s | Not maintained | | Only CI testing of install with kata-deploy |
| rke2 | Not maintained | | Only CI testing of install with kata-deploy |
| `microk8s` | Not maintained | | Only CI testing of install with kata-deploy |
| k3s | Not maintained | | Only CI testing of install with kata-deploy |
| CRI-O | Deprecated | [@kata-containers/cri-o](https://github.com/orgs/kata-containers/teams/cri-o) | Tests disabled in CI as of 2024. Minimal recent maintenance. Documentation exists but no active support |
| | | | |
