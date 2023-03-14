UTF-8 Width
====================

[![CI](https://github.com/magiclen/utf8-width/actions/workflows/ci.yml/badge.svg)](https://github.com/magiclen/utf8-width/actions/workflows/ci.yml)

To determine the width of a UTF-8 character by providing its first byte.

References: https://tools.ietf.org/html/rfc3629

## Examples

```rust
assert_eq!(1, utf8_width::get_width(b'1'));
assert_eq!(3, utf8_width::get_width("中".as_bytes()[0]));
```

## Benchmark

```bash
cargo bench
```

## Crates.io

https://crates.io/crates/utf8-width

## Documentation

https://docs.rs/utf8-width

## License

[MIT](LICENSE)