# Kata Containers with IBM Secure Execution VMs

This document assumes a trusted environment with a functioning kata container, as per the
[developer guide](https://github.com/kata-containers/kata-containers/blob/main/docs/Developer-Guide.md).
The term "trusted" implies that the system is authorized, authenticated and attested to use your artifacts and secrets safely.


## Tested Environment Specifications

1. Machine: IBM z16 LPAR
2. OS: Ubuntu 22.04.1 LTS
3. CPU: 16 vCPU
4. Memory: 16G

## Manual configuration

### Prerequisite

1. Host capable of Secure Execution

To take advantage of the IBM Secure Execution capability, the host machine on which
you intend to run workloads must be an IBM z15 (or a newer model) or an IBM LinuxONE III
(or a newer model). In addition to the hardware requirement, you need to verify the CPU
facility and kernel configuration, as outlined below:

```
$ # To check the protected virtualization support from kernel
$ cat /sys/firmware/uv/prot_virt_host
1
$ # To check if an ultravisor reserves memory for the current boot
$ sudo dmesg | grep -i ultravisor
[    0.063630] prot_virt.f9efb6: Reserving 98MB as ultravisor base storage
$ # To check a facility bit for Secure Execution
$ cat /proc/cpuinfo | grep 158
facilities      :  ... numbers ... 158 ... numbers ...
```

If any of the results are not identifiable, please reach out to the responsible cloud
provider to enable the Secure Execution capability. Alternatively, if you possess
administrative privileges and the facility bit is set, you can enable the Secure Execution
capability by adding `prot_virt=1` to the kernel parameters and performing a system reboot like:

```
$ sudo sed -i 's/^\(parameters.*\)/\1 prot_virt=1/g' /etc/zipl.conf
$ sudo zipl -V
$ sudo systemctl reboot
```

Please note that the method of enabling the Secure Execution capability may vary among Linux distributions.

2. Artifacts from Kata Containers

A secure image is constructed using the following artifacts

- A raw kernel
- An initial RAM disk

The most straightforward approach to obtain these artifacts is by reusing kata-containers:

```
$ export PATH="$PATH:/opt/kata/bin"
$ ls -1 $(dirname $(kata-runtime env --json | jq -r '.Kernel.Path'))
config-6.1.62-121
kata-containers.img
kata-containers-confidential.img
kata-containers-initrd.img
kata-containers-initrd-confidential.img
kata-ubuntu-20.04.initrd
kata-ubuntu-20.04-confidential.initrd
kata-ubuntu-latest.image
kata-ubuntu-latest-confidential.image
vmlinux-6.1.62-121
vmlinux-6.1.62-121-confidential
vmlinux.container
vmlinux-confidential.container
vmlinuz-6.1.62-121
vmlinuz-6.1.62-121-confidential
vmlinuz.container
vmlinuz-confidential.container
```

The output indicates the deployment of the kernel (`vmlinux-6.1.62-121-confidential`, though the version
may vary at the time of testing), rootfs-image (`kata-ubuntu-latest-confidential.image`), and rootfs-initrd (`kata-ubuntu-20.04-confidential.initrd`).
In this scenario, the available kernel and initrd can be utilized for a secure image.
However, if any of these components are absent, they must be built from the
[project source](https://github.com/kata-containers/kata-containers) as follows:

```
$ # Assume that the project is cloned at $GOPATH/src/github.com/kata-containers
$ cd $GOPATH/src/github.com/kata-containers/kata-containers
$ make rootfs-initrd-confidential-tarball
$ tar -tf build/kata-static-kernel-confidential.tar.xz | grep vmlinuz
./opt/kata/share/kata-containers/vmlinuz-confidential.container
./opt/kata/share/kata-containers/vmlinuz-6.7-136-confidential
$ kernel_version=6.7-136
$ tar -tf build/kata-static-rootfs-initrd-confidential.tar.xz | grep initrd
./opt/kata/share/kata-containers/kata-containers-initrd-confidential.img
./opt/kata/share/kata-containers/kata-ubuntu-20.04-confidential.initrd
$ mkdir artifacts
$ tar -xvf build/kata-static-kernel-confidential.tar.xz -C artifacts ./opt/kata/share/kata-containers/vmlinuz-${kernel_version}-confidential
$ tar -xvf build/kata-static-rootfs-initrd-confidential.tar.xz -C artifacts ./opt/kata/share/kata-containers/kata-ubuntu-20.04-confidential.initrd
$ ls artifacts/opt/kata/share/kata-containers/
kata-ubuntu-20.04-confidential.initrd  vmlinuz-${kernel_version}-confidential
```

3. Secure Image Generation Tool

`genprotimg` is a utility designed to generate an IBM Secure Execution image. It can be
installed either from the package manager of a distribution or from the source code.
The tool is included in the `s390-tools` package. Please ensure that you have a version
of the tool equal to or greater than `2.17.0`. If not, you will need to specify
an additional argument, `--x-pcf '0xe0'`, when running the command.
Here is an example of a native build from the source:

```
$ sudo apt-get install gcc libglib2.0-dev libssl-dev libcurl4-openssl-dev
$ tool_version=v2.34.0
$ git clone -b $tool_version https://github.com/ibm-s390-linux/s390-tools.git
$ pushd s390-tools/genprotimg && make && sudo make install && popd
$ rm -rf s390-tools
```

4. Host Key Document

A host key document is a public key employed for encrypting a secure image, which is
subsequently decrypted using a corresponding private key during the VM bootstrap process.
You can obtain the host key document either through IBM's designated
[Resource Link](http://www.ibm.com/servers/resourcelink)(you need to log in to access it) or by requesting it from the
cloud provider responsible for the IBM Z and LinuxONE instances where your workloads are intended to run.

To ensure security, it is essential to verify the authenticity and integrity of the host
key document belonging to an authentic IBM machine. To achieve this, please additionally
obtain the following files from the Resource Link:

- IBM Z signing key certificate
- IBM Z host key certificate revocation list
- `DigiCert` intermediate CA certificate

These files will be used for verification during secure image construction in the next section.

### Build a Secure Image

Assuming you have placed a host key document at `$HOME/host-key-document`:

- Host key document as `HKD-0000-0000000.crt`

and two certificates and one revocation list at `$HOME/certificates`:

- IBM Z signing-key certificate as `ibm-z-host-key-signing-gen2.crt`
- `DigiCert` intermediate CA certificate as `DigiCertCA.crt`
- IBM Z host key certificate revocation list as `ibm-z-host-key-gen2.crl`

you can construct a secure image using the following procedure:

```
$ # Change a directory to the project root
$ cd $GOPATH/src/github.com/kata-containers/kata-containers
$ host_key_document=$HOME/host-key-document/HKD-0000-0000000.crt
$ kernel_image=artifacts/opt/kata/share/kata-containers/vmlinuz-${kernel_version}-confidential
$ initrd_image=artifacts/opt/kata/share/kata-containers/kata-ubuntu-20.04-confidential.initrd
$ echo "panic=1 scsi_mod.scan=none swiotlb=262144 agent.log=debug" > parmfile
$ genprotimg --host-key-document=${host_key_document} \
--output=kata-containers-se.img --image=${kernel_image} --ramdisk=${initrd_image} \
--parmfile=parmfile --no-verify
WARNING: host-key document verification is disabled. Your workload is not secured.
$ file kata-containers-se.img
kata-containers-se.img: data
$ sudo cp kata-containers-se.img /opt/kata/share/kata-containers/
```

It is important to note that the `--no-verify` parameter, which allows skipping
the key verification process, is intended to be used solely in a development or
testing environment.
In production, the image construction should incorporate the verification
in the following manner:

```
$ signcert=$HOME/certificates/ibm-z-host-key-signing-gen2.crt
$ cacert=$HOME/certificates/DigiCertCA.crt
$ crl=$HOME/certificates/ibm-z-host-key-gen2.crl
$ genprotimg --host-key-document=${host_key_document} \
--output=kata-containers-se.img --image=${kernel_image} --ramdisk=${initrd_image} \
--cert=${cacert} --cert=${signcert} --crl=${crl} --parmfile=parmfile
```

The steps with no verification, including the dependencies for the kernel and initrd,
can be easily accomplished by issuing the following make target:

```
$ cd $GOPATH/src/github.com/kata-containers/kata-containers
$ mkdir hkd_dir && cp $host_key_document hkd_dir
$ HKD_PATH=hkd_dir SE_KERNEL_PARAMS="agent.log=debug" make boot-image-se-tarball
$ ls build/kata-static-boot-image-se.tar.xz
build/kata-static-boot-image-se.tar.xz
```

`SE_KERNEL_PARAMS` could be used to add any extra kernel parameters. If no additional kernel configuration is required, this can be omitted.

In production, you could build an image by running the same command, but with the
following environment variables for key verification:

```
$ export SIGNING_KEY_CERT_PATH=$HOME/certificates/ibm-z-host-key-signing-gen2.crt
$ export INTERMEDIATE_CA_CERT_PATH=$HOME/certificates/DigiCertCA.crt
$ export HOST_KEY_CRL_PATH=$HOME/certificates/ibm-z-host-key-gen2.crl
```

To build an image on the `x86_64` platform, set the following environment variables together with the variables above before `make boot-image-se-tarball`:

```
CROSS_BUILD=true TARGET_ARCH=s390x ARCH=s390x
```

### Adjust the configuration

There still remains an opportunity to fine-tune the configuration file:

```
$ export PATH=$PATH:/opt/kata/bin
$ runtime_config_path=$(kata-runtime kata-env --json | jq -r '.Runtime.Config.Path')
$ sudo cp ${runtime_config_path} ${runtime_config_path}.old
$ # Make the following adjustment to the original config file
$ diff ${runtime_config_path}.old ${runtime_config_path}
16,17c16,17
< kernel = "/opt/kata/share/kata-containers/vmlinux.container"
< image = "/opt/kata/share/kata-containers/kata-containers.img"
---
> kernel = "/opt/kata/share/kata-containers/kata-containers-se.img"
> # image = "/opt/kata/share/kata-containers/kata-containers.img"
41c41
< # confidential_guest = true
---
> confidential_guest = true
544c544
< dial_timeout = 45
---
> dial_timeout = 90
```

### Verification

To verify the successful decryption and loading of the secure image within a test VM,
please refer to the following commands:

```
$ cd $GOPATH/src/github.com/kata-containers/kata-containers
$ hypervisor_command=$(kata-runtime kata-env --json | jq -r '.Hypervisor.Path')
$ secure_kernel=kata-containers-se.img
$ sudo $hypervisor_command -machine confidential-guest-support=pv0 \
-object s390-pv-guest,id=pv0 -accel kvm -smp 2 --m 4096 -serial mon:stdio \
--nographic --nodefaults --kernel "${secure_kernel}"
[    0.110277] Linux version 5.19.2 (root@637f067c5f7d) (gcc (Ubuntu 11.3.0-1ubuntu1~22.04.1) 11.3.0, GNU ld (GNU Binutils for Ubuntu) 2.38) #1 SMP Wed May 31 09:06:49 UTC 2023                                                                     [    0.110279] setup: Linux is running under KVM in 64-bit mode

... log skipped ...

[    1.467228] Run /init as init process
{"msg":"baremount source=\"proc\", dest=\"/proc\", fs_type=\"proc\", options=\"\", flags=MS_NOSUID | MS_NODEV | MS_NOEXEC","level":"INFO","ts":"2023-06-07T10:17:23.537542429Z","pid":"1","subsystem":"baremount","name":"kata-agent","source":"agent
","version":"0.1.0"}

... log skipped ...

$ # Press ctrl + a + x to exit
```

Unless the host key document is legitimate, you will encounter the following error message:

```
qemu-system-s390x: KVM PV command 2 (KVM_PV_SET_SEC_PARMS) failed: header rc 108 rrc 5 IOCTL rc: -22
Protected boot has failed: 0xa02
```

If the hypervisor log does not indicate any errors, it provides assurance that the image
has been successfully loaded, and a Virtual Machine (VM) initiated by the kata runtime
will function properly.

Let us proceed with the final verification by running a test container in a Kubernetes
cluster. Please make user you have a running cluster like:

```
$ kubectl get node
NAME           STATUS   ROLES                  AGE     VERSION
test-cluster   Ready    control-plane,master   7m28s   v1.23.1
```

Please execute the following command to run a container:

```
$ cat <<EOF | kubectl apply -f -
apiVersion: v1
kind: Pod
metadata:
  name: nginx-kata
spec:
  runtimeClassName: kata-qemu
  containers:
  - name: nginx
    image: nginx
EOF
pod/nginx-kata created
$ kubectl get po
NAME         READY   STATUS    RESTARTS   AGE
nginx-kata   1/1     Running   0          29s
$ kubectl get po -oyaml | grep "runtimeClassName:"
    runtimeClassName: kata-qemu
$ # Please make sure if confidential-guest-support is set and a secure image is used
$ $ ps -ef | grep qemu | grep -v grep
root       76972   76959  0 13:40 ?        00:00:02 /opt/kata/bin/qemu-system-s390x
... qemu arguments ...
-machine s390-ccw-virtio,accel=kvm,confidential-guest-support=pv0
... qemu arguments ...
-kernel /opt/kata/share/kata-containers/kata-containers-se.img
... qemu arguments ...
```

Finally, an operational kata container with IBM Secure Execution is now running.

## Using Kata-Deploy with Confidential Containers Operator

It is reasonable to expect that the manual steps mentioned above can be easily executed.
Typically, you can use
[kata-deploy](https://github.com/kata-containers/kata-containers/blob/main/tools/packaging/kata-deploy/README.md)
to install Kata Containers on a Kubernetes cluster. However, when leveraging IBM Secure Execution,
you need to employ the confidential container's
[operator](https://github.com/confidential-containers/operator).
During this process, a `kata-deploy` container image serves as a payload image in a custom
resource `ccruntime` for confidential containers, enabling the operator to install Kata
binary artifacts such as kernel, shim-v2, and more.

This section will explain how to build a payload image
(i.e., `kata-deploy`) for confidential containers. For the remaining instructions,
please refer to the
[documentation](https://github.com/confidential-containers/confidential-containers/blob/main/guides/ibm-se.md)
for confidential containers.


```
$ cd $GOPATH/src/github.com/kata-containers/kata-containers
$ host_key_document=$HOME/host-key-document/HKD-0000-0000000.crt
$ mkdir hkd_dir && cp $host_key_document hkd_dir
$ # kernel-confidential and rootfs-initrd-confidential are built automactially by the command below
$ HKD_PATH=hkd_dir SE_KERNEL_PARAMS="agent.log=debug" make boot-image-se-tarball
$ make qemu-tarball
$ make virtiofsd-tarball
$ make shim-v2-tarball
$ mkdir kata-artifacts
$ build_dir=$(readlink -f build)
$ cp -r $build_dir/*.tar.xz kata-artifacts
$ ls -1 kata-artifacts
kata-static-agent.tar.xz
kata-static-boot-image-se.tar.xz
kata-static-coco-guest-components.tar.xz
kata-static-kernel-confidential-modules.tar.xz
kata-static-kernel-confidential.tar.xz
kata-static-pause-image.tar.xz
kata-static-qemu.tar.xz
kata-static-rootfs-initrd-confidential.tar.xz
kata-static-shim-v2.tar.xz
kata-static-virtiofsd.tar.xz
$ ./tools/packaging/kata-deploy/local-build/kata-deploy-merge-builds.sh kata-artifacts
```

In production, the environment variables `SIGNING_KEY_CERT_PATH`, `INTERMEDIATE_CA_CERT_PATH`
and `SIGNING_KEY_CERT_PATH` should be exported like the manual configuration.
If a rootfs-image is required for other available runtime classes (e.g. `kata` and
`kata-qemu`) without the Secure Execution functionality, please run the following
command before running `kata-deploy-merge-builds.sh`:

```
$ make rootfs-image-tarball
```

At this point, you should have an archive file named `kata-static.tar.xz` at the project root,
which will be used to build a payload image. If you are using a local container registry at
`localhost:5000`, proceed with the following:

```
$ docker run -d -p 5000:5000 --name local-registry registry:2.8.1
```

Build and push a payload image with the name `localhost:5000/build-kata-deploy` and the tag
`latest` using the following:

```
$ ./tools/packaging/kata-deploy/local-build/kata-deploy-build-and-upload-payload.sh kata-static.tar.xz localhost:5000/build-kata-deploy latest
... logs ...
Pushing the image localhost:5000/build-kata-deploy:latest to the registry
The push refers to repository [localhost:5000/build-kata-deploy]
76c6644d9790: Layer already exists
2413aff53bb1: Layer already exists
91462f44bb06: Layer already exists
2ad49fac591a: Layer already exists
5c75aa64ef7a: Layer already exists
test: digest: sha256:25825c7a4352f75403ee59a683eb122d5518e8ed6a244aacd869e41e2cafd385 size: 1369
```

## Considerations for CI

If you intend to integrate the aforementioned procedure with a CI system,
configure the following setup for an environment variable.
The setup helps speed up CI jobs by caching container images used during the build:

```
$ export BUILDER_REGISTRY=$YOUR_PRIVATE_REGISTRY_FOR_CI
$ export PUSH_TO_REGISTRY=yes
```
