# Guest assets

Kata Containers creates a VM in which to run one or more containers.
It does this by launching a [hypervisor](README.md#hypervisor) to
create the VM. The hypervisor needs two assets for this task: a Linux
kernel and a small root filesystem image to boot the VM.

## Guest kernel

The [guest kernel](../../../tools/packaging/kernel)
is passed to the hypervisor and used to boot the VM.
The default kernel provided in Kata Containers is highly optimized for
kernel boot time and minimal memory footprint, providing only those
services required by a container workload. It is based on the latest
Linux LTS (Long Term Support) [kernel](https://www.kernel.org).

## Guest image

The hypervisor uses an image file which provides a minimal root
filesystem used by the guest kernel to boot the VM and host the Kata
Container. Kata Containers supports both initrd and rootfs based
minimal guest images. The [default packages](../../install/) provide both
an image and an initrd, both of which are created using the
[`osbuilder`](../../../tools/osbuilder) tool.

> **Notes:**
>
> - Although initrd and rootfs based images are supported, not all
>   [hypervisors](README.md#hypervisor) support both types of image.
>
> - The guest image is *unrelated* to the image used in a container
>   workload.
>
>   For example, if a user creates a container that runs a shell in a
>   BusyBox image, they will run that shell in a BusyBox environment.
>   However, the guest image running inside the VM that is used to
>   *host* that BusyBox image could be running Clear Linux, Ubuntu,
>   Fedora or any other distribution potentially.
>
>   The `osbuilder` tool provides
>   [configurations for various common Linux distributions](../../../tools/osbuilder/rootfs-builder)
>   which can be built into either initrd or rootfs guest images.
>
> - If you are using a [packaged version of Kata
>   Containers](../../install), you can see image details by running the
>   [`kata-collect-data.sh`](../../../src/runtime/data/kata-collect-data.sh.in)
>   script as `root` and looking at the "Image details" section of the
>   output.

#### Root filesystem image

The default packaged rootfs image, sometimes referred to as the _mini
O/S_, is a highly optimized container bootstrap system.

If this image type is [configured](README.md#configuration), when the
user runs the [example command](example-command.md):

- The [runtime](README.md#runtime) will launch the configured [hypervisor](README.md#hypervisor).
- The hypervisor will boot the mini-OS image using the [guest kernel](#guest-kernel).
- The kernel will start the init daemon as PID 1 (`systemd`) inside the VM root environment.
- `systemd`, running inside the mini-OS context, will launch the [agent](README.md#agent)
  in the root context of the VM.
- The agent will create a new container environment, setting its root
  filesystem to that requested by the user (Ubuntu in [the example](example-command.md)).
- The agent will then execute the command (`sh(1)` in [the example](example-command.md))
  inside the new container.

The table below summarises the default mini O/S showing the
environments that are created, the services running in those
environments (for all platforms) and the root filesystem used by
each service:

| Process | Environment | systemd service? | rootfs | User accessible | Notes |
|-|-|-|-|-|-|
| systemd | VM root | n/a | [VM guest image](#guest-image)| [debug console][debug-console] | The init daemon, running as PID 1 |
| [Agent](README.md#agent) | VM root | yes | [VM guest image](#guest-image)| [debug console][debug-console] | Runs as a systemd service |
| `chronyd` | VM root | yes | [VM guest image](#guest-image)| [debug console][debug-console] | Used to synchronise the time with the host |
| container workload (`sh(1)` in [the example](example-command.md)) | VM container | no | User specified (Ubuntu in [the example](example-command.md)) | [exec command](README.md#exec-command) | Managed by the agent |

See also the [process overview](README.md#process-overview).

> **Notes:**
>
> - The "User accessible" column shows how an administrator can access
>   the environment.
>
> - The container workload is running inside a full container
>   environment which itself is running within a VM environment.
>
> - See the [configuration files for the `osbuilder` tool](../../../tools/osbuilder/rootfs-builder)
>   for details of the default distribution for platforms other than
>   Intel x86_64.

#### Initrd image

The initrd image is a compressed `cpio(1)` archive, created from a
rootfs which is loaded into memory and used as part of the Linux
startup process. During startup, the kernel unpacks it into a special
instance of a `tmpfs` mount that becomes the initial root filesystem.

If this image type is [configured](README.md#configuration), when the user runs
the [example command](example-command.md):

- The [runtime](README.md#runtime) will launch the configured [hypervisor](README.md#hypervisor).
- The hypervisor will boot the mini-OS image using the [guest kernel](#guest-kernel).
- The kernel will start the init daemon as PID 1 (the
  [agent](README.md#agent))
  inside the VM root environment.
- The [agent](README.md#agent) will create a new container environment, setting its root
  filesystem to that requested by the user (`ubuntu` in
  [the example](example-command.md)).
- The agent will then execute the command (`sh(1)` in [the example](example-command.md))
  inside the new container.

The table below summarises the default mini O/S showing the environments that are created,
the processes running in those environments (for all platforms) and
the root filesystem used by each service:

| Process | Environment | rootfs | User accessible | Notes |
|-|-|-|-|-|
| [Agent](README.md#agent) | VM root | [VM guest image](#guest-image) | [debug console][debug-console] | Runs as the init daemon (PID 1) |
| container workload | VM container | User specified (Ubuntu in this example) | [exec command](README.md#exec-command) | Managed by the agent |

> **Notes:**
>
> - The "User accessible" column shows how an administrator can access
>   the environment.
>
> - It is possible to use a standard init daemon such as systemd with
>   an initrd image if this is desirable.

See also the [process overview](README.md#process-overview).

#### Image summary

| Image type | Default distro | Init daemon | Reason | Notes |
|-|-|-|-|-|
| [image](background.md#root-filesystem-image) | [Ubuntu](https://ubuntu.com) (for x86_64 systems) | systemd | Fully tested in our CI |  systemd offers flexibility |
| [initrd](#initrd-image) | [Alpine Linux](https://alpinelinux.org) | Kata [agent](README.md#agent) (as no systemd support) | Security hardened and tiny C library |

See also:

- The [osbuilder](../../../tools/osbuilder) tool

  This is used to build all default image types.

- The [versions database](../../../versions.yaml)

  The `default-image-name` and `default-initrd-name` options specify
  the default distributions for each image type.

[debug-console]: ../../Developer-Guide.md#connect-to-debug-console
