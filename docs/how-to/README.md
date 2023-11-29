# Howto Guides

## Kubernetes Integration

- [Run Kata containers with `crictl`](run-kata-with-crictl.md)
- [Run Kata Containers with Kubernetes](run-kata-with-k8s.md)
- [How to use Kata Containers and Containerd](containerd-kata.md)
- [How to use Kata Containers and containerd with Kubernetes](how-to-use-k8s-with-containerd-and-kata.md)
- [Kata Containers and service mesh for Kubernetes](service-mesh.md)
- [How to import Kata Containers logs into Fluentd](how-to-import-kata-logs-with-fluentd.md)

## Hypervisors Integration

  Currently supported hypervisors with Kata Containers include:
- `qemu`
- `cloud-hypervisor`
- `firecracker`

   In the case of `firecracker` the use of a block device `snapshotter` is needed
   for the VM rootfs. Refer to the following guide for additional configuration
   steps:
   - [Setup Kata containers with `firecracker`](how-to-use-kata-containers-with-firecracker.md)
- `ACRN`

  While `qemu` , `cloud-hypervisor` and `firecracker` work out of the box with installation of Kata,
  some additional configuration is needed in case of `ACRN`.
  Refer to the following guides for additional configuration steps:
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
- [How to setup swap devices in guest kernel](how-to-setup-swap-devices-in-guest-kernel.md)
- [How to run rootless vmm](how-to-run-rootless-vmm.md)
- [How to run Docker with Kata Containers](how-to-run-docker-with-kata.md)
- [How to run Kata Containers with `nydus`](how-to-use-virtio-fs-nydus-with-kata.md)
- [How to run Kata Containers with AMD SEV-SNP](how-to-run-kata-containers-with-SNP-VMs.md)
- [How to use EROFS to build rootfs in Kata Containers](how-to-use-erofs-build-rootfs.md)
## Confidential Containers
- [How to use build and test the Confidential Containers `CCv0` proof of concept](how-to-build-and-test-ccv0.md)
- [How to generate a Kata Containers payload for the Confidential Containers Operator](how-to-generate-a-kata-containers-payload-for-the-confidential-containers-operator.md)
- [How to use the Kata Agent Policy](how-to-use-the-kata-agent-policy.md)
