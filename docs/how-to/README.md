# Howto Guides

* [Howto Guides](#howto-guides)
  * [Kubernetes Integration](#kubernetes-integration)
  * [Hypervisors Integration](#hypervisors-integration)
  * [Advanced Topics](#advanced-topics)

## Kubernetes Integration
- [Run Kata containers with `crictl`](run-kata-with-crictl.md)
- [Run Kata Containers with Kubernetes](run-kata-with-k8s.md)
- [How to use Kata Containers and Containerd](containerd-kata.md)
- [How to use Kata Containers and CRI (containerd plugin) with Kubernetes](how-to-use-k8s-with-cri-containerd-and-kata.md)
- [Kata Containers and service mesh for Kubernetes](service-mesh.md)
- [How to import Kata Containers logs into Fluentd](how-to-import-kata-logs-with-fluentd.md)

## Hypervisors Integration

  Currently supported hypervisors with Kata Containers include:
- `qemu`
- `cloud-hypervisor`
- `firecracker`
- `ACRN`

  While `qemu` and `cloud-hypervisor` work out of the box with installation of Kata,
  some additional configuration is needed in case of `firecracker` and `ACRN`.
  Refer to the following guides for additional configuration steps:
- [Kata Containers with Firecracker](https://github.com/kata-containers/documentation/wiki/Initial-release-of-Kata-Containers-with-Firecracker-support)
- [Kata Containers with ACRN Hypervisor](how-to-use-kata-containers-with-acrn.md)

## Advanced Topics
- [How to use Kata Containers with virtio-fs](how-to-use-virtio-fs-with-kata.md)
- [Setting Sysctls with Kata](how-to-use-sysctls-with-kata.md)
- [What Is VMCache and How To Enable It](what-is-vm-cache-and-how-do-I-use-it.md)
- [What Is VM Templating and How To Enable It](what-is-vm-templating-and-how-do-I-use-it.md)
- [Privileged Kata Containers](privileged.md)
- [How to load kernel modules in Kata Containers](how-to-load-kernel-modules-with-kata.md)
- [How to use Kata Containers with `virtio-mem`](how-to-use-virtio-mem-with-kata.md)
- [How to set sandbox Kata Containers configurations with pod annotations](how-to-set-sandbox-config-kata.md)
- [How to monitor Kata Containers in K8s](how-to-set-prometheus-in-k8s.md)
- [How to use hotplug memory on arm64 in Kata Containers](how-to-hotplug-memory-arm64.md)
