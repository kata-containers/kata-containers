# rusticata-macros

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE-MIT)
[![Apache License 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](./LICENSE-APACHE)
[![Build Status](https://travis-ci.org/rusticata/rusticata-macros.svg?branch=master)](https://travis-ci.org/rusticata/rusticata-macros)
[![Github CI](https://github.com/rusticata/rusticata-macros/workflows/Continuous%20integration/badge.svg)](https://github.com/rusticata/rusticata-macros/actions)
[![Crates.io Version](https://img.shields.io/crates/v/rusticata-macros.svg)](https://crates.io/crates/rusticata-macros)

<!-- cargo-sync-readme start -->

# Rusticata-macros

Helper macros for the [rusticata](https://github.com/rusticata) project.

This crate contains some additions to [nom](https://github.com/Geal/nom).

For example, the [`combinator::cond_else`] function allows to apply the first parser if the
condition is true, and the second if the condition is false:

```rust
use rusticata_macros::combinator::cond_else;
let r: IResult<_, _, ()> = cond_else(
        || s.len() > 1,
        be_u16,
        map(be_u8, u16::from)
    )(s);
```

See the documentation for more details and examples.

<!-- cargo-sync-readme end -->

## Nom versions

Different versions of this crate are available, depending on nom version.

- `rusticata-macros` 4.x depends on nom 7
- `rusticata-macros` 3.x depends on nom 6
- `rusticata-macros` 2.x depends on nom 5

## Documentation

Crate is documented, do running `cargo doc` will crate the offline documentation.

Reference documentation can be found [here](https://docs.rs/rusticata-macros/)

## Changes

### 4.1.0

- Remove macro `slice_fixed`

### 4.0.0

- Upgrade to nom 7
  - Remove macros

### 3.0.1

- Add `be_var_u64` and `le_var_u64`

### 3.0.0

- Upgrade to nom 6

### 2.1.0

- Add common trait `Serialize` for structures serialization

### 2.0.4

- Add function version of most combinators

### 2.0.3

- Add macros `q` (quote) and `align32`

### 2.0.2

- Add `upgrade_error` and `upgrade_error_to`

### 2.0.1

- Add macro `custom_check`
- Add macro `flat_take`

### 2.0.0

- Upgrade to nom 5
- Debug types: use newtypes

### 1.1.0

- Add macro `newtype_enum`

### 1.0.0

- Upgrade to nom 4.0
  - Warning: this is a breaking change!
- Mark `parse_uint24` as deprecated

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

