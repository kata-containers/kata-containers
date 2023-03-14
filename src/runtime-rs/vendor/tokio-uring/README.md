# tokio-uring

This crate provides [`io-uring`] for [Tokio] by exposing a new Runtime that is
compatible with Tokio but also can drive [`io-uring`]-backed resources. Any
library that works with [Tokio] also works with `tokio-uring`. The crate
provides new resource types that work with [`io-uring`].

[`io-uring`]: https://unixism.net/loti/
[Tokio]: https://github.com/tokio-rs/tokio
[`fs::File`]: https://docs.rs/tokio-uring/latest/tokio_uring/fs/struct.File.html

[API Docs](https://docs.rs/tokio-uring/latest/tokio_uring) |
[Chat](https://discord.gg/tokio)

# Getting started

Using `tokio-uring` requires starting a [`tokio-uring`] runtime. This
runtime internally manages the main Tokio runtime and a `io-uring` driver.

```rust
use tokio_uring::fs::File;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tokio_uring::start(async {
        // Open a file
        let file = File::open("hello.txt").await?;

        let buf = vec![0; 4096];
        // Read some data, the buffer is passed by ownership and
        // submitted to the kernel. When the operation completes,
        // we get the buffer back.
        let (res, buf) = file.read_at(buf, 0).await;
        let n = res?;

        // Display the contents
        println!("{:?}", &buf[..n]);

        Ok(())
    })
}
```
## Requirements
`tokio-uring` requires a very recent linux kernel. (Not even all kernels with io_uring support will work)
In particular `5.4.0` does not work (This is standard on Ubuntu 20.4). However `5.11.0` (the ubuntu hwe image) does work.
 
## Project status

The `tokio-uring` project is still very young. Currently, we are focusing on
supporting filesystem and network operations. Eventually, we will add safe APIs for all
io-uring compatible operations.

## License

This project is licensed under the [MIT license].

[MIT license]: LICENSE

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in tokio-uring by you, shall be licensed as MIT, without any
additional terms or conditions.
