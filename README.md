<img src="https://www.openstack.org/assets/kata/kata-vertical-on-white.png" width="150">

# Kata Containers

* [Raising issues](#raising-issues)
* [Kata Containers repositories](#kata-containers-repositories)
    * [Code Repositories](#code-repositories)
        * [Kata Containers-developed components](#kata-containers-developed-components)
            * [Agent](#agent)
            * [KSM throttler](#ksm-throttler)
            * [Proxy](#proxy)
            * [Runtime](#runtime)
            * [Shim](#shim)
        * [Additional](#additional)
            * [Hypervisor](#hypervisor)
            * [Kernel](#kernel)
    * [CI](#ci)
    * [Community](#community)
    * [Documentation](#documentation)
    * [Packaging](#packaging)
    * [Test code](#test-code)
    * [Utilities](#utilities)
        * [OS builder](#os-builder)
    * [Web content](#web-content)

---

Welcome to Kata Containers!

The purpose of this repository is to act as a "top level" site for the project. Specifically it is used:

- To provide a list of the various *other* [Kata Containers repositories](#kata-containers-repositories),
  along with a brief explanation of their purpose.

- To provide a general area for [Raising Issues](#raising-issues).

## Raising issues

This repository is used for [raising
issues](https://github.com/kata-containers/kata-containers/issues/new):

- That might affect multiple code repositories.

- Where the raiser is unsure which repositories are affected.

> **Note:**
> 
> - If an issue affects only a single component, it should be raised in that
>   components repository.

## Kata Containers repositories

### CI

The [CI](https://github.com/kata-containers/ci) repository stores the Continuous
Integration (CI) system configuration information.

### Community

The [Community](https://github.com/kata-containers/community) repository is
the first place to go if you want to use or contribute to the project.

### Code Repositories

#### Kata Containers-developed components

##### Agent

The [`kata-agent`](https://github.com/kata-containers/agent) runs inside the
virtual machine and sets up the container environment.

##### KSM throttler

The [`kata-ksm-throttler`](https://github.com/kata-containers/ksm-throttler)
is an optional utility that monitors containers and deduplicates memory to
maximize container density on a host.

##### Proxy

The [`kata-proxy`](https://github.com/kata-containers/proxy) is a process that
runs on the host and co-ordinates access to the agent running inside the
virtual machine.

##### Runtime

The [`kata-runtime`](https://github.com/kata-containers/runtime) is usually
invoked by a container manager and provides high-level verbs to manage
containers.

##### Shim

The [`kata-shim`](https://github.com/kata-containers/shim) is a process that
runs on the host. It acts as though it is the workload (which actually runs
inside the virtual machine). This shim is required to be compliant with the
expectations of the [OCI runtime
specification](https://github.com/opencontainers/runtime-spec).

#### Additional

##### Hypervisor

The [`qemu`](https://github.com/kata-containers/qemu) hypervisor is used to
create virtual machines for hosting the containers.

##### Kernel

The hypervisor uses a [Linux\* kernel](https://github.com/kata-containers/linux) to boot the guest image.

### Documentation

The [documentation](https://github.com/kata-containers/documentation)
repository hosts documentation common to all code components.

### Packaging

We use the [packaging](https://github.com/kata-containers/packaging)
repository to create packages for the [system
components](#kata-containers-developed-components) including
[rootfs](#os-builder) and [kernel](#kernel) images.

### Test code

The [tests](https://github.com/kata-containers/tests) repository hosts all
test code except the unit testing code (which is kept in the same repository
as the component it tests).

### Utilities

#### OS builder

The [osbuilder](https://github.com/kata-containers/osbuilder) tool can create
a rootfs and a "mini O/S" image. This image is used by the hypervisor to setup
the environment before switching to the workload.

### Web content

The
[www.katacontainers.io](https://github.com/kata-containers/www.katacontainers.io)
repository contains all sources for the https://www.katacontainers.io site.

## Credits

Kata Containers uses [packagecloud](https://packagecloud.io) for package
hosting.
