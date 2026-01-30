# Support & Maintenance Levels for Kata Containers Features

## Introduction

Maintainers/Committers are vital to any open-source projects vitality and that is the same for Kata Containers.
Within the project we have code for many components, features and platforms, but not all of these have the
same level of support. This document aims to outline our maintenance levels of components in Kata Containers, so
help users understand what the classification of components means.


## Levels of Support
We have six categories of maintenance/support:

1. Supported: Actively maintained by assigned or paid contributor(s), has updated documentation, upstream CI
to test behaviour and ensure it stays stable
1. Best-effort: Either limited availability from maintainers, lack of support on certain environments,
missing/outdated documentation, or lacking CI tests to ensure its stability
1. Not maintained: No current maintainers, but the community is open to receive them. May be moved into deprecated
1. Experimental: A new(ish) feature that hasn't reach maturity yet. Not recommended for use
1. Deprecated: Deprecated and unsupported. Recommended to move away from it. Likely to be removed
1. Obsolete: To be removed.


## Becoming a maintainer

To become a maintainer, first become a [committer](https://github.com/kata-containers/community/blob/main/README.md#committer)
and then volunteer to join the maintainer team for one, or more features. Reach out to the Kata Containers
Architecture Committee, or other admins to be added to, or removed from teams.

## Support classification
*Note: github teams may need updating*
| "Feature" | Support State | Maintainers | Notes |
| --- | --- | --- | --- |
| **Architectures** | |  ||
| amd64 | Supported | [@kata-containers/arch-amd64](https://github.com/orgs/kata-containers/teams/arch-amd64) | |
| s390x | Supported | [@kata-containers/arch-s390x](https://github.com/orgs/kata-containers/teams/arch-s390x) | s390x only works with the qemu hypervisor and not all features are supported on this platform. |
| arm64 (aarch64) | Best-effort | [@kata-containers/arch-aarch64](https://github.com/orgs/kata-containers/teams/arch-aarch64) | arm64 only works with the qemu hypervisor and not all features are supported on this platform. Limit CI runners. |
| ppc64le | Best-effort | [@kata-containers/arch-ppc64le](https://github.com/orgs/kata-containers/teams/arch-ppc64le) | ppc64le only works with the qemu hypervisor and not all features are supported on this platform. ppc64le doesn't currently support the runtime-rs runtime. Limit CI runners for e2e tests. |
| risc-v | Experimental | [@kata-containers/arch-riscv](https://github.com/orgs/kata-containers/teams/arch-riscv) | Still WIP, not all components buildable, No reliable CI |
| darwin | Deprecated? | | Limited testing to build a few runtime packages and genpolicy (which Markus maintains)?|
| |
| **TEE** | | | (Trusted Execution environment) |
| IBM Secure Execution for Linux (SEL) | Supported | [@kata-containers/arch-s390x](https://github.com/orgs/kata-containers/teams/arch-s390x) | There is no public SEL (the s390x TEE) runner, so the CI is run downstream and the results publish upstream [nightly](https://github.com/kata-containers/kata-containers/actions/workflows/ci-nightly-s390x.yaml).|
| AMD SEV-SNP | Best effort | [@kata-containers/amd-snp](https://github.com/orgs/kata-containers/teams/amd-snp) | The project has CI and active development, but no current committers. Not tested on runtimes-rs. Limit CI runners. |
| Intel TDX | Best effort | [@kata-containers/intel-tdx](https://github.com/orgs/kata-containers/teams/intel-tdx) | The project has CI and active maintainers, but not tested on runtimes-rs. Limit CI runners. |
| ARM CCA | Experimental | [@kata-containers/arch-aarch64](https://github.com/orgs/kata-containers/teams/arch-aarch64) | Very limited code currently |
| |
| **Hypervisors** | | | |
| qemu | Supported | [@kata-containers/qemu](https://github.com/orgs/kata-containers/teams/qemu) | Widely supported across all architectures and multiple TEEs |
| dragonball | Best effort | [@kata-containers/dragonball](https://github.com/orgs/kata-containers/teams/dragonball) | Not actively developed. Only supported with runtime-rs on amd64 architecture |
| cloud-hypervisor | Not maintained | [@kata-containers/cloud-hypervisor](https://github.com/orgs/kata-containers/teams/cloud-hypervisor) | No currently identified maintainers. CI limited to the go runtime |
| firecracker | Not maintained | [@kata-containers/firecracker](https://github.com/orgs/kata-containers/teams/firecracker) | Some development. No CI testing or runtime-rs support |
| |
| **Tools** | | | |
| genpolicy | Supported | [@kata-containers/genpolicy](https://github.com/orgs/kata-containers/teams/genpolicy) | Tested in the CI on multiple platforms |
| kata-deploy | Supported | [@kata-containers/kata-deploy](https://github.com/orgs/kata-containers/teams/kata-deploy) | Tested in the CI on all supported platforms |
| agent-ctl | Not maintained | | Some CI testing |
| kata-ctl | Deprecated? | | Meant as a replacement of kata runtime. No CI testing or maintainers |
| kata-manager | Deprecated? | | No CI testing |
| kata-monitor | Deprecated? | | Limited CI testing |
| log-parser | Obsolete? | | No CI testing |
| trace-forwarder | Obsolete? | | No CI testing. Long term CVEs |
| vsock-exporter | Obsolete? | | No mainainers, no testing? Has CVEs |
| |
| **Rootfs base Operating System** | | | |
| ubuntu | Supported |  | Tested in the CI on all platforms |
| cbl-mariner | Supported | | Tested in the CI on all x86 |
| alpine | Obsolete? | | No CI testing |
| centos | Obsolete? | | No CI testing |
| debian | Obsolete? | | No CI testing |
| |
| **Tests** | | | |
| Build checks | Supported |  | Tested in the CI on all platforms |
| cri-containerd tests | Supported |  | Tested in the CI on multiple platforms |
| K8s tests | Supported |  | Tested in the CI on multiple platforms |
| CoCo Stability | Obsolete? | | Not maintained and disabled for 7 months |
| Basic-CI | Obsolete? | | No CI testing |
| Containerd-sandbox API | Experimental | | No CI testing. Blocked by https://github.com/containerd/containerd/issues/11640 |
| Containerd stability | Not maintained | | CI testing on multiple platforms, but not updated |
| nydus | Best effort? | | CI testing on multiple platforms, but not updated |
| run-tracing | Obsolete? | | Not maintained. CI disabled for over 18 months |
| run-vfio | Obsolete? | | Not maintained. CI disabled for over 18 months |
| nerdctl | Not maintained | | Not maintained. CI runs against multiple hypervisors |
| darwin tests | Not maintained | | Not maintained. CI runs against multiple hypervisors |
| docs url alive check | Obsolete | | Not maintained. Never passed on github |
| metrics tests | Obsolete | | Not maintained. Test runner not available |
| kata-monitor tests | Deprecated? | | Not maintained. Only running against crio with qemu |
| docker tests | Not maintained | | Not maintained. Not working for many months |
| static checks | Deprecated? | | Not maintained, a huge file with many tests, some of which have value. Should be reviewed and split up? |
| |
| **Runtime Variants** | | | |
| Golang Runtime | Supported | [@kata-containers/runtime](https://github.com/orgs/kata-containers/teams/runtime) | Legacy runtime with complete feature set. Extensive CI coverage across all architectures. Planned for deprecation around Q4 2026, with removal in Kata Containers 5.0 |
| runtime-rs (Rust) | Best effort? | [@kata-containers/runtime-rs](https://github.com/orgs/kata-containers/teams/runtime-rs) | Incomplete feature set compared to Go runtime and not all hypervisors and platforms supported yet, but under development with the gap to the go runtime closing |
| |
| **Container Runtime Integrations** | | | |
| Containerd | Supported | | Extensive CI across multiple platforms and with two containerd versions (LTS, Active). |
| Kubernetes | Supported | | Extensive CI across multiple platforms, with varying levels of testing across multiple K8s platforms e.g. AKS, kubeadm, k3s, rke2, k0s, microk8s |
| CRI-O | Not maintained | [@kata-containers/cri-o](https://github.com/orgs/kata-containers/teams/cri-o) | Tests disabled in CI as of 2024. Minimal recent maintenance. Documentation exists but limited active support |
| |

### Table TODO
- Add configuration options e.g. snapshotters

## Doc TODOs

Do we want to use the CODEOWNER/MAINTAINER file to document these rather than this page?
