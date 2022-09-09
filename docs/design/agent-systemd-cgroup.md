# Systemd Cgroup for Agent

## usage

As we know, we can interact with cgroups in two ways, **cgroupfs** and **systemd**. The former is achieved by reading and writing cgroup tmpfs' files under `/sys/fs/cgroup` while the latter is done by configuring a transient unit by requesting systemd. Here's an interesting fact that no matter which cgroup driver you want to take, the attribute `linux.cgroupsPath` in `bundle/config.json` with ***corresponding*** format should be specified at the same time. Therefore, which cgroup driver the kata agent uses depends on the `linux.cgroupsPath` you provide. 

To be concrete, when you assign something like `/path_a/path_b` to  `linux.cgroupsPath`, the kata agent will use **cgroupfs** to configure. Instead, when you assign `[slice]:[prefix]:[name]` to ` linux.cgroupsPath`, the kata agent will use **systemd**. In particular, when `linux.cgroupsPath` is not specified or specified as an empty string, which is the default case after `runc spec`, kata agent will use **cgroupfs** to configure cgroups for you, which is also the default behavior.

For systemd, we configure it according to the following `linux.cgroupsPath` format standard provided by runc (`[slice]:[prefix]:[name]`).

> Here slice is a systemd slice under which the container is placed. If empty, it defaults to system.slice, except when cgroup v2 is used and rootless container is created, in which case it defaults to user.slice.
>
> Note that slice can contain dashes to denote a sub-slice (e.g. user-1000.slice is a correct notation, meaning a subslice of user.slice), but it must not contain slashes (e.g. user.slice/user-1000.slice is invalid).
>
> A slice of - represents a root slice.
>
> Next, prefix and name are used to compose the unit name, which is `<prefix>-<name>.scope`, unless name has .slice suffix, in which case prefix is ignored and the name is used as is.

If you don't want to assign a specific name to `linux.cgroupsPath` and just want to use systemd cgroup driver, you just need to configure it as "::", which will be expand as `"system.slice:kata_agent:<container-id>"`. 

## supported properties

The kata agent will translate the parameters in the `linux.resources` of `config.json` into systemd unit properties, and send it to systemd for configuration. Since systemd supports limited properties, only the following parameters in `linux.resources` will be applied.

- cpu

  - v1

  | runtime spec resource | systemd property name |
  | --------------------- | --------------------- |
  | cpu.shares            | CPUShares             |

  - v2

  | runtime spec resource  | systemd property name    |
  | ---------------------- | ------------------------ |
  | cpu.shares             | CPUShares                |
  | cpu.period             | CPUQuotaPeriodUSec(v242) |
  | cpu.period & cpu.quota | CPUQuotaPerSecUSec       |

- memory

  - v1

  | runtime spec resource | systemd property name |
  | --------------------- | --------------------- |
  | memory.limit          | MemoryLimit           |

  - v2

  | runtime spec resource      | systemd property name |
  | -------------------------- | --------------------- |
  | memory.low                 | MemoryLow             |
  | memory.max                 | MemoryMax             |
  | memory.swap & memory.limit | MemorySwapMax         |

- pids

  | runtime spec resource | systemd property name |
  | --------------------- | --------------------- |
  | pids.limit            | TasksMax              |

- cpuset

  | runtime spec resource | systemd property name    |
  | --------------------- | ------------------------ |
  | cpuset.cpus           | AllowedCPUs (v244)       |
  | cpuset.mems           | AllowedMemoryNodes(v244) |

## references

- [runc - systemd cgroup driver](https://github.com/opencontainers/runc/blob/main/docs/systemd.md)

- [systemd.resource-control  — Resource control unit settings](https://www.freedesktop.org/software/systemd/man/systemd.resource-control.html)

