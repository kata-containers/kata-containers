//! An implementation of the [SHA-1][1] cryptographic hash algorithm.
//!
//! This is a port of Marc Stevens' sha1collisiondetection algorithm to
//! Rust.  The code is translated from C to Rust using c2rust.
//!
//! To improve the translation, the code is transformed first, replacing
//! macros with inline functions.  Running the test suite using `make
//! check` tests the correctness of the transformation.
//!
//! This crate, like the C implementation, is distributed under the MIT
//! Software License.
//!
//! # Installation of the command line utility
//!
//! The command line utility is a drop-in replacement for coreutils'
//! `sha1sum` utility.  It can be installed, for example, using cargo:
//!
//! ```sh
//! $ cargo install sha1collisiondetection
//! [...]
//! $ sha1cdsum --help
//! sha1cdsum 0.2.3
//! Print or check SHA1 (160-bit) checksums with collision detection.
//!
//! USAGE:
//!     sha1cdsum [FLAGS] [files]...
//! [...]
//! $ sha1cdsum test/*1.*
//! 4f3d9be4a472c4dae83c6314aa6c36a064c1fd14 *coll* test/sha-mbles-1.bin
//! 16e96b70000dd1e7c85b8368ee197754400e58ec *coll* test/shattered-1.pdf
//! ```
//!
//! //! # About
//!
//! This library was designed as near drop-in replacements for common
//! SHA-1 libraries.  They will compute the SHA-1 hash of any given
//! file and additionally will detect cryptanalytic collision attacks
//! against SHA-1 present in each file. It is very fast and takes less
//! than twice the amount of time as regular SHA-1.
//!
//! More specifically they will detect any cryptanalytic collision attack
//! against SHA-1 using any of the top 32 SHA-1 disturbance vectors with
//! probability 1:
//!
//! ```text
//!     I(43,0), I(44,0), I(45,0), I(46,0), I(47,0), I(48,0), I(49,0), I(50,0), I(51,0), I(52,0),
//!     I(46,2), I(47,2), I(48,2), I(49,2), I(50,2), I(51,2),
//!     II(45,0), II(46,0), II(47,0), II(48,0), II(49,0), II(50,0), II(51,0), II(52,0), II(53,0), II(54,0), II(55,0), II(56,0),
//!     II(46,2), II(49,2), II(50,2), II(51,2)
//! ```
//!
//! The possibility of false positives can be neglected as the probability
//! is smaller than 2^-90.
//!
//! The library supports both an indicator flag that applications can
//! check and act on, as well as a special _safe-hash_ mode that returns
//! the real SHA-1 hash when no collision was detected and a different
//! _safe_ hash when a collision was detected.  Colliding files will have
//! the same SHA-1 hash, but will have different unpredictable
//! safe-hashes.  This essentially enables protection of applications
//! against SHA-1 collisions with no further changes in the application,
//! e.g., digital signature forgeries based on SHA-1 collisions
//! automatically become invalid.
//!
//! For the theoretical explanation of collision detection see the
//! award-winning paper on _Counter-Cryptanalysis_:
//!
//! Counter-cryptanalysis, Marc Stevens, CRYPTO 2013, Lecture Notes in
//! Computer Science, vol. 8042, Springer, 2013, pp. 129-146,
//! https://marc-stevens.nl/research/papers/C13-S.pdf
//!
//! # Developers
//!
//! The C implementation of the collision detection algorithm is
//! implemented by:
//!
//! - Marc Stevens, CWI Amsterdam (https://marc-stevens.nl)
//! - Dan Shumow, Microsoft Research (https://www.microsoft.com/en-us/research/people/danshu/)
//!
//! The C implementation is maintained
//! [here](https://github.com/cr-marcstevens/sha1collisiondetection).
//!
//! Please report issues with the rust port
//! [here](https://gitlab.com/sequoia-pgp/sha1collisiondetection).
//!
//! # Usage
//!
//! ```rust
//! use hex_literal::hex;
//! use sha1collisiondetection::Sha1CD;
//!
//! // create a Sha1CD object
//! let mut hasher = Sha1CD::default();
//!
//! // process input message
//! hasher.update(b"hello world");
//!
//! // acquire hash digest in the form of GenericArray,
//! // which in this case is equivalent to [u8; 20]
//! let result = hasher.finalize_cd().unwrap();
//! assert_eq!(result[..], hex!("2aae6c35c94fcfb415dbe95f408b9ce91ee846ed"));
//! ```
//!
//! If this crate's "digest-trait" feature is used, `Sha1CD` also
//! implements the `Digest` trait from the `digest` crate.  Also see
//! [RustCrypto/hashes][2] readme.
//!
//! [1]: https://en.wikipedia.org/wiki/SHA-1
//! [2]: https://github.com/RustCrypto/hashes

