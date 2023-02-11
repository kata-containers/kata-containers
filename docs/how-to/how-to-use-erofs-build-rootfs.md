# Configure Kata Containers to use EROFS build rootfs

## Introduction
For kata containers, rootfs is used in the read-only way. EROFS can noticeably decrease metadata overhead.

`mkfs.erofs` can generate compressed and uncompressed EROFS images.

For uncompressed images, no files are compressed. However, it is optional to inline the data blocks at the end of the file with the metadata.

For compressed images, each file will be compressed using the lz4 or lz4hc algorithm, and it will be confirmed whether it can save space. Use No compression of the file if compression does not save space.

## Performance comparison
|                 | EROFS | EXT4 | XFS |
|-----------------|-------| --- | --- |
| Image Size [MB] | 106(uncompressed) | 256 | 126 |


## Guidance
### Install the `erofs-utils`
#### `apt/dnf` install
On newer `Ubuntu/Debian` systems, it can be installed directly using the `apt` command, and on `Fedora` it can be installed directly using the `dnf` command.

```shell
# Debian/Ubuntu
$ apt install erofs-utils
# Fedora
$ dnf install erofs-utils
```

#### Source install
[https://git.kernel.org/pub/scm/linux/kernel/git/xiang/erofs-utils.git](https://git.kernel.org/pub/scm/linux/kernel/git/xiang/erofs-utils.git)

##### Compile dependencies
If you need to enable the `Lz4` compression feature, `Lz4 1.8.0+` is required, and `Lz4 1.9.3+` is strongly recommended.

##### Compilation process
For some old lz4 versions (lz4-1.8.0~1.8.3), if lz4-static is not installed, the lz4hc algorithm will not be supported. lz4-static can be installed with apt install lz4-static.x86_64. However, these versions have some bugs in compression, and it is not recommended to use these versions directly.
If you use `lz4 1.9.0+`, you can directly use the following command to compile.

```shell
$ ./autogen.sh
$ ./configure
$ make
```

The compiled `mkfs.erofs` program will be saved in the `mkfs` directory. Afterwards, the generated tools can be installed to a system directory using make install (requires root privileges).

### Create a local rootfs
```shell
$ export distro="ubuntu"
$ export FS_TYPE="erofs"
$ export ROOTFS_DIR="realpath kata-containers/tools/osbuilder/rootfs-builder/rootfs"
$ sudo rm -rf "${ROOTFS_DIR}"
$ pushd kata-containers/tools/osbuilder/rootfs-builder
$ script -fec 'sudo -E SECCOMP=no ./rootfs.sh "${distro}"'
$ popd
```

### Add a custom agent to the image - OPTIONAL
> Note:
> - You should only do this step if you are testing with the latest version of the agent.
```shell
$ sudo install -o root -g root -m 0550 -t "${ROOTFS_DIR}/usr/bin" "${ROOTFS_DIR}/../../../../src/agent/target/x86_64-unknown-linux-musl/release/kata-agent"
$ sudo install -o root -g root -m 0440 "${ROOTFS_DIR}/../../../../src/agent/kata-agent.service" "${ROOTFS_DIR}/usr/lib/systemd/system/"
$ sudo install -o root -g root -m 0440 "${ROOTFS_DIR}/../../../../src/agent/kata-containers.target" "${ROOTFS_DIR}/usr/lib/systemd/system/"
```

### Build a root image
```shell
$ pushd  kata-containers/tools/osbuilder/image-builder
$ script -fec 'sudo -E ./image_builder.sh "${ROOTFS_DIR}"'
$ popd
```

### Install the rootfs image
```shell
$ pushd kata-containers/tools/osbuilder/image-builder
$ commit="$(git log --format=%h -1 HEAD)"
$ date="$(date +%Y-%m-%d-%T.%N%z)"
$ rootfs="erofs"
$ image="kata-containers-${rootfs}-${date}-${commit}"
$ sudo install -o root -g root -m 0640 -D kata-containers.img "/usr/share/kata-containers/${image}"
$ (cd /usr/share/kata-containers && sudo ln -sf "$image" kata-containers.img)
$ popd
```

### Use `EROFS` in the runtime
```shell
$ sudo sed -i -e 's/^# *\(rootfs_type\).*=.*$/\1 = erofs/g' /etc/kata-containers/configuration.toml
```
