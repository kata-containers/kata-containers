// See ./pinned_drop-expanded.rs for generated code.

#![allow(dead_code)]

use pin_project::{pin_project, pinned_drop};
use std::pin::Pin;

#[pin_project(PinnedDrop)]
pub struct Struct<'a, T> {
    was_dropped: &'a mut bool,
    #[pin]
    field: T,
}

#[pinned_drop]
impl<T> PinnedDrop for Struct<'_, T> {
    fn drop(self: Pin<&mut Self>) {
        **self.project().was_dropped = true;
    }
}

fn main() {}
