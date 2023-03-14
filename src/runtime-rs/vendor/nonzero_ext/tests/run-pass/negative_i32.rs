#[macro_use]
extern crate nonzero_ext;

use std::num::NonZeroI32;

#[cfg_attr(rustfmt, rustfmt_skip)]
fn main() {
    let _a: NonZeroI32 = nonzero!(-2i32);
}
