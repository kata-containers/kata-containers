# `no-std-compat`

A `#![no_std]` compatibility layer that will make porting your crate
to no_std *easy*.

It supports stable rust by default since no-std-compat version 0.2.0
([See issue #2](https://gitlab.com/jD91mZM2/no-std-compat/issues/2)).

## Why this exists

In Rust, you can disable the standard library (see
[here](https://docs.rust-embedded.org/embedonomicon/smallest-no-std.html)). Doing
this gets rid of the normal `std` standard library and instead adds
`core`, with an option to also add `alloc` for things requiring memory
allocation. Using `core` + `alloc` results in something similar to the
`std`, and many things using `std` can already be "ported" to use
`core` + `alloc`.

But *every single library* written in rust needs to be updated. This
is because the norm is to use `std`. Using core needs someone to break
the norm, often only behind a feature flag. Compare this to Web
Assembly, where almost only a few low-level crates like `rand` needs
to care, because everything is still under `std` even though some
features don't work there.

Many crates migrating to `#![no_std]` today write a small module
called `std` that forwards imports libcore and liballoc
together. These efforts should be **unified**. We're stronger if not
every single one of us needs to hit and figure out how to fix the same
errors.

## Usage

This library is designed to require as few lines of code as possible,
so that these can be copy-pasted to a bunch of different libraries. My
goal is to turn more crates into `#![no_std]` compatible. It also has
in mind to support the std, as well as supporting no std, meaning you
should only need few conditional compilation attributes.

*Examples can be found in the `example-crates/` folder.*

1​. Add this crate to Cargo.toml, and enable any features you want to
   require (see next section).

`Cargo.toml`:

```toml
[dependencies]
no-std-compat = { version = "...", features = [ "alloc" ] }
```

2​. Optionally, add a `std` flag that pulls in the entire standard
   library and bypasses this compatibility crate. This is useful so
   you can use the standard library for debugging and for extra
   functionality for those who support it. The below code *optionally*
   adds the `std` feature as well to `no-std-compat`, which makes it
   just link to the standard library.

`Cargo.toml`:

```toml
[features]
default = [ "std" ] # Default to using the std
std = [ "no-std-compat/std" ]
```

3​. Enable `no_std`, and import this crate renamed to `std`. This ensures all
   old imports still work on `no_std`. Even if you do want to use the std,
   enabling `no_std` is okay - `no-std-compat` will pull in std if you send the
   right feature flags anyway. You could, of course, use any other name than
   "std" here too. But this is what I would recommend.

`src/lib.rs`:

```rust
#![no_std]

extern crate no_std_compat as std;
```

4​. Import the prelude *in all files*. This is because in `no_std`,
   rust removes the `std` import and instead only imports the `core`
   prelude. That is: Currently, it doesn't import the `alloc` prelude
   on its own. This also imports macros and other needed stuff.

`src/**.rs`:

```rust
use std::prelude::v1::*;
```

## Optional features

- `alloc`: This feature pulls in `alloc` and exposes it in all the usual
  locations. I.e `std::collection` gets mapped to `alloc::collections` and all
  the allocation stuff is added to the prelude.
- `std`: This feature pulls in the entire standard library and overrides all
  other features. This effectively bypasses this crate completely. This is here
  to avoid needing feature gates: Just forward your optional `std` feature to
  here, we handle the rest.
- `unstable`: This feature also re-exports all unstable modules, which isn't
  possible to do unless you compile with nightly. Unless you need an unstable
  module, this crate supports stable rust.
- `compat_hash`: This pulls in
  [hashbrown](https://github.com/rust-lang/hashbrown) (which is not
  HashDoS-resistant!! but #![no_std]). The point is so you can keep using the
  standard, safe, HashMap for those who have the standard library, and fall
  back to a less ideal alternative for those who do not. Be advised, however,
  that this used in a public function signature could be confusing and should
  perhaps be avoided. But that is up to you!
- `compat_sync`: This pulls in [spin](https://github.com/mvdnes/spin-rs) and
  provides replacements for several things used in `std::sync`.
- `compat_macros`: This feature adds dummy `println`, `eprintln`, `dbg`,
  etc. implementations that do absolutely nothing. The point is that any debug
  functions or other loggings that are not required for the library to
  function, just stay silent in `no_std`.

## Contributing

### Updating the glue

Did you pull this crate and realize that it's outdated? Lucky for you,
this crate came prepared. The glue can simply be regenerated with a
python script.

Make sure you have the rust source downloaded somewhere. With rustup,
it's a non-issue:

```rust
rustup component add rust-src
```

Now you can run `./generate.py > src/generated.rs`. If it chooses the
wrong rust version or maybe crashes all together, you can manually
specify the source directory with `--src`. It's that easy. You can
also, of course, run `./generate.py --help` if you forgot the argument
name.

### Updating the feature list

If rust complains about a feature being required but not specified, or
maybe about a feature being unused, this is because some imports are
behind feature gates, and feature gates change. More often than not it
is as trivial as adding or removing stuff from the long, long line in
`src/lib.rs` that specifies features. Should only be a problem when
using the `unstable` feature.
