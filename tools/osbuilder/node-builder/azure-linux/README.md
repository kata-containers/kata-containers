# Overview

This guide serves as a reference on how to build and install the underlying software stack for *Pod Sandboxing with AKS* and for *Confidential Containers on AKS* using Azure Linux.
This enables running Kata (Confidential) Containers via the OCI interface, or via a local kubelet, or leveraging AKS' Kubernetes solution.

In the following, the terms *Kata* and *Kata-CC* refer to *Pod Sandboxing with AKS* and *Confidential Containers on AKS*, respectively.
The term *building* refers to build the components from source, whereas the term *installing* refers to utilizing components released by the Azure Linux team for straightforward evaluation.

The guide provides the steps for two different environments:
- Azure Linux 3 based systems, such as Azure VMs
  - Variant I: Utilize released components
  - Variant II: Build components from source
- AKS nodes (based on Azure Linux 2 as of today)

# Steps for Azure Linux 3 based environments

## Set up AzL3 environment

While build can happen in any Azure Linux 3 based environment, the stack can only be evaluated on environments with proper virtualization support and, for Kata-CC, on top of AMD SEV-SNP. An example of such environment are Azure Linux 3 based Azure VMs using a proper SKU:
- Deploy an Azure Linux 3 VM via `az vm create` using a [CC vm size SKU](https://learn.microsoft.com/en-us/azure/virtual-machines/dcasccv5-dcadsccv5-series)
  - Example: `az vm create --resource-group <rg_name> --name <vm_name> --os-disk-size-gb <e.g. 60> --public-ip-sku Standard --size <e.g. Standard_DC4as_cc_v5> --admin-username azureuser --ssh-key-values <ssh_pubkey> --image <MicrosoftCBLMariner:azure-linux-3:azure-linux-3-gen2:latest>`
- SSH onto the VM

Not validated for evaluation: Install [Azure Linux 3](https://github.com/microsoft/azurelinux) on a bare metal machine supporting AMD SEV-SNP.

To merely build the stack, we refer to the official [Azure Linux GitHub page](https://github.com/microsoft/azurelinux) to set up an Azure Linux 3 environment.

## Deploy required host packages (incl. VMM, SEV-SNP capable kernel and Microsoft Hypervisor) and extend containerd configuration

Install relevant packages, append a configuration snippet to `/etc/containerd/config.toml` to register the Kata(-CC) handlers, then reboot the system:
```
sudo dnf -y makecache
sudo dnf -y install kata-packages-host

sudo tee -a /etc/containerd/config.toml 2&>1 <<EOF

[plugins."io.containerd.grpc.v1.cri".containerd.runtimes.kata]
  runtime_type = "io.containerd.kata.v2"
  privileged_without_host_devices = true
  [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.kata.options]
    ConfigPath = "/usr/share/defaults/kata-containers/configuration.toml"
[proxy_plugins]
  [proxy_plugins.tardev]
    type = "snapshot"
    address = "/run/containerd/tardev-snapshotter.sock"
[plugins."io.containerd.grpc.v1.cri".containerd.runtimes.kata-cc]
  snapshotter = "tardev"
  runtime_type = "io.containerd.kata-cc.v2"
  privileged_without_host_devices = true
  [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.kata-cc.options]
    ConfigPath = "/opt/confidential-containers/share/defaults/kata-containers/configuration-clh-snp.toml"
EOF

sudo reboot
```

## Variant I: Utilize released components to assemble the UVM

While the priorly installed `kata-packages-host` package delivers all host-side components, the tools required to assemble the UVM components are delivered through the `kata-packages-uvm-build` package.
Using this package, it is straightforward to assemble the UVM and then to run pods.

For Kata:
```
sudo dnf -y install kata-packages-uvm-build
pushd /opt/kata-containers/uvm/tools/osbuilder/node-builder/azure-linux
sudo make uvm
sudo make deploy-uvm
popd
```

For Kata-CC:
```
sudo dnf -y install kata-packages-uvm-build curl jq # curl and jq are only required for installing the IGVM tool
pushd /opt/confidential-containers/uvm/tools/osbuilder
pushd igvm-builder
sudo ./igvm_builder.sh -i
popd
pushd node-builder/azure-linux
# Note: see explanation on AGENT_POLICY_FILE below. We build with a permissive agent policy
# as security policy annotations part of the pod metadata field are not passed to the shim.
# This setup SHOULD NOT BE USED for Confidential Containers in production.
sudo make AGENT_POLICY_FILE=allow-all.rego uvm-confpods
sudo make deploy-confpods-uvm
popd
popd
# Note: currently depends on kubelet, need to manually start at every reboot.
sudo systemctl start tardev-snapshotter
```

You environment is ready. Continue with section *Run Kata (Confidential) Containers*

## Variant II: Build components from source

### Install AzL3 build dependencies

```
sudo dnf -y install git golang rust cargo build-essential protobuf-compiler protobuf-devel expect openssl-devel clang-devel libseccomp-devel btrfs-progs-devel device-mapper-devel cmake fuse-devel kata-packages-uvm-build
```

Continue with the section *Build the Kata(-CC) host and guest components from source and install*.

# Steps for AKS nodes

## Set up AKS environment

- Deploy a [Confidential Containers for AKS cluster](https://learn.microsoft.com/en-us/azure/aks/deploy-confidential-containers-default-policy) via `az aks create` (using `AzureLinux` as `os-sku`). Note, this way the bits built in this guide will already be present on the cluster's Azure Linux based nodes.
- Deploy a debugging pod onto one of the nodes
- From the debugging pod, SSH onto the node you intend to use to build on.

As released components are already pre-installed onto AKS nodes, the remainder of this section focuses on how to (re-)build the components from source.

## Install AKS build dependencies

```
sudo dnf -y makecache
sudo dnf -y install git golang rust cargo build-essential protobuf-compiler protobuf-devel expect openssl-devel clang-devel libseccomp-devel btrfs-progs-devel device-mapper-devel cmake fuse-devel kata-packages-uvm-build parted qemu-img kernel-uvm-devel curl jq # curl and jq are only required for installing the IGVM tool
```

From here on, continue with the following section to build.

# Build the Kata(-CC) host and guest components from source and install

Clone the Microsoft's fork of the kata-containers repository:

```git clone https://github.com/microsoft/kata-containers.git```

## Install IGVM tooling for ConfPods

When intending to build the components for Confidential Containers, install the IGVM tool that will be used by the build tooling to create IGVM files with their reference measurements for the ConfPods UVM.

```
pushd kata-containers/tools/osbuilder/igvm-builder
sudo ./igvm_builder.sh -i
popd
```

This command installs the latest release of the [IGVM tooling](https://github.com/microsoft/igvm-tooling/) using `pip3 install`. The tool can be uninstalled at any time by calling the script using the -u parameter instead.

## Build and deploy

To build and install Kata components, run:
```
pushd kata-containers/tools/osbuilder/node-builder/azure-linux
make all
sudo make deploy
popd
```

To build and install Kata-CC components, use the `all-confpods` and `deploy-confpods` targets:
```
pushd kata-containers/tools/osbuilder/node-builder/azure-linux
make all-confpods
sudo make deploy-confpods
popd
```

The `all[-confpods]` target runs the targets `package[-confpods]` and `uvm[-confpods]` in a single step (the `uvm[-confpods]` target depends on the `package[-confpods]` target). The `deploy[-confpods]` target moves the build artifacts to proper places (and calls into `deploy[-confpods]-package`, `deploy[-confpods]-uvm`).

Notes:
  - To retrieve more detailed build output, prefix the make commands with `DEBUG=1`.
  - To build an IGVM file for CondPods with a non-default SVN of 0, prefix the `make uvm-confpods` command with `IGVM_SVN=<number>`
  - For build and deployment of both Kata and Kata-CC artifacts, first run the `make all` and `make deploy` commands to build and install the Kata Containers for AKS components followed by `make clean`, and then run `make all-confpods` and `make deploy-confpods` to build and install the Confidential Containers for AKS components - or vice versa (using `make clean-confpods`).

## Debug builds

This section describes how to build and deploy in debug mode.

`make all-confpods` takes the following variables:

 * `AGENT_BUILD_TYPE`: Specify `release` (default) to build the agent in
   release mode, or `debug` to build it in debug mode.
 * `AGENT_POLICY_FILE`: Specify `allow-set-policy.rego` (default) to use
   a restrictive policy, or `allow-all.rego` to use a permissive policy.

`make deploy-confpods` takes the following variable:

 * `SHIM_USE_DEBUG_CONFIG`: Specify `no` (default) to use the production
   configuration, or `yes` to use the debug configuration (all debug
   logging enabled). In this case you'll want to enable debug logging
   in containerd as well. Note that this variable has no effect if
   `SHIM_REDEPLOY_CONFIG=no`.

In general, you can specify the debug configuration for all the above
variables by using `BUILD_TYPE=debug` as such:

```shell
sudo make BUILD_TYPE=debug all-confpods deploy-confpods
```

Also note that make still lets you override the other variables even
after setting `BUILD_TYPE`. For example, you can use the production shim
config with `BUILD_TYPE=debug`:

```shell
sudo make BUILD_TYPE=debug SHIM_USE_DEBUG_CONFIG=no all-confpods deploy-confpods
```

### Prevent redeploying the shim configuration

If you're manually modifying the shim configuration directly on the host
during development and you don't want to redeploy and overwrite that
file each time you redeploy binaries, you can separately specify the
`SHIM_REDEPLOY_CONFIG` (default `yes`):

```shell
sudo make SHIM_REDEPLOY_CONFIG=no all-confpods deploy-confpods
```

Note that this variable is independent from the other variables
mentioned above. So if you want to avoid redeploying the shim
configuration AND build in debug mode, you have to use the following
command:

```shell
sudo make BUILD_TYPE=debug SHIM_REDEPLOY_CONFIG=no all-confpods deploy-confpods
```

# Run Kata (Confidential) Containers

## Run via CRI or via containerd API

Use e.g. `crictl` (or `ctr`) to schedule Kata (Confidential) containers, referencing either the Kata or Kata-CC handlers.

Note: On Kubernetes nodes, pods created via `crictl` will be deleted by the control plane.

The following instructions serve as a general reference:
- Install `crictl`, `cni` binaries, and set runtime endpoint in `crictl` configuration:

  ```
  sudo dnf -y install cri-tools cni
  sudo crictl config --set runtime-endpoint=unix:///run/containerd/containerd.sock
  ```

- Set a proper CNI configuration and create a sample pod manifest: This step is omitted as it depends on the individual needs.

- Run pods with `crictl`, for example:

  `sudo crictl runp -T 30s -r <handler-name> <sample-pod.yaml>`

- Run containers with `ctr`, for example a confidential container:

  `sudo ctr -n=k8s.io image pull --snapshotter=tardev docker.io/library/busybox:latest`

  `sudo ctr -n=k8s.io run --cni --runtime io.containerd.run.kata-cc.v2 --runtime-config-path /opt/confidential-containers/share/defaults/kata-containers/configuration-clh-snp.toml --snapshotter tardev -t --rm docker.io/library/busybox:latest hello sh`

For further usage we refer to the upstream `crictl` (or `ctr`) and CNI documentation.

## Run via Kubernetes

If your environment was set up through `az aks create` the respective node is ready to run Kata (Confidential) Containers as AKS Kubernetes pods.
Other types of Kubernetes clusters should work as well. While this document doesn't cover how to set-up those clusters, you can
apply the kata and kata-cc runtime classes to your cluster from the machine that holds your kubeconfig file, for example:
```
cat << EOF > runtimeClass-kata-cc.yaml
kind: RuntimeClass
apiVersion: node.k8s.io/v1
metadata:
    name: kata-cc
handler: kata-cc
overhead:
    podFixed:
        memory: "600Mi"
scheduling:
  nodeSelector:
    katacontainers.io/kata-runtime: "true"
EOF

cat << EOF > runtimeClass-kata.yaml
kind: RuntimeClass
apiVersion: node.k8s.io/v1
metadata:
    name: kata
handler: kata
overhead:
    podFixed:
        memory: "600Mi"
scheduling:
  nodeSelector:
    katacontainers.io/kata-runtime: "true"
EOF

kubectl apply -f runtimeClass-kata-cc.yaml -f runtimeClass-kata.yaml
```

And label your node appropriately:
```
kubectl label node <nodename> katacontainers.io/kata-runtime=true
```

# Build attestation scenarios
The build artifacts for the UVM ConfPods target include an IGVM file and a so-called reference measurement file (unsigned). The IGVM file is being loaded into memory measured by the AMD SEV-SNP PSP (when a Confidential Container is started). With this and with the Kata security policy feature, attestation scenarios can be built: the reference measurement (often referred to as 'endorsement') can, for example, be signed by a trusted party (such as Microsoft in Confidential Containers on AKS) and be compared with the actual measurement part of the attestation report. The latter can be retrieved through respective system calls inside the Kata Confidential Containers Guest VM.

An example for an attestation scenario through Microsoft Azure Attestation is presented in [Attestation in Confidential containers on Azure Container Instances](https://learn.microsoft.com/en-us/azure/container-instances/confidential-containers-attestation-concepts).
Documentation for leveraging the Kata security policy feature can be found in [Security policy for Confidential Containers on Azure Kubernetes Service](https://learn.microsoft.com/en-us/azure/confidential-computing/confidential-containers-aks-security-policy).
