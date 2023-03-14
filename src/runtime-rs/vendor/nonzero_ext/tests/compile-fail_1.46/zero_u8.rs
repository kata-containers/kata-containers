#[macro_use]
extern crate nonzero_ext;

use std::num::NonZeroU8;

#[cfg_attr(rustfmt, rustfmt_skip)]
fn main() {
    let _a: NonZeroU8 = nonzero!(0u8);
}
