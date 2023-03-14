# Rust FUSE library for server, virtio-fs and vhost-user-fs

![Crates.io](https://img.shields.io/crates/l/fuse-backend-rs)
[![Crates.io](https://img.shields.io/crates/v/fuse-backend-rs)](https://crates.io/crates/fuse-backend-rs)

## Design

The fuse-backend-rs crate is an rust library to implement Fuse daemons based on the
[Linux FUSE device (/dev/fuse)](https://www.kernel.org/doc/html/latest/filesystems/fuse.html)
or the [virtiofs](https://stefanha.github.io/virtio/virtio-fs.html#x1-41500011) draft specification.

Linux FUSE is an userspace filesystem framework, and the /dev/fuse device node is the interface for
userspace filesystem daemons to communicate with the in-kernel fuse driver.

And the virito-fs specification extends the FUSE framework into the virtualization world, which uses
the Virtio protocol to transfer FUSE requests and responses between the Fuse client and server.
With virtio-fs, the Fuse client runs within the guest kernel and the Fuse server runs on the host
userspace or hardware.

So the fuse-rs crate is a library to communicate with the Linux FUSE clients, which includes:
- ABI layer, which defines all data structures shared between linux Fuse framework and Fuse daemons.
- API layer, defines the interfaces for Fuse daemons to implement a userspace file system.
- Transport layer, which supports both the Linux Fuse device and virtio-fs protocol.
- VFS/pseudo_fs, an abstraction layer to support multiple file systems by a single virtio-fs device.
- A sample passthrough file system implementation, which passes through files from daemons to clients. 

![arch](docs/images/fuse-backend-architecture.svg)

## Examples

### Filesystem Drivers
- [Virtual File System](https://github.com/cloud-hypervisor/fuse-backend-rs/tree/master/src/api/vfs)
  for an example of union file system.
- [Pseudo File System](https://github.com/cloud-hypervisor/fuse-backend-rs/blob/master/src/api/pseudo_fs.rs)
  for an example of pseudo file system.
- [Passthrough File System](https://github.com/cloud-hypervisor/fuse-backend-rs/tree/master/src/passthrough)
  for an example of passthrough(stacked) file system.
- [Registry Accelerated File System](https://github.com/dragonflyoss/image-service/tree/master/rafs)
  for an example of readonly file system for container images.

### Fuse Servers
- [Dragonfly Image Service fusedev Server](https://github.com/dragonflyoss/image-service/blob/master/src/bin/nydusd/fusedev.rs)
for an example of implementing a fuse server based on the
[fuse-backend-rs](https://crates.io/crates/fuse-backend-rs) crate.
- [Dragonfly Image Service vhost-user-fs Server](https://github.com/dragonflyoss/image-service/blob/master/src/bin/nydusd/virtiofs.rs)
  for an example of implementing vhost-user-fs server based on the
  [fuse-backend-rs](https://crates.io/crates/fuse-backend-rs) crate.
  
### Fuse Server and Main Service Loop
A sample fuse server based on the Linux Fuse device (/dev/fuse):

```rust
use fuse_backend_rs::api::{server::Server, Vfs, VfsOptions};
use fuse_backend_rs::transport::fusedev::{FuseSession, FuseChannel};

struct FuseServer {
    server: Arc<Server<Arc<Vfs>>>,
    ch: FuseChannel,
}

impl FuseServer {
    fn svc_loop(&self) -> Result<()> {
      // Given error EBADF, it means kernel has shut down this session.
      let _ebadf = std::io::Error::from_raw_os_error(libc::EBADF);
      loop {
        if let Some((reader, writer)) = self
                .ch
                .get_request()
                .map_err(|_| std::io::Error::from_raw_os_error(libc::EINVAL))?
        {
          if let Err(e) = self.server.handle_message(reader, writer, None, None) {
            match e {
              fuse_backend_rs::Error::EncodeMessage(_ebadf) => {
                break;
              }
              _ => {
                error!("Handling fuse message failed");
                continue;
              }
            }
          }
        } else {
          info!("fuse server exits");
          break;
        }
      }
      Ok(())
    }
}
```

## License
This project is licensed under
- [Apache License](http://www.apache.org/licenses/LICENSE-2.0), Version 2.0
- [BSD-3-Clause License](https://opensource.org/licenses/BSD-3-Clause)

Source code under [src/tokio-uring] is temporarily copied from [tokio-uring](https://github.com/tokio-rs/tokio-uring)
with modifications, which is licensed under [MIT](https://github.com/tokio-rs/tokio-uring/blob/master/LICENSE).

We will use `crate.io` directly instead of this temporary copy when the pendding PRs is merged.
https://github.com/tokio-rs/tokio-uring/pull/87
https://github.com/tokio-rs/tokio-uring/pull/88
