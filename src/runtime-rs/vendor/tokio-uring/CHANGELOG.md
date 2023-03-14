# 0.3.0 (March 2nd, 2022)
### Added
net: add unix stream & listener ([#74])
net: add tcp and udp support ([#40])

[#66]: https://github.com/tokio-rs/tokio-uring/pull/74
[#66]: https://github.com/tokio-rs/tokio-uring/pull/40

# 0.2.0 (January 9th, 2022)

### Fixed
fs: fix error handling related to changes in rustc ([#69])
op: fix 'already borrowed' panic ([#39])

### Added
fs: add fs::remove_file ([#66])
fs: implement Debug for File ([#65])
fs: add remove_dir and unlink ([#63])
buf: impl IoBuf/IoBufMut for bytes::Bytes/BytesMut ([#43])

[#69]: https://github.com/tokio-rs/tokio-uring/pull/69
[#66]: https://github.com/tokio-rs/tokio-uring/pull/66
[#65]: https://github.com/tokio-rs/tokio-uring/pull/65
[#63]: https://github.com/tokio-rs/tokio-uring/pull/63
[#39]: https://github.com/tokio-rs/tokio-uring/pull/39
[#43]: https://github.com/tokio-rs/tokio-uring/pull/43