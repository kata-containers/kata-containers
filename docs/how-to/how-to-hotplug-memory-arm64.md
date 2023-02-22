# How to use memory hotplug feature in Kata Containers on arm64

## Introduction

Memory hotplug is a key feature for containers to allocate memory dynamically in deployment.
As Kata Container bases on VM, this feature needs support both from VMM and guest kernel. Luckily, it has been fully supported for the current default version of QEMU and guest kernel used by Kata on arm64. For other VMMs, e.g, Cloud Hypervisor, the enablement work is on the road. Apart from VMM and guest kernel, memory hotplug also depends on ACPI which depends on firmware either. On x86, you can boot a VM using QEMU with ACPI enabled directly, because it boots up with firmware implicitly. For arm64, however, you need specify firmware explicitly. That is to say, if you are ready to run a normal Kata Container on arm64, what you need extra to do is to install the UEFI ROM before use the memory hotplug feature.

## Install UEFI ROM

We have offered a helper script for you to install the UEFI ROM. If you have installed Kata normally on your host, you just need to run the script as fellows:

```bash
$ pushd $GOPATH/src/github.com/kata-containers/tests
$ sudo .ci/aarch64/install_rom_aarch64.sh
$ popd
```

## Config KATA QEMU

After executing the above script, two files will be generated under the directory `/usr/share/kata-containers/` by default, namely `kata-flash0.img` and `kata-flash1.img`. Next we need to change the configuration file of `kata qemu`, which is in `/opt/kata/share/defaults/kata-containers/configuration-qemu.toml` by default, specify in the configuration file to use the UEFI ROM installed above. The above is an example of `kata deploy` installation. For package management installation, please use `kata-runtime env` to find the location of the configuration file. Please refer to the following configuration.

```
[hypervisor.qemu]

# -pflash can add image file to VM. The arguments of it should be in format
# of ["/path/to/flash0.img", "/path/to/flash1.img"]
pflashes = ["/usr/share/kata-containers/kata-flash0.img", "/usr/share/kata-containers/kata-flash1.img"]
```

## Run for test

Let's test if the memory hotplug is ready for Kata after install the UEFI ROM. Make sure containerd is ready to run Kata before test.

```bash
$ sudo ctr image pull docker.io/library/ubuntu:latest
$ sudo ctr run --runtime io.containerd.run.kata.v2 -t --rm docker.io/library/ubuntu:latest hello sh -c "free -h"
$ sudo ctr run --runtime io.containerd.run.kata.v2 -t --memory-limit 536870912 --rm docker.io/library/ubuntu:latest hello sh -c "free -h"
```

Compare the results between the two tests. If the latter is 0.5G larger than the former, you have done what you want, and congratulation!
