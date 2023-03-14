#[macro_use] extern crate nonzero_ext;

use std::num::NonZeroUsize;

fn main() {
    let _a: NonZeroUsize = nonzero!(20usize);
}

