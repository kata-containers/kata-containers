# nydus-utils

A collection of utilities to support [Nydus Image Service](https://nydus.dev/).
It provides:
- Asynchronous Multi-Producer Multi-Consumer channel
- Blake3 and SHA256 message digest algorithms
- LZ4 and zstd compression algorithms
- `InodeBitmap`: a bitmap implementation to manage inode numbers
- Per-thread async runtime of type tokio current thread Runtime.
- exec() helper
- metric helpers

## Support

**Platforms**:
- x86_64
- aarch64

**Operating Systems**:
- Linux
- MacOS

## License

This code is licensed under [Apache-2.0](LICENSE-APACHE) or [BSD-3-Clause](LICENSE-BSD-3-Clause).
