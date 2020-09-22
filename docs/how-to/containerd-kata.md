# How to use Kata Containers and Containerd

- [Concepts](#concepts)
    - [Kubernetes `RuntimeClass`](#kubernetes-runtimeclass)
    - [Containerd Runtime V2 API: Shim V2 API](#containerd-runtime-v2-api-shim-v2-api)
- [Install](#install)
    - [Install Kata Containers](#install-kata-containers)
    - [Install containerd with CRI plugin](#install-containerd-with-cri-plugin)
    - [Install CNI plugins](#install-cni-plugins)
    - [Install `cri-tools`](#install-cri-tools)
- [Configuration](#configuration)
    - [Configure containerd to use Kata Containers](#configure-containerd-to-use-kata-containers)
        - [Kata Containers as a `RuntimeClass`](#kata-containers-as-a-runtimeclass)
        - [Kata Containers as the runtime for untrusted workload](#kata-containers-as-the-runtime-for-untrusted-workload)
    - [Kata Containers as the default runtime](#kata-containers-as-the-default-runtime)
    - [Configuration for `cri-tools`](#configuration-for-cri-tools)
- [Run](#run)
    - [Launch containers with `ctr` command line](#launch-containers-with-ctr-command-line)
    - [Launch Pods with `crictl` command line](#launch-pods-with-crictl-command-line)

This document covers the installation and configuration of [containerd](https://containerd.io/) 
and [Kata Containers](https://katacontainers.io). The containerd provides not only the `ctr`
command line tool, but also the [CRI](https://kubernetes.io/blog/2016/12/container-runtime-interface-cri-in-kubernetes/) 
interface for [Kubernetes](https://kubernetes.io) and other CRI clients.

This document is primarily written for Kata Containers v1.5.0-rc2 or above, and containerd v1.2.0 or above. 
Previous versions are addressed here, but we suggest users upgrade to the newer versions for better support.

## Concepts

### Kubernetes `RuntimeClass`

[`RuntimeClass`](https://kubernetes.io/docs/concepts/containers/runtime-class/) is a Kubernetes feature first 
introduced in Kubernetes 1.12 as alpha. It is the feature for selecting the container runtime configuration to 
use to run a pod’s containers. This feature is supported in `containerd` since [v1.2.0](https://github.com/containerd/containerd/releases/tag/v1.2.0).

Before the `RuntimeClass` was introduced, Kubernetes was not aware of the difference of runtimes on the node. `kubelet`
creates Pod sandboxes and containers through CRI implementations, and treats all the Pods equally. However, there
are requirements to run trusted Pods (i.e. Kubernetes plugin) in a native container like runc, and to run untrusted 
workloads with isolated sandboxes (i.e. Kata Containers).

As a result, the CRI implementations extended their semantics for the requirements:

- At the beginning, [Frakti](https://github.com/kubernetes/frakti) checks the network configuration of a Pod, and
  treat Pod with `host` network as trusted, while others are treated as untrusted.
- The containerd introduced an annotation for untrusted Pods since [v1.0](https://github.com/containerd/cri/blob/v1.0.0-rc.0/docs/config.md):
  ```yaml
  annotations:
     io.kubernetes.cri.untrusted-workload: "true"
  ```
- Similarly, CRI-O introduced the annotation `io.kubernetes.cri-o.TrustedSandbox` for untrusted Pods.

To eliminate the complexity of user configuration introduced by the non-standardized annotations and provide 
extensibility, `RuntimeClass` was introduced. This gives users the ability to affect the runtime behavior 
through `RuntimeClass` without the knowledge of the CRI daemons. We suggest that users with multiple runtimes 
use `RuntimeClass` instead of the deprecated annotations.

### Containerd Runtime V2 API: Shim V2 API

The [`containerd-shim-kata-v2` (short as `shimv2` in this documentation)](../../src/runtime/containerd-shim-v2) 
implements the [Containerd Runtime V2 (Shim API)](https://github.com/containerd/containerd/tree/master/runtime/v2) for Kata.
With `shimv2`, Kubernetes can launch Pod and OCI-compatible containers with one shim per Pod. Prior to `shimv2`, `2N+1` 
shims (i.e. a `containerd-shim` and a `kata-shim` for each container and the Pod sandbox itself) and no standalone `kata-proxy` 
process were used, even with VSOCK not available.

![Kubernetes integration with shimv2](../design/arch-images/shimv2.svg)

The shim v2 is introduced in containerd [v1.2.0](https://github.com/containerd/containerd/releases/tag/v1.2.0) and Kata `shimv2`
is implemented in Kata Containers v1.5.0.

## Install

### Install Kata Containers

Follow the instructions to [install Kata Containers](../install/README.md).

### Install containerd with CRI plugin

> **Note:** `cri` is a native plugin of containerd 1.1 and above. It is built into containerd and enabled by default.
> You do not need to install `cri` if you have containerd 1.1 or above. Just remove the `cri` plugin from the list of
> `disabled_plugins` in the containerd configuration file (`/etc/containerd/config.toml`).

Follow the instructions from the [CRI installation guide](http://github.com/containerd/cri/blob/master/docs/installation.md).

Then, check if `containerd` is now available:

```bash
$ command -v containerd
```

### Install CNI plugins

> **Note:** You do not need to install CNI plugins if you do not want to use containerd with Kubernetes.
> If you have installed Kubernetes with `kubeadm`, you might have already installed the CNI plugins.

You can manually install CNI plugins as follows:

```bash
$ go get github.com/containernetworking/plugins
$ pushd $GOPATH/src/github.com/containernetworking/plugins
$ ./build_linux.sh
$ sudo mkdir /opt/cni
$ sudo cp -r bin /opt/cni/
$ popd
```

### Install `cri-tools`

> **Note:** `cri-tools` is a set of tools for CRI used for development and testing. Users who only want 
> to use containerd with Kubernetes can skip the `cri-tools`.

You can install the `cri-tools` from source code:

```bash
$ go get github.com/kubernetes-incubator/cri-tools
$ pushd $GOPATH/src/github.com/kubernetes-incubator/cri-tools
$ make
$ sudo -E make install
$ popd
```

## Configuration

### Configure containerd to use Kata Containers

By default, the configuration of containerd is located at `/etc/containerd/config.toml`, and the 
`cri` plugins are placed in the following section:

```toml
[plugins]
  [plugins.cri]
    [plugins.cri.containerd]
      [plugins.cri.containerd.default_runtime]
        #runtime_type = "io.containerd.runtime.v1.linux"

    [plugins.cri.cni]
      # conf_dir is the directory in which the admin places a CNI conf.
      conf_dir = "/etc/cni/net.d"
```

The following sections outline how to add Kata Containers to the configurations.

#### Kata Containers as a `RuntimeClass`

For 
- Kata Containers v1.5.0 or above (including `1.5.0-rc`)
- Containerd v1.2.0 or above
- Kubernetes v1.12.0 or above

The `RuntimeClass` is suggested.

The following configuration includes three runtime classes:
- `plugins.cri.containerd.runtimes.runc`: the runc, and it is the default runtime.
- `plugins.cri.containerd.runtimes.kata`: The function in containerd (reference [the document here](https://github.com/containerd/containerd/tree/master/runtime/v2#binary-naming)) 
  where the dot-connected string `io.containerd.kata.v2` is translated to `containerd-shim-kata-v2` (i.e. the 
  binary name of the Kata implementation of [Containerd Runtime V2 (Shim API)](https://github.com/containerd/containerd/tree/master/runtime/v2)).
- `plugins.cri.containerd.runtimes.katacli`: the `containerd-shim-runc-v1` calls `kata-runtime`, which is the legacy process.

```toml
    [plugins.cri.containerd]
      no_pivot = false
    [plugins.cri.containerd.runtimes]
      [plugins.cri.containerd.runtimes.runc]
         runtime_type = "io.containerd.runc.v1"
         [plugins.cri.containerd.runtimes.runc.options]
           NoPivotRoot = false
           NoNewKeyring = false
           ShimCgroup = ""
           IoUid = 0
           IoGid = 0
           BinaryName = "runc"
           Root = ""
           CriuPath = ""
           SystemdCgroup = false
      [plugins.cri.containerd.runtimes.kata]
         runtime_type = "io.containerd.kata.v2"
      [plugins.cri.containerd.runtimes.katacli]
         runtime_type = "io.containerd.runc.v1"
         [plugins.cri.containerd.runtimes.katacli.options]
           NoPivotRoot = false
           NoNewKeyring = false
           ShimCgroup = ""
           IoUid = 0
           IoGid = 0
           BinaryName = "/usr/bin/kata-runtime"
           Root = ""
           CriuPath = ""
           SystemdCgroup = false
```

From Containerd v1.2.4 and Kata v1.6.0, there is a new runtime option supported, which allows you to specify a specific Kata configuration file as follows:

```toml
      [plugins.cri.containerd.runtimes.kata]
         runtime_type = "io.containerd.kata.v2"
	 privileged_without_host_devices = true
	 [plugins.cri.containerd.runtimes.kata.options]
	   ConfigPath = "/etc/kata-containers/config.toml"
```

`privileged_without_host_devices` tells containerd that a privileged Kata container should not have direct access to all host devices. If unset, containerd will pass all host devices to Kata container, which may cause security issues.

This `ConfigPath` option is optional. If you do not specify it, shimv2 first tries to get the configuration file from the environment variable `KATA_CONF_FILE`. If neither are set, shimv2 will use the default Kata configuration file paths (`/etc/kata-containers/configuration.toml` and `/usr/share/defaults/kata-containers/configuration.toml`).

If you use Containerd older than v1.2.4 or a version of Kata older than v1.6.0  and also want to specify a configuration file, you can use the following workaround, since the shimv2 accepts an environment variable, `KATA_CONF_FILE` for the configuration file path. Then, you can create a
shell script with the following:

```bash
#!/bin/bash
KATA_CONF_FILE=/etc/kata-containers/firecracker.toml containerd-shim-kata-v2 $@
```

Name it as `/usr/local/bin/containerd-shim-katafc-v2` and reference it in the configuration of containerd:

```toml
      [plugins.cri.containerd.runtimes.kata-firecracker]
         runtime_type = "io.containerd.katafc.v2"
```

#### Kata Containers as the runtime for untrusted workload

For cases without `RuntimeClass` support, we can use the legacy annotation method to support using Kata Containers 
for an untrusted workload. With the following configuration, you can run trusted workloads with a runtime such as `runc` 
and then, run an untrusted workload with Kata Containers: 

```toml
    [plugins.cri.containerd]
    # "plugins.cri.containerd.default_runtime" is the runtime to use in containerd.
    [plugins.cri.containerd.default_runtime]
      # runtime_type is the runtime type to use in containerd e.g. io.containerd.runtime.v1.linux
      runtime_type = "io.containerd.runtime.v1.linux"

    # "plugins.cri.containerd.untrusted_workload_runtime" is a runtime to run untrusted workloads on it.
    [plugins.cri.containerd.untrusted_workload_runtime]
      # runtime_type is the runtime type to use in containerd e.g. io.containerd.runtime.v1.linux
      runtime_type = "io.containerd.kata.v2"
```

For the earlier versions of Kata Containers and containerd that do not support Runtime V2 (Shim API), you can use the following alternative configuration:

```toml
    [plugins.cri.containerd]
  
    # "plugins.cri.containerd.default_runtime" is the runtime to use in containerd.
    [plugins.cri.containerd.default_runtime]
      # runtime_type is the runtime type to use in containerd e.g. io.containerd.runtime.v1.linux
      runtime_type = "io.containerd.runtime.v1.linux"

    # "plugins.cri.containerd.untrusted_workload_runtime" is a runtime to run untrusted workloads on it.
    [plugins.cri.containerd.untrusted_workload_runtime]
      # runtime_type is the runtime type to use in containerd e.g. io.containerd.runtime.v1.linux
      runtime_type = "io.containerd.runtime.v1.linux"

      # runtime_engine is the name of the runtime engine used by containerd.
      runtime_engine = "/usr/bin/kata-runtime"
```

You can find more information on the [Containerd config documentation](https://github.com/containerd/cri/blob/master/docs/config.md)


#### Kata Containers as the default runtime

If you want to set Kata Containers as the only runtime in the deployment, you can simply configure as follows:

```toml
    [plugins.cri.containerd]
    [plugins.cri.containerd.default_runtime]
      runtime_type = "io.containerd.kata.v2"
```

Alternatively, for the earlier versions of Kata Containers and containerd that do not support Runtime V2 (Shim API), you can use the following alternative configuration:

```toml
    [plugins.cri.containerd]
    [plugins.cri.containerd.default_runtime]
      runtime_type = "io.containerd.runtime.v1.linux"
      runtime_engine = "/usr/bin/kata-runtime"
```

### Configuration for `cri-tools`

> **Note:** If you skipped the [Install `cri-tools`](#install-cri-tools) section, you can skip this section too.

First, add the CNI configuration in the containerd configuration. 

The following is the configuration if you installed CNI as the *[Install CNI plugins](#install-cni-plugins)* section outlined. 

Put the CNI configuration as `/etc/cni/net.d/10-mynet.conf`:

```json
{
	"cniVersion": "0.2.0",
	"name": "mynet",
	"type": "bridge",
	"bridge": "cni0",
	"isGateway": true,
	"ipMasq": true,
	"ipam": {
		"type": "host-local",
		"subnet": "172.19.0.0/24",
		"routes": [
			{ "dst": "0.0.0.0/0" }
		]
	}
}
```

Next, reference the configuration directory through containerd `config.toml`:

```toml
[plugins.cri.cni]
    # conf_dir is the directory in which the admin places a CNI conf.
    conf_dir = "/etc/cni/net.d"
```

The configuration file of `crictl` command line tool in `cri-tools` locates at `/etc/crictl.yaml`:

```yaml
runtime-endpoint: unix:///var/run/containerd/containerd.sock
image-endpoint: unix:///var/run/containerd/containerd.sock
timeout: 10
debug: true
```

## Run

### Launch containers with `ctr` command line

To run a container with Kata Containers through the containerd command line, you can run the following:

```bash
$ sudo ctr image pull docker.io/library/busybox:latest
$ sudo ctr run --runtime io.containerd.run.kata.v2 -t --rm docker.io/library/busybox:latest hello sh
```

This launches a BusyBox container named `hello`, and it will be removed by `--rm` after it quits.

### Launch Pods with `crictl` command line

With the `crictl` command line of `cri-tools`, you can specify runtime class with `-r` or `--runtime` flag.
 Use the following to launch Pod with `kata` runtime class with the pod in [the example](https://github.com/kubernetes-sigs/cri-tools/tree/master/docs/examples)
of `cri-tools`:

```bash
$ sudo crictl runp -r kata podsandbox-config.yaml
36e23521e8f89fabd9044924c9aeb34890c60e85e1748e8daca7e2e673f8653e
```

You can add container to the launched Pod with the following:

```bash
$ sudo crictl create 36e23521e8f89 container-config.yaml podsandbox-config.yaml
1aab7585530e62c446734f12f6899f095ce53422dafcf5a80055ba11b95f2da7
```

Now, start it with the following:

```bash
$ sudo crictl start 1aab7585530e6
1aab7585530e6
```

In Kubernetes, you need to create a `RuntimeClass` resource and add the `RuntimeClass` field in the Pod Spec 
(see this [document](https://kubernetes.io/docs/concepts/containers/runtime-class/) for more information).

If `RuntimeClass` is not supported, you can use the following annotation in a Kubernetes pod to identify as an untrusted workload:

```yaml
annotations:
   io.kubernetes.cri.untrusted-workload: "true"
```
