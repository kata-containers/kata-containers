#[macro_use]
extern crate nonzero_ext;

use std::num::NonZeroIsize;

fn main() {
    let _a: NonZeroIsize = nonzero!(20isize);
}
