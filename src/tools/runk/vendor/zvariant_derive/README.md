# zvariant_derive

[![](https://docs.rs/zvariant_derive/badge.svg)](https://docs.rs/zvariant_derive/) [![](https://img.shields.io/crates/v/zvariant_derive)](https://crates.io/crates/zvariant_derive)

This crate provides derive macros helpers for [`zvariant`]. The `zvariant` crate re-exports these
macros for your convenience so you do not need to use this crate directly.

**Status:** Stable.

## Example code

```rust
use zvariant::{EncodingContext, from_slice, to_bytes, Type};
use serde::{Deserialize, Serialize};
use byteorder::LE;

#[derive(Deserialize, Serialize, Type, PartialEq, Debug)]
struct Struct<'s> {
    field1: u16,
    field2: i64,
    field3: &'s str,
}

assert_eq!(Struct::signature(), "(qxs)");
let s = Struct {
    field1: 42,
    field2: i64::max_value(),
    field3: "hello",
};
let ctxt = EncodingContext::<LE>::new_dbus(0);
let encoded = to_bytes(ctxt, &s).unwrap();
let decoded: Struct = from_slice(&encoded, ctxt).unwrap();
assert_eq!(decoded, s);
```

[`zvariant`]: https://crates.io/crates/zvariant
