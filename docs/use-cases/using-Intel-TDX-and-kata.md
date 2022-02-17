# Kata Containers with TDX

Intel® Trust Domain Extensions (TDX) introduce new, architectural
elements to help deploy hardware-isolated, virtual machines (VMs)
called trust domains (TDs). See
[Intel® Trust Domain Extensions (Intel® TDX)](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-trust-domain-extensions.html)
for more information.

## Preconditions

* Intel TDX capable bare metal nodes
* Guest kernel with TDX support
* Hypervisor with TDX support (Cloud Hypervisor or QEMU)
* Host kernel Linux with TDX support(https://github.com/intel/tdx/tree/kvm)

> NOTE:
> At the time of writing, full TDX support is not available in the
> upstream Linux kernel so a custom host kernel must be built and installed.

```sh
$ grep TDX /boot/config-`uname -r`
CONFIG_KVM_INTEL_TDX=y
$ grep -qom1 tdx /proc/cpuinfo && echo "TDX available"
```

## Installation

### Kata Containers TDX Guest Kernel

Run the following commands to install guest kernel with TDX support:

```sh
$ latest=$(curl http://jenkins.katacontainers.io/job/kata-containers-2.0-kernel-tdx-x86_64-nightly/lastSuccessfulBuild/artifact/artifacts/latest)
$ curl -L http://jenkins.katacontainers.io/job/kata-containers-2.0-kernel-tdx-x86_64-nightly/lastSuccessfulBuild/artifact/artifacts/vmlinuz-${latest} -o vmlinuz-tdx.container
$ sudo mv vmlinuz-tdx.container /usr/share/kata-containers/vmlinuz-tdx.container
```

### Kata Containers TDX QEMU

Run the following command to install QEMU with TDX support:

```sh
$ curl http://jenkins.katacontainers.io/job/kata-containers-2.0-qemu-tdx-x86_64/lastSuccessfulBuild/artifact/artifacts/kata-static-qemu.tar.gz | sudo tar --strip-components=1 -C /usr/local/ -zxf -
```

### Firmware with TDX support

Refer to [TDVF](https://github.com/tianocore/edk2-staging/tree/TDVF) to build
and install a firmware with Intel® TDX support.

### Kata Containers Configuration

#### Configuration for QEMU

Edit `/usr/share/defaults/kata-containers/configuration.toml` and apply
the following configuration (look for the sections and change their values accordingly):

```toml
[hypervisor.qemu]
path = "/usr/local/bin/qemu-system-x86_64"
kernel = "/usr/share/kata-containers/vmlinuz-tdx.container"
confidential_guest = true
kernel_params = "force_tdx_guest tdx_disable_filter"
firmware = "PATH to your TDX firmware"
cpu_features= "pmu=off,-kvm-steal-time"
disable_image_nvdimm = true
```

## Usage

### Run a Kata TDX container

```sh
sudo ctr run --rm --runtime io.containerd.run.kata.v2 -t --rm docker.io/library/busybox:latest hello dmesg | grep -i tdx
[    0.000000] tdx: Force enabling TDX Guest feature
[    0.000000] TDX: Disabled TDX guest filter support
[    0.000000] tdx: Guest initialized
```
