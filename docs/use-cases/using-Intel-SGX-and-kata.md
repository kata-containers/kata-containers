# Kata Containers with SGX

- [Check if SGX is enabled](#check-if-sgx-is-enabled)
- [Install Host kernel with SGX support](#install-host-kernel-with-sgx-support)
- [Install Guest kernel with SGX support](#install-guest-kernel-with-sgx-support)
- [Run Kata Containers with SGX enabled](#run-kata-containers-with-sgx-enabled)

Intel® Software Guard Extensions (SGX) is a set of instructions that increases the security
of applications code and data, giving them more protections from disclosure or modification.

> **Note:** At the time of writing this document, SGX patches have not landed on the Linux kernel
> project, so specific versions for guest and host kernels must be installed to enable SGX.

## Check if SGX is enabled

Run the following command to check if your host supports SGX.

```sh
$ grep -o sgx /proc/cpuinfo
```

Continue to the following section if the output of the above command is empty,
otherwise continue to section [Install Guest kernel with SGX support](#install-guest-kernel-with-sgx-support)

## Install Host kernel with SGX support

The following commands were tested on Fedora 32, they might work on other distros too.

```sh
$ git clone --depth=1 https://github.com/intel/kvm-sgx
$ pushd kvm-sgx
$ cp /boot/config-$(uname -r) .config
$ yes "" | make oldconfig
$ # In the following step, enable: INTEL_SGX and INTEL_SGX_VIRTUALIZATION
$ make menuconfig
$ make -j$(($(nproc)-1)) bzImage
$ make -j$(($(nproc)-1)) modules
$ sudo make modules_install
$ sudo make install
$ popd
$ sudo reboot
```

> **Notes:**
> * Run: `mokutil --sb-state` to check whether secure boot is enabled, if so, you will need to sign the kernel.
> * You'll lose SGX support when a new distro kernel is installed and the system rebooted.

Once you have restarted your system with the new brand Linux Kernel with SGX support, run
the following command to make sure it's enabled. If the output is empty, go to the BIOS
setup and enable SGX manually.

```sh
$ grep -o sgx /proc/cpuinfo
```

## Install Guest kernel with SGX support

Install the guest kernel in the Kata Containers directory, this way it can be used to run
Kata Containers.

```sh
$ curl -LOk https://github.com/devimc/kvm-sgx/releases/download/v0.0.1/kata-virtiofs-sgx.tar.gz
$ sudo tar -xf kata-virtiofs-sgx.tar.gz -C /usr/share/kata-containers/
$ sudo sed -i 's|kernel =|kernel = "/usr/share/kata-containers/vmlinux-virtiofs-sgx.container"|g' \
  /usr/share/defaults/kata-containers/configuration.toml
```

## Run Kata Containers with SGX enabled

Before running a Kata Container make sure that your version of `crio` or `containerd`
supports annotations.
For `containerd` check in `/etc/containerd/config.toml` that the list of `pod_annotations` passed
to the `sandbox` are: `["io.katacontainers.*", "sgx.intel.com/epc"]`.

> `sgx.yaml`
```yaml
apiVersion: v1
kind: Pod
metadata:
  name: sgx
  annotations:
    sgx.intel.com/epc: "32Mi"
spec:
  terminationGracePeriodSeconds: 0
  runtimeClassName: kata
  containers:
  - name: c1
    image: busybox
    command:
        - sh
    stdin: true
    tty: true
    volumeMounts:
    - mountPath: /dev/sgx/
      name: test-volume
  volumes:
  - name: test-volume
    hostPath:
      path: /dev/sgx/
      type: Directory
```

```sh
$ kubectl apply -f sgx.yaml
$ kubectl exec -ti sgx ls /dev/sgx/
enclave    provision
```

The output of the latest command shouldn't be empty, otherwise check
your system environment to make sure SGX is fully supported.

[1]: github.com/cloud-hypervisor/cloud-hypervisor/
