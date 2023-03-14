use nonzero_ext::nonzero;
use nonzero_ext::{literals::NonZeroLiteral, NonZeroAble};
use std::num::NonZeroUsize;

struct BadInt(usize);
impl BadInt {
    const fn count_ones(&self) -> usize {
        1 // oops, even zero counts!
    }
}
impl NonZeroAble for BadInt {
    type NonZero = NonZeroUsize;

    fn into_nonzero(self) -> Option<Self::NonZero> {
        NonZeroUsize::new(self.0)
    }

    unsafe fn into_nonzero_unchecked(self) -> Self::NonZero {
        NonZeroUsize::new_unchecked(self.0)
    }
}

// And we can't impl:
// impl NonZeroLiteral<BadInt> {} // <- also errors.
trait OtherTrait {
    /// # Safety
    /// self must not be NonZeroLiteral(BadInt(0))
    unsafe fn into_nonzero(self) -> NonZeroUsize;
}

impl OtherTrait for NonZeroLiteral<BadInt> {
    unsafe fn into_nonzero(self) -> NonZeroUsize {
        unsafe { self.0.into_nonzero_unchecked() }
    }
}

fn main() {
    nonzero!(BadInt(0));
}
