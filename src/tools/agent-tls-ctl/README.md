# Agent TLS Control tool

## Overview

The Kata Containers agent TLS control tool (`kata-agent-tls-ctl`) is derived from the [`kata-agent-ctl`](../agent-tls-ctl/) tool. This tool communicates over a grpc tls channel with the `kata-agent` that runs inside the virtual machine (VM). Similar to the `kata-agent-ctl` tool, the same warning applies; this tool is for advance users!

## Build from Source

Since the agent is written in the Rust language this section assumes the tool
chain has been installed using standard Rust`rustup`tool.

### Prerequisites

Similar to the `kata-agent-ctl`, this tool requires an OCI bundle, please see `kata-agent-ctl`'s [prerequisites](../agent-ctl/README.md/#prerequisites).

The tool also requires a set of client public and private key pair and the
server's CA public key certificate to establish a TLS connection with the `kata-agent`.
```bash
$ export key_dir=${KATA_DIR}/src/agent/grpc_tls_keys 
````

 > [!NOTE]
 > The `kata-agent` depends on the KBS to provision cryptographic keys to the split API proxy server for establishing a secure channel. For testing the API proxy server,  create a zip file named, `tls-keys.zip`, containing the CA public key and the server’s public and private key pair.
```bash
$ cd $key_dir
$ zip tls-keys.zip server.pem server.key ca.pem
````
> Place the `tls-keys.zip`  file in the KBS resource path `/default/tenant-keys/`.  At sandbox creation time, the kata-agent retrieves this file using the KBS get resource API. Future extensions to the KBS will create automatically the server’s public and private key pair for each sandbox.

### Compile tool

```bash
$ make
```

## Run the tool

### Connect to a real Kata Container

The method used to connect to Kata Containers agent is TCP/IP.

#### Retrieve Sandbox VM IP Address

1. Start a Kata Container and save sandbox ID (pod ID) in `POD_ID`

   ```sh
   $ export POD_ID=<GET_FROM_CRI_RUNTIME>
   ```
2. Retrieve the sandbox VM’s IP address; use for example `crictl` to get the address of the pod. This may require running `crictl` with `–runtime-point` value (`-r`) for customized installation

   ```sh
	$ crictl inspectp --output table $POD_ID | grep Address
    # or
	$ crictl -r "unix://${CONTAINERD_SOCK}" inspectp --output table ${POD_ID} | grep Address
  ```

3.	Run the tool to connect the agent and list running containers.  Note the `kata-agent-tls-ctl` listens on port `50090` for grpc tls requests

   ```sh
   $ export guest_port=50090
   $ export guest_addr=< Set from step two >
   $ export ctl=./target/x86_64-unknown-linux-musl/release/kata-agent-tls-ctl

   ${ctl} -l trace connect --no-auto-values  --key-dir "${key_dir}" --bundle-dir "${bundle_dir}" --server-address "ipaddr://${guest_addr}:${guest_port}" -c "ListContainers"
   ```

## Examples

### QEMU examples

The following examples assume you have:
- Created an OCI bundle, and set the `bundle_dir` environmental variable,
- Set TLS keys environmental variable, `key_dir`,
- Built `kata-agent` with grpc-tls support,
- Created a pod, and retrieved pod address, i.e, guest_address, and set environmental variables accordingly
   ```sh
   export guest_addr=10.89.0.28
   export guest_port=50090
   export ctl=./target/x86_64-unknown-linux-musl/release/kata-agent-tls-ctl
   ```

**NOTE**
 
PullImage, CreateContainer, and StartContainer commands require `pull_image` support in the guest! 

#### Pull an image (TBD)

```bash
image=docker.io/library/alpine:latest

${ctl} -l trace connect --key-dir "${key_dir}" --bundle-dir "${bundle_dir}" --server-address "ipaddr://${guest_addr}:${guest_port}" -c "PullImage cid=${container_id} image=${image}”
```

#### Create a container
Specify a uuid as container ID and use the sample OCI config file in the directory, setting the following environment variables:

```bash
# randomly generate container ID
container_id=9e3d1d4750e4e20945d22c358e13c85c6b88922513bce2832c0cf403f065dc6
OCI_SPEC_CONFIG=${KATA_DIR}/src/tools/agent-tls-ctl/config.json

${ctl} -l trace connect --key-dir "${key_dir}" --bundle-dir "${bundle_dir}" --server-address "ipaddr://${guest_addr}:${guest_port}" -c "CreateContainer cid=${container_id} spec=file:///${OCI_SPEC_CONFIG}"
```

#### Start a container

```bash
${ctl} -l trace connect --no-auto-values --key-dir "${key_dir}" --bundle-dir "${bundle_dir}" --server-address "ipaddr://${guest_addr}:${guest_port}" -c "StartContainer json://{\"container_id\": \"${container_id}\"}"
```
[!NOTE]
Currently, `PullImage`, `CreateContainer`, and `StartContainer` commands are not supported since they require guest image pull support.

#### List running containers

```bash
${ctl} -l trace connect --no-auto-values --key-dir "${key_dir}" --bundle-dir "${bundle_dir}" --server-address "ipaddr://${guest_addr}:${guest_port}" -c "ListContainers"
```

#### Show container statistics

```bash
${ctl} -l trace connect --no-auto-values --key-dir "${key_dir}" --bundle-dir "${bundle_dir}" --server-address "ipaddr://${guest_addr}:${guest_port}" -c "StatsContainer json://{\"container_id\": \"${container_id}\"}"
```

#### Pause a container

```bash
${ctl} -l trace connect --no-auto-values --key-dir "${key_dir}" --bundle-dir "${bundle_dir}" --server-address "ipaddr://${guest_addr}:${guest_port}" -c "PauseContainer json://{\"container_id\": \"${container_id}\"}"
```

#### Resume a paused container

```bash
${ctl} -l trace connect --no-auto-values --key-dir "${key_dir}" --bundle-dir "${bundle_dir}" --server-address "ipaddr://${guest_addr}:${guest_port}" -c "ResumeContainer json://{\"container_id\": \"${container_id}\"}"

```
