# nydus-error

The `nydus-error` crate is a collection of helper utilities to handle error codes for [`Nydus Image Service`](https://github.com/dragonflyoss/image-service) project, which provides:
- `macro define_error_macro!()` to optionally augment Posix errno with backtrace information.
- `macro einval!(), enoent!()` etc for commonly used error codes.
- `struct ErrorHolder` to provide a circular buffer to hold the latest error messages.

## Support

**Platforms**:
- x86_64
- aarch64

**Operating Systems**:
- Linux

## Usage

Add `nydus-error` as a dependency in `Cargo.toml`

```toml
[dependencies]
nydus-error = "*"
```

Then add `extern crate nydus-error;` to your crate root if needed.

## Examples

- Return an error with backtracing information:

```rust
fn check_size(size: usize) -> std::io::Result<()> {
    if size > 0x1000 {
        return Err(einval!());
    }

    Ok(())
}
```

- Put an error message into an `ErrorHolder` object.

```rust
fn record_error(size: usize) {
    let mut holder = ErrorHolder::new(10, 80);
    let error_msg = "123456789";
    let r = holder.push(error_msg);

    assert_eq!(r.is_ok(), true);
    let _msg = holder.export().unwrap();
}
```

## License

This code is licensed under [Apache-2.0](LICENSE-APACHE) or [BSD-3-Clause](LICENSE-BSD-3-Clause).
