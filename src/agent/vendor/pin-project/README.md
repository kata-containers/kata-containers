# pin-project

[![crates-badge]][crates-url]
[![docs-badge]][docs-url]
[![license-badge]][license]
[![rustc-badge]][rustc-url]

[crates-badge]: https://img.shields.io/crates/v/pin-project.svg
[crates-url]: https://crates.io/crates/pin-project
[docs-badge]: https://docs.rs/pin-project/badge.svg
[docs-url]: https://docs.rs/pin-project
[license-badge]: https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue.svg
[license]: #license
[rustc-badge]: https://img.shields.io/badge/rustc-1.37+-lightgray.svg
[rustc-url]: https://blog.rust-lang.org/2019/08/15/Rust-1.37.0.html

A crate for safe and ergonomic [pin-projection].

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
pin-project = "1"
```

The current pin-project requires Rust 1.37 or later.

## Examples

[`#[pin_project]`][`pin_project`] attribute creates projection types
covering all the fields of struct or enum.

```rust
use pin_project::pin_project;
use std::pin::Pin;

#[pin_project]
struct Struct<T, U> {
    #[pin]
    pinned: T,
    unpinned: U,
}

impl<T, U> Struct<T, U> {
    fn method(self: Pin<&mut Self>) {
        let this = self.project();
        let _: Pin<&mut T> = this.pinned; // Pinned reference to the field
        let _: &mut U = this.unpinned; // Normal reference to the field
    }
}
```

[*code like this will be generated*][struct-default-expanded]

To use `#[pin_project]` on enums, you need to name the projection type
returned from the method.

```rust
use pin_project::pin_project;
use std::pin::Pin;

#[pin_project(project = EnumProj)]
enum Enum<T, U> {
    Pinned(#[pin] T),
    Unpinned(U),
}

impl<T, U> Enum<T, U> {
    fn method(self: Pin<&mut Self>) {
        match self.project() {
            EnumProj::Pinned(x) => {
                let _: Pin<&mut T> = x;
            }
            EnumProj::Unpinned(y) => {
                let _: &mut U = y;
            }
        }
    }
}
```

[*code like this will be generated*][enum-default-expanded]

See [documentation][docs-url] for more details, and
see [examples] directory for more examples and generated code.

[`pin_project`]: https://docs.rs/pin-project/1/pin_project/attr.pin_project.html
[enum-default-expanded]: examples/enum-default-expanded.rs
[examples]: examples/README.md
[pin-projection]: https://doc.rust-lang.org/nightly/std/pin/index.html#projections-and-structural-pinning
[struct-default-expanded]: examples/struct-default-expanded.rs

## Related Projects

* [pin-project-lite]: A lightweight version of pin-project written with declarative macros.

[pin-project-lite]: https://github.com/taiki-e/pin-project-lite

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
