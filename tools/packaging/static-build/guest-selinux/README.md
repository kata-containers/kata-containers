# Kata guest SELinux policy

This directory builds the `selinux` guest extension: a binary SELinux policy
for the guest, the file contexts used to label the base image, and the context
NVRC mounts every other extension with. The design is described in
[`docs/design/guest-selinux-extension.md`](../../../../docs/design/guest-selinux-extension.md).

## Contents

| File | Purpose |
| --- | --- |
| `kata-guest.cil` | Kata module on top of Fedora's `targeted` policy and `container-selinux` |
| `extension_contexts` | `<extension> <context>` mount contexts NVRC applies (`*` is the fallback) |
| `components.toml` | Extension manifest; the extension launches nothing and only lists paths |
| `build-guest-selinux.sh` | Runs in the builder: installs the module and lays out the extension tree |
| `build.sh` | Host wrapper that runs the builder container |

The extension tree is:

```
etc/kata-extensions/components.toml
etc/selinux/kata/policy               binary policy (format 35)
etc/selinux/kata/file_contexts        targeted + container-selinux + kata-guest
etc/selinux/kata/extension_contexts
etc/selinux/kata/build-info           policy format and package versions
```

## Domains

| Domain | Entered by | Confinement |
| --- | --- | --- |
| `kernel_t` | NVRC as PID 1 (the policy is loaded after it starts) | unconfined |
| `kata_agent_t` | exec of `/usr/bin/kata-agent` (`kata_agent_exec_t`) | `container_runtime_t` minus policy load, `setenforce` and `setbool` |
| `kata_nvidia_t` | NVRC exec'ing anything from the gpu extension | permissive |
| `kata_coco_t` | the agent exec'ing anything from the coco extension | permissive |
| `container_t` | the agent, from the OCI process label | enforced, as on an SELinux host |

`kata_nvidia_t` and `kata_coco_t` are permissive until their rule set has been
harvested from AVC denials on real workloads; switch them to enforcing by
removing the `typepermissive` lines once it has.

## Labels for content without xattrs

None of the guest's filesystems carry `security.selinux` xattrs except the base
image, which `mkfs.erofs --file-contexts` labels at build time:

- **Extensions** get one type per extension through NVRC's `context=` mount
  option. Only `kata_gpu_extension_t` is visible to containers, because CDI
  bind-mounts the NVIDIA userspace into them. The extension types are
  deliberately not `exec_type`, which container-selinux lets every sandbox
  domain run.
- **erofs snapshotter** layers are mounted by the agent, which passes the
  container's `mountLabel` (or `container_file_t:s0`) as `context=` on the
  container overlay.
- **nydus snapshotter** (guest pull) layers are unpacked by CDH under
  `/run/kata-containers/image`. Type transitions label that content
  `container_file_t` when it is created, on tmpfs (`tmpfs_t` parent) and on a
  freshly formatted ext4 secure mount (`unlabeled_t` root).

## Checking a change

```sh
docker build -t kata-guest-selinux tools/packaging/static-build/guest-selinux
docker run --rm -v "$PWD:$PWD" -v /tmp/out:/tmp/out -e DESTDIR=/tmp/out \
    kata-guest-selinux "$PWD/tools/packaging/static-build/guest-selinux/build-guest-selinux.sh"
docker run --rm -v /tmp/out:/tmp/out kata-guest-selinux \
    sesearch /tmp/out/etc/selinux/kata/policy -A -s container_t -t kata_coco_extension_t
```

The build itself fails if the Kata domains or the `kernel_t -> kata_agent_t`
transition are missing from the compiled policy.
