// only used on Linux right now, so allow dead code elsewhere
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use alloc::vec;
use alloc::vec::Vec;
use core::cell::UnsafeCell;

/// A simple arena allocator for byte buffers.
pub struct Stash {
    buffers: UnsafeCell<Vec<Vec<u8>>>,
}

impl Stash {
    pub fn new() -> Stash {
        Stash {
            buffers: UnsafeCell::new(Vec::new()),
        }
    }

    /// Allocates a buffer of the specified size and returns a mutable reference
    /// to it.
    pub fn allocate(&self, size: usize) -> &mut [u8] {
        // SAFETY: this is the only function that ever constructs a mutable
        // reference to `self.buffers`.
        let buffers = unsafe { &mut *self.buffers.get() };
        let i = buffers.len();
        buffers.push(vec![0; size]);
        // SAFETY: we never remove elements from `self.buffers`, so a reference
        // to the data inside any buffer will live as long as `self` does.
        &mut buffers[i]
    }
}
