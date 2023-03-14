#[macro_use]
extern crate nonzero_ext;

use std::num::NonZeroU32;

fn main() {
    const _MY_NON_ZERO_U32: NonZeroU32 = nonzero!((42 * 2 - 21) as u32);
}
