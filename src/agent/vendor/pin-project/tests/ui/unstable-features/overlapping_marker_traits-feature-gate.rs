// NB: If you change this test, change 'overlapping_marker_traits.rs' at the same time.

use pin_project::pin_project;
use std::marker::PhantomPinned;

#[pin_project] //~ ERROR E0119
struct Struct<T> {
    #[pin]
    f: T,
}

// unsound Unpin impl
impl<T> Unpin for Struct<T> {}

fn is_unpin<T: Unpin>() {}

fn main() {
    is_unpin::<Struct<PhantomPinned>>()
}
