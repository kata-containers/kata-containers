//! An implementation of the [RIPEMD-160][1] cryptographic hash.
//!
//! # Usage
//!
//! ```rust
//! use hex_literal::hex;
//! use ripemd160::{Ripemd160, Digest};
//!
//! // create a RIPEMD-160 hasher instance
//! let mut hasher = Ripemd160::new();
//!
//! // process input message
//! hasher.update(b"Hello world!");
//!
//! // acquire hash digest in the form of GenericArray,
//! // which in this case is equivalent to [u8; 20]
//! let result = hasher.finalize();
//! assert_eq!(result[..], hex!("7f772647d88750add82d8e1a7a3e5c0902a346a3"));
//! ```
//!
//! Also see [RustCrypto/hashes][2] readme.
//!
//! [1]: https://en.wikipedia.org/wiki/RIPEMD
//! [2]: https://github.com/RustCrypto/hashes

#![no_std]
#![doc(html_logo_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo_small.png")]
#![deny(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms)]

#[cfg(feature = "std")]
extern crate std;

mod block;

pub use digest::{self, Digest};

use crate::block::{process_msg_block, DIGEST_BUF_LEN, H0};
use block_buffer::BlockBuffer;
use digest::consts::{U20, U64};
use digest::{BlockInput, FixedOutputDirty, Reset, Update};

/// Structure representing the state of a Ripemd160 computation
#[derive(Clone)]
pub struct Ripemd160 {
    h: [u32; DIGEST_BUF_LEN],
    len: u64,
    buffer: BlockBuffer<U64>,
}

impl Default for Ripemd160 {
    fn default() -> Self {
        Ripemd160 {
            h: H0,
            len: 0,
            buffer: Default::default(),
        }
    }
}

impl BlockInput for Ripemd160 {
    type BlockSize = U64;
}

impl Update for Ripemd160 {
    fn update(&mut self, input: impl AsRef<[u8]>) {
        let input = input.as_ref();
        // Assumes that input.len() can be converted to u64 without overflow
        self.len += input.len() as u64;
        let h = &mut self.h;
        self.buffer.input_block(input, |b| process_msg_block(h, b));
    }
}

impl FixedOutputDirty for Ripemd160 {
    type OutputSize = U20;

    fn finalize_into_dirty(&mut self, out: &mut digest::Output<Self>) {
        let h = &mut self.h;
        let l = self.len << 3;
        self.buffer.len64_padding_le(l, |b| process_msg_block(h, b));

        for (chunk, v) in out.chunks_exact_mut(4).zip(self.h.iter()) {
            chunk.copy_from_slice(&v.to_le_bytes());
        }
    }
}

impl Reset for Ripemd160 {
    fn reset(&mut self) {
        self.buffer.reset();
        self.len = 0;
        self.h = H0;
    }
}

opaque_debug::implement!(Ripemd160);
digest::impl_write!(Ripemd160);
