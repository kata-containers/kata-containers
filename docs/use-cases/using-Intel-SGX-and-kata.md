# Kata Containers with SGX

Intel Software Guard Extensions (SGX) is a set of instructions that increases the security
of applications code and data, giving them more protections from disclosure or modification.

This document guides you to run containers with SGX enclaves with Kata Containers in Kubernetes.

## Preconditions

* Intel SGX capable bare metal nodes
* Host kernel Linux 5.13 or later with SGX and SGX KVM enabled:

```sh
$ grep SGX /boot/config-`uname -r`
CONFIG_X86_SGX=y
CONFIG_X86_SGX_KVM=y
```

* Kubernetes cluster configured with:
   * [`kata-deploy`](https://github.com/kata-containers/kata-containers/tree/main/tools/packaging/kata-deploy) based Kata Containers installation
   * [Intel SGX Kubernetes device plugin](https://github.com/intel/intel-device-plugins-for-kubernetes/tree/main/cmd/sgx_plugin#deploying-with-pre-built-images)

> Note: Kata Containers supports creating VM sandboxes with Intel® SGX enabled
> using [cloud-hypervisor](https://github.com/cloud-hypervisor/cloud-hypervisor/) VMM only. QEMU support is waiting to get the
> Intel SGX enabled QEMU upstream release.

## Installation

### Kata Containers Guest Kernel

Follow the instructions to [setup](../../tools/packaging/kernel/README.md#setup-kernel-source-code) and [build](../../tools/packaging/kernel/README.md#build-the-kernel) the experimental guest kernel. Then, install as:

```sh
$ sudo cp kata-linux-experimental-*/vmlinux /opt/kata/share/kata-containers/vmlinux.sgx
$ sudo sed -i 's|vmlinux.container|vmlinux.sgx|g' \
  /opt/kata/share/defaults/kata-containers/configuration-clh.toml
```

### Kata Containers Configuration

Before running a Kata Container make sure that your version of `crio` or `containerd`
supports annotations.

For `containerd` check in `/etc/containerd/config.toml` that the list of `pod_annotations` passed
to the `sandbox` are: `["io.katacontainers.*", "sgx.intel.com/epc"]`.

## Usage

With the following sample job deployed using `kubectl apply -f`:

```yaml
apiVersion: batch/v1
kind: Job
metadata:
  name: oesgx-demo-job
  labels:
    jobgroup: oesgx-demo
spec:
  template:
    metadata:
      labels:
        jobgroup: oesgx-demo
    spec:
      runtimeClassName: kata-clh
      initContainers:
        - name: init-sgx
          image: busybox
          command: ['sh', '-c', 'mkdir /dev/sgx; ln -s /dev/sgx_enclave /dev/sgx/enclave; ln -s /dev/sgx_provision /dev/sgx/provision']
          volumeMounts:
          - mountPath: /dev
            name: dev-mount
      restartPolicy: Never
      containers:
        -
          name: eosgx-demo-job-1
          image: oeciteam/oe-helloworld:latest
          imagePullPolicy: IfNotPresent
          securityContext:
            readOnlyRootFilesystem: true
            capabilities:
              add: ["IPC_LOCK"]
          resources:
            limits:
              sgx.intel.com/epc: "512Ki"
      volumes:
        - name: dev-mount
          hostPath:
            path: /dev
```

You'll see the enclave output:

```sh
$ kubectl logs oesgx-demo-job-wh42g
Hello world from the enclave
Enclave called into host to print: Hello World!
```

### Notes

* The Kata VM's SGX Encrypted Page Cache (EPC) memory size is based on the sum of `sgx.intel.com/epc`
resource requests within the pod.
* `init-sgx` can be removed from the YAML configuration file if the Kata rootfs is modified with the
necessary udev rules.
   See the [note on SGX backwards compatibility](https://github.com/intel/intel-device-plugins-for-kubernetes/tree/main/cmd/sgx_plugin#backwards-compatibility-note).
* Intel SGX DCAP attestation is known to work from Kata sandboxes but it comes with one limitation: If
the Intel SGX `aesm` daemon runs on the bare metal node and DCAP `out-of-proc` attestation is used,
containers within the Kata sandbox cannot get the access to the host's `/var/run/aesmd/aesm.sock`
because socket passthrough is not supported. An alternative is to deploy the `aesm` daemon as a side-car
container.
* Projects like [Gramine Shielded Containers (GSC)](https://gramine-gsc.readthedocs.io/en/latest/) are
also known to work. For GSC specifically, the Kata guest kernel needs to have the `CONFIG_NUMA=y`
enabled and at least one CPU online when running the GSC container.
