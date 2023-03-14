//! Generic implementation of [Cipher-based Message Authentication Code (CMAC)][1],
//! otherwise known as OMAC1.
//!
//! # Usage
//! We will use AES-128 block cipher from [aes](https://docs.rs/aes) crate.
//!
//! To get the authentication code:
//!
//! ```rust
//! use aes::Aes128;
//! use cmac::{Cmac, Mac, NewMac};
//!
//! // Create `Mac` trait implementation, namely CMAC-AES128
//! let mut mac = Cmac::<Aes128>::new_varkey(b"very secret key.").unwrap();
//! mac.update(b"input message");
//!
//! // `result` has type `Output` which is a thin wrapper around array of
//! // bytes for providing constant time equality check
//! let result = mac.finalize();
//! // To get underlying array use the `into_bytes` method, but be careful,
//! // since incorrect use of the tag value may permit timing attacks which
//! // defeat the security provided by the `Output` wrapper
//! let tag_bytes = result.into_bytes();
//! ```
//!
//! To verify the message:
//!
//! ```rust
//! # use aes::Aes128;
//! # use cmac::{Cmac, Mac, NewMac};
//! let mut mac = Cmac::<Aes128>::new_varkey(b"very secret key.").unwrap();
//!
//! mac.update(b"input message");
//!
//! # let tag_bytes = mac.clone().finalize().into_bytes();
//! // `verify` will return `Ok(())` if tag is correct, `Err(MacError)` otherwise
//! mac.verify(&tag_bytes).unwrap();
//! ```
//!
//! [1]: https://en.wikipedia.org/wiki/One-key_MAC

#![no_std]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo.svg",
    html_favicon_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo.svg"
)]
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms)]

#[cfg(feature = "std")]
extern crate std;

pub use crypto_mac::{self, FromBlockCipher, Mac, NewMac};

use core::fmt;
use crypto_mac::{
    cipher::BlockCipher,
    generic_array::{typenum::Unsigned, ArrayLength, GenericArray},
    Output,
};
use dbl::Dbl;

type Block<N> = GenericArray<u8, N>;

/// Generic CMAC instance
#[derive(Clone)]
pub struct Cmac<C>
where
    C: BlockCipher + Clone,
    Block<C::BlockSize>: Dbl,
{
    cipher: C,
    key1: Block<C::BlockSize>,
    key2: Block<C::BlockSize>,
    buffer: Block<C::BlockSize>,
    pos: usize,
}

impl<C> FromBlockCipher for Cmac<C>
where
    C: BlockCipher + Clone,
    Block<C::BlockSize>: Dbl,
{
    type Cipher = C;

    fn from_cipher(cipher: C) -> Self {
        let mut subkey = GenericArray::default();
        cipher.encrypt_block(&mut subkey);

        let key1 = subkey.dbl();
        let key2 = key1.clone().dbl();

        Cmac {
            cipher,
            key1,
            key2,
            buffer: Default::default(),
            pos: 0,
        }
    }
}

impl<C> Mac for Cmac<C>
where
    C: BlockCipher + Clone,
    Block<C::BlockSize>: Dbl,
{
    type OutputSize = C::BlockSize;

    #[inline]
    fn update(&mut self, mut data: &[u8]) {
        let n = C::BlockSize::to_usize();

        let rem = n - self.pos;
        if data.len() >= rem {
            let (l, r) = data.split_at(rem);
            data = r;
            for (a, b) in self.buffer[self.pos..].iter_mut().zip(l) {
                *a ^= *b;
            }
            self.pos = n;
        } else {
            for (a, b) in self.buffer[self.pos..].iter_mut().zip(data) {
                *a ^= *b;
            }
            self.pos += data.len();
            return;
        }

        while data.len() >= n {
            self.cipher.encrypt_block(&mut self.buffer);

            let (l, r) = data.split_at(n);
            let block = GenericArray::from_slice(l);
            data = r;

            xor(&mut self.buffer, block);
        }

        if !data.is_empty() {
            self.cipher.encrypt_block(&mut self.buffer);
            for (a, b) in self.buffer.iter_mut().zip(data) {
                *a ^= *b;
            }
            self.pos = data.len();
        }
    }

    #[inline]
    fn finalize(self) -> Output<Self> {
        let n = C::BlockSize::to_usize();
        let mut buf = self.buffer.clone();
        if self.pos == n {
            xor(&mut buf, &self.key1);
        } else {
            xor(&mut buf, &self.key2);
            buf[self.pos] ^= 0x80;
        }
        self.cipher.encrypt_block(&mut buf);
        Output::new(buf)
    }

    fn reset(&mut self) {
        self.buffer = Default::default();
        self.pos = 0;
    }
}

impl<C> fmt::Debug for Cmac<C>
where
    C: BlockCipher + fmt::Debug + Clone,
    Block<C::BlockSize>: Dbl,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "Cmac-{:?}", self.cipher)
    }
}

#[inline(always)]
fn xor<L: ArrayLength<u8>>(buf: &mut Block<L>, data: &Block<L>) {
    for i in 0..L::to_usize() {
        buf[i] ^= data[i];
    }
}

#[cfg(feature = "std")]
impl<C> std::io::Write for Cmac<C>
where
    C: BlockCipher + Clone,
    Block<C::BlockSize>: Dbl,
{
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Mac::update(self, buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