#![no_std]
#![warn(missing_docs, rust_2018_idioms)]

use core::fmt;

#[cfg(feature = "std")]
extern crate std;

mod sha1;
use crate::sha1 as sys;
mod ubc_check;

pub use generic_array;
use generic_array::{GenericArray, typenum::consts::U20};
/// The digest output.
pub type Output = GenericArray<u8, U20>;

#[cfg(feature = "digest-trait")]
use digest::consts::U64;
#[cfg(feature = "digest-trait")]
pub use digest::{self, Digest};
#[cfg(feature = "digest-trait")]
use digest::{BlockInput, FixedOutputDirty, Reset, Update};

/// Configures the collision-detecting SHA-1 algorithm.
pub struct Builder(sys::SHA1_CTX);

impl Default for Builder {
    fn default() -> Self {
        Self(unsafe {
            let mut ctx = core::mem::MaybeUninit::uninit();
            sys::SHA1DCInit(ctx.as_mut_ptr());
            ctx.assume_init()
        })
    }
}

impl Builder {
    /// Configures collision mitigation.
    ///
    /// Collision attacks are thwarted by hashing a detected
    /// near-collision block 3 times.  Think of it as extending SHA-1
    /// from 80-steps to 240-steps for such blocks: The best collision
    /// attacks against SHA-1 have complexity about 2^60, thus for
    /// 240-steps an immediate lower-bound for the best cryptoanalytic
    /// attacks would be 2^180.  An attacker would be better off using
    /// a generic birthday search of complexity 2^80.
    ///
    /// Enabling safe SHA-1 hashing will result in the correct SHA-1
    /// hash for messages where no collision attack was detected, but
    /// it will result in a different SHA-1 hash for messages where a
    /// collision attack was detected.  This will automatically
    /// invalidate SHA-1 based digital signature forgeries.
    ///
    /// Enabled by default.
    pub fn safe_hash(mut self, v: bool) -> Self {
        unsafe {
            sys::SHA1DCSetSafeHash(&mut self.0, if v { 1 } else { 0 });
        }
        self
    }

    /// Configures use of Unavoidable Bitconditions.
    ///
    /// This provides a significant speed up.
    ///
    /// Enabled by default.
    pub fn use_ubc(mut self, v: bool) -> Self {
        unsafe {
            sys::SHA1DCSetUseUBC(&mut self.0, if v { 1 } else { 0 });
        }
        self
    }

    /// Configures collision detection.
    ///
    /// Enabled by default.
    pub fn detect_collisions(mut self, v: bool) -> Self {
        unsafe {
            sys::SHA1DCSetUseDetectColl(&mut self.0, if v { 1 } else { 0 });
        }
        self
    }

    /// Finalizes the configuration.
    pub fn build(self) -> Sha1CD {
        Sha1CD(self.0)
    }
}

/// Structure representing the state of a SHA-1 computation.
#[derive(Clone)]
pub struct Sha1CD(sys::SHA1_CTX);

impl fmt::Debug for Sha1CD {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Sha1CD { ... }")
    }
}

impl Default for Sha1CD {
    fn default() -> Self {
        Builder::default().build()
    }
}

#[cfg(feature = "digest-trait")]
impl BlockInput for Sha1CD {
    type BlockSize = U64;
}

#[cfg(feature = "digest-trait")]
impl Update for Sha1CD {
    fn update(&mut self, input: impl AsRef<[u8]>) {
        Sha1CD::update(self, input);
    }
}

