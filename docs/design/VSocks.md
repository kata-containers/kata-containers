# Kata Containers and VSOCKs

- [Introduction](#introduction)
    - [VSOCK communication diagram](#vsock-communication-diagram)
- [System requirements](#system-requirements)
- [Advantages of using VSOCKs](#advantages-of-using-vsocks)
    - [High density](#high-density)
    - [Reliability](#reliability)

## Introduction

There are two different ways processes in the virtual machine can communicate
with processes in the host. The first one is by using serial ports, where the
processes in the virtual machine can read/write data from/to a serial port
device and the processes in the host can read/write data from/to a Unix socket.
Most GNU/Linux distributions have support for serial ports, making it the most
portable solution. However, the serial link limits read/write access to one
process at a time.

A newer, simpler method is [VSOCKs][1], which can accept connections from
multiple clients. The following diagram shows how it's implemented in Kata Containers.

### VSOCK communication diagram

```
.----------------------.
| .------------------. |
| | .-----.  .-----. | |
| | |cont1|  |cont2| | |
| | `-----'  `-----' | |
| |       |   |      | |
| |    .---------.   | |
| |    |  agent  |   | |
| |    `---------'   | |
| |       |   |      | |
| | POD .-------.    | |
| `-----| vsock |----' |
|       `-------'      |
|         |   |        |
|  .------.   .------. |
|  | shim |   | shim | |
|  `------'   `------' |
| Host                 |
`----------------------'
```

## System requirements

The host Linux kernel version must be greater than or equal to v4.8, and the
`vhost_vsock` module must be loaded or built-in (`CONFIG_VHOST_VSOCK=y`). To
load the module run the following command:

```
$ sudo modprobe -i vhost_vsock
```

The Kata Containers version must be greater than or equal to 1.2.0 and `use_vsock`
must be set to `true` in the runtime [configuration file][1].

### With VMWare guest

To use Kata Containers with VSOCKs in a VMWare guest environment, first stop the `vmware-tools` service and unload the VMWare Linux kernel module.
```
sudo systemctl stop vmware-tools
sudo modprobe -r vmw_vsock_vmci_transport
sudo modprobe -i vhost_vsock
```

## Advantages of using VSOCKs

### High density

Using a proxy for multiplexing the connections between the VM and the host uses
4.5MB per [POD][2]. In a high density deployment this could add up to GBs of
memory that could have been used to host more PODs. When we talk about density
each kilobyte matters and it might be the decisive factor between run another
POD or not. For example if you have 500 PODs running in a server, the same
amount of [`kata-proxy`][3] processes will be running and consuming for around
2250MB of RAM. Before making the decision not to use VSOCKs, you should ask
yourself, how many more containers can run with the memory RAM consumed by the
Kata proxies?

### Reliability

[`kata-proxy`][3] is in charge of multiplexing the connections between virtual
machine and host processes, if it dies all connections get broken. For example
if you have a [POD][2] with 10 containers running, if `kata-proxy` dies it would
be impossible to contact your containers, though they would still be running.
Since communication via VSOCKs is direct, the only way to lose communication
with the containers is if the VM itself or the `containerd-shim-kata-v2` dies, if this happens
the containers are removed automatically.

[1]: https://wiki.qemu.org/Features/VirtioVsock
[2]: ./vcpu-handling.md#virtual-cpus-and-kubernetes-pods
[3]: https://github.com/kata-containers/proxy
