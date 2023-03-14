//! An implementation of the [SHA-3][1] cryptographic hash algorithms.
//!
//! There are 6 standard algorithms specified in the SHA-3 standard:
//!
//! * `SHA3-224`
//! * `SHA3-256`
//! * `SHA3-384`
//! * `SHA3-512`
//! * `SHAKE128`, an extendable output function (XOF)
//! * `SHAKE256`, an extendable output function (XOF)
//! * `Keccak224`, `Keccak256`, `Keccak384`, `Keccak512` (NIST submission
//!    without padding changes)
//!
//! # Examples
//!
//! Output size of SHA3-256 is fixed, so its functionality is usually
//! accessed via the `Digest` trait:
//!
//! ```
//! use hex_literal::hex;
//! use sha3::{Digest, Sha3_256};
//!
//! // create a SHA3-256 object
//! let mut hasher = Sha3_256::new();
//!
//! // write input message
//! hasher.update(b"abc");
//!
//! // read hash digest
//! let result = hasher.finalize();
//!
//! assert_eq!(result[..], hex!("
//!     3a985da74fe225b2045c172d6bd390bd855f086e3e9d525b46bfe24511431532
//! ")[..]);
//! ```
//!
//! SHAKE functions have an extendable output, so finalization method returns
//! XOF reader from which results of arbitrary length can be read. Note that
//! these functions do not implement `Digest`, so lower-level traits have to
//! be imported:
//!
//! ```
//! use sha3::{Shake128, digest::{Update, ExtendableOutput, XofReader}};
//! use hex_literal::hex;
//!
//! let mut hasher = Shake128::default();
//! hasher.update(b"abc");
//! let mut reader = hasher.finalize_xof();
//! let mut res1 = [0u8; 10];
//! reader.read(&mut res1);
//! assert_eq!(res1, hex!("5881092dd818bf5cf8a3"));
//! ```
//!
//! Also see [RustCrypto/hashes][2] readme.
//!
//! [1]: https://en.wikipedia.org/wiki/SHA-3
//! [2]: https://github.com/RustCrypto/hashes

#![no_std]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/RustCrypto/media/6ee8e381/logo.svg",
    html_favicon_url = "https://raw.githubusercontent.com/RustCrypto/media/6ee8e381/logo.svg"
)]
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms)]

pub use digest::{self, Digest};

use core::fmt;
#[cfg(feature = "oid")]
use digest::const_oid::{AssociatedOid, ObjectIdentifier};
use digest::{
    block_buffer::Eager,
    consts::{U104, U136, U144, U168, U200, U28, U32, U48, U64, U72},
    core_api::{
        AlgorithmName, Block, BlockSizeUser, Buffer, BufferKindUser, CoreWrapper,
        ExtendableOutputCore, FixedOutputCore, OutputSizeUser, Reset, UpdateCore, XofReaderCore,
        XofReaderCoreWrapper,
    },
    generic_array::typenum::Unsigned,
    HashMarker, Output,
};

#[macro_use]
mod macros;
mod state;

use crate::state::Sha3State;

// Paddings
const KECCAK: u8 = 0x01;
const SHA3: u8 = 0x06;
const SHAKE: u8 = 0x1f;
const CSHAKE: u8 = 0x4;

impl_sha3!(Keccak224Core, Keccak224, U28, U144, KECCAK, "Keccak-224");
impl_sha3!(Keccak256Core, Keccak256, U32, U136, KECCAK, "Keccak-256");
impl_sha3!(Keccak384Core, Keccak384, U48, U104, KECCAK, "Keccak-384");
impl_sha3!(Keccak512Core, Keccak512, U64, U72, KECCAK, "Keccak-512");

impl_sha3!(
    Keccak256FullCore,
    Keccak256Full,
    U200,
    U136,
    KECCAK,
    "SHA-3 CryptoNight variant",
);

impl_sha3!(
    Sha3_224Core,
    Sha3_224,
    U28,
    U144,
    SHA3,
    "SHA-3-224",
    "2.16.840.1.101.3.4.2.7",
);
impl_sha3!(
    Sha3_256Core,
    Sha3_256,
    U32,
    U136,
    SHA3,
    "SHA-3-256",
    "2.16.840.1.101.3.4.2.8",
);
impl_sha3!(
    Sha3_384Core,
    Sha3_384,
    U48,
    U104,
    SHA3,
    "SHA-3-384",
    "2.16.840.1.101.3.4.2.9",
);
impl_sha3!(
    Sha3_512Core,
    Sha3_512,
    U64,
    U72,
    SHA3,
    "SHA-3-512",
    "2.16.840.1.101.3.4.2.10",
);

impl_shake!(
    Shake128Core,
    Shake128,
    Shake128ReaderCore,
    Shake128Reader,
    U168,
    SHAKE,
    "SHAKE128",
    "2.16.840.1.101.3.4.2.11",
);
impl_shake!(
    Shake256Core,
    Shake256,
    Shake256ReaderCore,
    Shake256Reader,
    U136,
    SHAKE,
    "SHAKE256",
    "2.16.840.1.101.3.4.2.11",
);

impl_cshake!(
    CShake128Core,
    CShake128,
    CShake128ReaderCore,
    CShake128Reader,
    U168,
    SHAKE,
    CSHAKE,
    "CSHAKE128",
);
impl_cshake!(
    CShake256Core,
    CShake256,
    CShake256ReaderCore,
    CShake256Reader,
    U136,
    SHAKE,
    CSHAKE,
    "CSHAKE256",
);

#[inline(always)]
pub(crate) fn left_encode(val: u64, b: &mut [u8; 9]) -> &[u8] {
    b[1..].copy_from_slice(&val.to_be_bytes());
    let i = b[1..8].iter().take_while(|&&a| a == 0).count();
    b[i] = (8 - i) as u8;
    &b[i..]
}
