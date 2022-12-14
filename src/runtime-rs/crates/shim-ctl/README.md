### Purpose
`shim-ctl` is a binary to exercise the shim proper without containerd
dependencies.

The actual Kata shim is hard to execute outside of deployment environments due
to its dependency on containerd's shim v2 protocol.  Among others, the
dependency requires having a socket with a remote end that's capable of driving
the shim using the shim v2 `ttrpc` protocol, and a binary for shim to publish
events to.

Since at least some of the shim v2 protocol dependencies are fairly hard to
mock up, this presents a significant obstacle to development.

`shim-ctl` takes advantage of the fact that due to the shim implementation
architecture, only the outermost couple of shim layers are
containerd-dependent and all of the inner layers that do the actual heavy
lifting don't depend on containerd.  This allows `shim-ctl` to replace the
containerd-dependent layers with something that's easier to use on a
developer's machine.

### Usage

After building the binary as usual with `cargo build` run `shim-ctl` as follows.

Even though `shim-ctl` does away with containerd dependencies it still has
some requirements of its execution environment.  In particular, it needs a
Kata `configuration.toml` file, some Kata distribution files to point a bunch
of `configuration.toml` keys to (like hypervisor keys `path`, `kernel` or
`initrd`) and a container bundle.  These are however much easier to fulfill
than the original containerd dependencies, and doing so is a one-off task -
once done they can be reused for an unlimited number of modify-build-run
development cycles.

`shim-ctl` also needs to be launched from an exported container bundle
directory.  One can be created by running

```
mkdir rootfs
podman export $(podman create busybox) | tar -C ./rootfs -xvf -
runc spec -b .
```

in a suitable directory.

The program can then be launched like this:

```
cd /the/bundle/directory
KATA_CONF_FILE=/path/to/configuration-qemu.toml /path/to/shim-ctl
```

