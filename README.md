<img src="https://www.openstack.org/assets/kata/kata-vertical-on-white.png" width="150">

# Kata Containers

* [Raising issues](#raising-issues)
* [Kata Containers repositories](#kata-containers-repositories)
    * [Code Repositories](#code-repositories)
        * [Kata Containers-developed components](#kata-containers-developed-components)
            * [Agent](#agent)
            * [KSM throttler](#ksm-throttler)
            * [Runtime](#runtime)
            * [Trace forwarder](#trace-forwarder)
        * [Additional](#additional)
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

The [`kata-agent`](src/agent/README.md) runs inside the
virtual machine and sets up the container environment.

##### KSM throttler

The [`kata-ksm-throttler`](https://github.com/kata-containers/ksm-throttler)
is an optional utility that monitors containers and deduplicates memory to
maximize container density on a host.

##### Runtime

The [`kata-runtime`](src/runtime/README.md) is usually
invoked by a container manager and provides high-level verbs to manage
containers.

##### Trace forwarder

The [`kata-trace-forwarder`](src/trace-forwarder) is a component only used
when tracing the [agent](#agent) process.

#### Additional

##### Kernel

The hypervisor uses a [Linux\* kernel](https://github.com/kata-containers/linux) to boot the guest image.

### Documentation

The [docs](docs/README.md) directory holds documentation common to all code components.

### Packaging

We use the [packaging](tools/packaging/README.md) to create packages for the [system
components](#kata-containers-developed-components) including
[rootfs](#os-builder) and [kernel](#kernel) images.

### Test code

The [tests](https://github.com/kata-containers/tests) repository hosts all
test code except the unit testing code (which is kept in the same repository
as the component it tests).

### Utilities

#### OS builder

The [osbuilder](tools/osbuilder/README.md) tool can create
a rootfs and a "mini O/S" image. This image is used by the hypervisor to setup
the environment before switching to the workload.

#### `kata-agent-ctl`

[`kata-agent-ctl`](tools/agent-ctl) is a low-level test tool for
interacting with the agent.

### Web content

The
[www.katacontainers.io](https://github.com/kata-containers/www.katacontainers.io)
repository contains all sources for the https://www.katacontainers.io site.

## Credits

Kata Containers uses [packagecloud](https://packagecloud.io) for package
hosting.
