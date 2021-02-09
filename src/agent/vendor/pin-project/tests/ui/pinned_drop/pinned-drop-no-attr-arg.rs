use pin_project::{pin_project, pinned_drop};
use std::pin::Pin;

#[pin_project]
struct S {
    #[pin]
    f: u8,
}

#[pinned_drop]
impl PinnedDrop for S {
    //~^ ERROR E0119
    fn drop(self: Pin<&mut Self>) {}
}

fn main() {}