#[cfg(feature = "digest-trait")]
impl Reset for Sha1CD {
    fn reset(&mut self) {
        Sha1CD::reset(self);
        let safe_hash = self.0.safe_hash;
        let ubc_check = self.0.ubc_check;
        let detect_coll = self.0.detect_coll;
        let reduced_round_coll = self.0.reduced_round_coll;
        let callback = self.0.callback;
        unsafe {
            sys::SHA1DCInit(&mut self.0);
        }
        self.0.safe_hash = safe_hash;
        self.0.ubc_check = ubc_check;
        self.0.detect_coll = detect_coll;
        self.0.reduced_round_coll = reduced_round_coll;
        self.0.callback = callback;
    }
}

#[cfg(feature = "digest-trait")]
impl FixedOutputDirty for Sha1CD {
    type OutputSize = U20;

    fn finalize_into_dirty(&mut self, out: &mut digest::Output<Self>) {
        let _ = self.finalize_into_dirty_cd(out);
    }
}

impl Sha1CD {
    /// Configures the algorithm.
    pub fn configure() -> Builder {
        Builder::default()
    }

    /// Digest input data.
    ///
    /// This method can be called repeatedly, e.g. for processing
    /// streaming messages.
    pub fn update(&mut self, input: impl AsRef<[u8]>) {
        let input = input.as_ref();
        unsafe {
            sys::SHA1DCUpdate(&mut self.0,
                              input.as_ptr() as *const i8,
                              input.len());
        }
    }

    /// Reset hasher instance to its initial state.
    pub fn reset(&mut self) {
        let safe_hash = self.0.safe_hash;
        let ubc_check = self.0.ubc_check;
        let detect_coll = self.0.detect_coll;
        let reduced_round_coll = self.0.reduced_round_coll;
        let callback = self.0.callback;
        unsafe {
            sys::SHA1DCInit(&mut self.0);
        }
        self.0.safe_hash = safe_hash;
        self.0.ubc_check = ubc_check;
        self.0.detect_coll = detect_coll;
        self.0.reduced_round_coll = reduced_round_coll;
        self.0.callback = callback;
    }

    /// Retrieve result and consume hasher instance, reporting
    /// collisions.
    pub fn finalize_cd(mut self)
                       -> Result<Output, Collision> {
        let mut digest = Output::default();
        self.finalize_into_dirty_cd(&mut digest)?;
        Ok(digest)
    }

    /// Retrieve result and reset hasher instance, reporting
    /// collisions.
    ///
    /// This method sometimes can be more efficient compared to hasher
    /// re-creation.
    pub fn finalize_reset_cd(&mut self)
                             -> Result<Output, Collision> {
        let mut digest = Output::default();
        self.finalize_into_dirty_cd(&mut digest)?;
        Sha1CD::reset(self);
        Ok(digest)
    }

    /// Computes the digest and returns if a collision attack was
    /// detected.
    ///
    /// In case of collisions, the digest will still be returned.
    /// Depending on whether or not the collision mitigation is
    /// enabled (see [`Builder::safe_hash`]), either an attacker
    /// controlled digest as produced by the SHA-1 function, or a
    /// digest computed by a modified SHA-1 function mitigating the
    /// attack.
    ///
    /// By default, the mitigations are enabled, hence this is a safer
    /// variant of Sha1, which invalidates all signatures over any
    /// objects hashing to an attacker-controlled digest.
    ///
    /// [`Builder::safe_hash`]: struct.Builder.html#method.safe_hash
    pub fn finalize_into_dirty_cd(&mut self, out: &mut Output)
                                  -> Result<(), Collision> {
        let ret = unsafe {
            sys::SHA1DCFinal(out.as_mut_ptr(), &mut self.0)
        };
        if !ret {
            Ok(())
        } else {
            Err(Collision::new())
        }
    }
}

#[cfg(feature = "std")]
impl std::io::Write for Sha1CD {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Sha1CD::update(self, buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A detected collision.
///
/// This is returned by [`Sha1CD::finalize_cd`] and similar functions
/// when a collision attack has been detected.
///
/// [`Sha1CD::finalize_cd`]: struct.Sha1CD.html#method.finalize_cd
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Collision {
    // XXX: Register a callback and store information about the
    // colliding blocks.
}

impl fmt::Display for Collision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SHA-1 Collision detected")
    }
}

impl Collision {
    fn new() -> Self {
        Collision {
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Collision {}
