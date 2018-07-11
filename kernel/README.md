# Build Kata Containers Kernel

This document explains the steps to build a compatible kernel with Kata
Containers. To do this use build-kernel.sh, this script automates the
process to build a kernel for Kata Containers. 

## Setup kernel source code

```bash
./build-kernel.sh setup
```

The script `./build-kernel.sh` tries to apply the patches from
`${GOPATH}/src/github.com/kata-containers/packaging/kernel/patches/` when it
sets up a kernel. If you want to add a source modification, add a patch on this
directory.

The script also adds a kernel config file from
`${GOPATH}/src/github.com/kata-containers/packaging/kernel/configs/` to .config
in the kernel source code. You can modify it as needed.

# Build the kernel

After the kernel source code is ready it is possible to build the kernel.

```bash
./build-kernel.sh build
```


## Install the Kernel in the default path for Kata

Kata Containers uses some default path to search a kernel to boot. To install
on this path, the following command will install it to the default Kata
containers path.
```bash
./build-kernel.sh install
```
