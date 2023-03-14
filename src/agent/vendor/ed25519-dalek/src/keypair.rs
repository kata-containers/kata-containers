// -*- mode: rust; -*-
//
// This file is part of ed25519-dalek.
// Copyright (c) 2017-2019 isis lovecruft
// See LICENSE for licensing information.
//
// Authors:
// - isis agora lovecruft <isis@patternsinthevoid.net>

//! ed25519 keypairs.

#[cfg(feature = "rand")]
use rand::{CryptoRng, RngCore};

#[cfg(feature = "serde")]
use serde::de::Error as SerdeError;
#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};
#[cfg(feature = "serde")]
use serde_bytes::{Bytes as SerdeBytes, ByteBuf as SerdeByteBuf};

pub use sha2::Sha512;

use curve25519_dalek::digest::generic_array::typenum::U64;
pub use curve25519_dalek::digest::Digest;

use ed25519::signature::{Signer, Verifier};

use crate::constants::*;
use crate::errors::*;
use crate::public::*;
use crate::secret::*;

/// An ed25519 keypair.
#[derive(Debug)]
pub struct Keypair {
    /// The secret half of this keypair.
    pub secret: SecretKey,
    /// The public half of this keypair.
    pub public: PublicKey,
}

impl Keypair {
    /// Convert this keypair to bytes.
    ///
    /// # Returns
    ///
    /// An array of bytes, `[u8; KEYPAIR_LENGTH]`.  The first
    /// `SECRET_KEY_LENGTH` of bytes is the `SecretKey`, and the next
    /// `PUBLIC_KEY_LENGTH` bytes is the `PublicKey` (the same as other
    /// libraries, such as [Adam Langley's ed25519 Golang
    /// implementation](https://github.com/agl/ed25519/)).
    pub fn to_bytes(&self) -> [u8; KEYPAIR_LENGTH] {
        let mut bytes: [u8; KEYPAIR_LENGTH] = [0u8; KEYPAIR_LENGTH];

        bytes[..SECRET_KEY_LENGTH].copy_from_slice(self.secret.as_bytes());
        bytes[SECRET_KEY_LENGTH..].copy_from_slice(self.public.as_bytes());
        bytes
    }

    /// Construct a `Keypair` from the bytes of a `PublicKey` and `SecretKey`.
    ///
    /// # Inputs
    ///
    /// * `bytes`: an `&[u8]` representing the scalar for the secret key, and a
    ///   compressed Edwards-Y coordinate of a point on curve25519, both as bytes.
    ///   (As obtained from `Keypair::to_bytes()`.)
    ///
    /// # Warning
    ///
    /// Absolutely no validation is done on the key.  If you give this function
    /// bytes which do not represent a valid point, or which do not represent
    /// corresponding parts of the key, then your `Keypair` will be broken and
    /// it will be your fault.
    ///
    /// # Returns
    ///
    /// A `Result` whose okay value is an EdDSA `Keypair` or whose error value
    /// is an `SignatureError` describing the error that occurred.
    pub fn from_bytes<'a>(bytes: &'a [u8]) -> Result<Keypair, SignatureError> {
        if bytes.len() != KEYPAIR_LENGTH {
            return Err(InternalError::BytesLengthError {
                name: "Keypair",
                length: KEYPAIR_LENGTH,
            }.into());
        }
        let secret = SecretKey::from_bytes(&bytes[..SECRET_KEY_LENGTH])?;
        let public = PublicKey::from_bytes(&bytes[SECRET_KEY_LENGTH..])?;

        Ok(Keypair{ secret: secret, public: public })
    }

    /// Generate an ed25519 keypair.
    ///
    /// # Example
    ///
    /// ```
    /// extern crate rand;
    /// extern crate ed25519_dalek;
    ///
    /// # #[cfg(feature = "std")]
    /// # fn main() {
    ///
    /// use rand::rngs::OsRng;
    /// use ed25519_dalek::Keypair;
    /// use ed25519_dalek::Signature;
    ///
    /// let mut csprng = OsRng{};
    /// let keypair: Keypair = Keypair::generate(&mut csprng);
    ///
    /// # }
    /// #
    /// # #[cfg(not(feature = "std"))]
    /// # fn main() { }
    /// ```
    ///
    /// # Input
    ///
    /// A CSPRNG with a `fill_bytes()` method, e.g. `rand_os::OsRng`.
    ///
    /// The caller must also supply a hash function which implements the
    /// `Digest` and `Default` traits, and which returns 512 bits of output.
    /// The standard hash function used for most ed25519 libraries is SHA-512,
    /// which is available with `use sha2::Sha512` as in the example above.
    /// Other suitable hash functions include Keccak-512 and Blake2b-512.
    #[cfg(feature = "rand")]
    pub fn generate<R>(csprng: &mut R) -> Keypair
    where
        R: CryptoRng + RngCore,
    {
        let sk: SecretKey = SecretKey::generate(csprng);
        let pk: PublicKey = (&sk).into();

        Keypair{ public: pk, secret: sk }
    }

    /// Sign a `prehashed_message` with this `Keypair` using the
    /// Ed25519ph algorithm defined in [RFC8032 §5.1][rfc8032].
    ///
    /// # Inputs
    ///
    /// * `prehashed_message` is an instantiated hash digest with 512-bits of
    ///   output which has had the message to be signed previously fed into its
    ///   state.
    /// * `context` is an optional context string, up to 255 bytes inclusive,
    ///   which may be used to provide additional domain separation.  If not
    ///   set, this will default to an empty string.
    ///
    /// # Returns
    ///
    /// An Ed25519ph [`Signature`] on the `prehashed_message`.
    ///
    /// # Examples
    ///
    /// ```
    /// extern crate ed25519_dalek;
    /// extern crate rand;
    ///
    /// use ed25519_dalek::Digest;
    /// use ed25519_dalek::Keypair;
    /// use ed25519_dalek::Sha512;
    /// use ed25519_dalek::Signature;
    /// use rand::rngs::OsRng;
    ///
    /// # #[cfg(feature = "std")]
    /// # fn main() {
    /// let mut csprng = OsRng{};
    /// let keypair: Keypair = Keypair::generate(&mut csprng);
    /// let message: &[u8] = b"All I want is to pet all of the dogs.";
    ///
    /// // Create a hash digest object which we'll feed the message into:
    /// let mut prehashed: Sha512 = Sha512::new();
    ///
    /// prehashed.update(message);
    /// # }
    /// #
    /// # #[cfg(not(feature = "std"))]
    /// # fn main() { }
    /// ```
    ///
    /// If you want, you can optionally pass a "context".  It is generally a
    /// good idea to choose a context and try to make it unique to your project
    /// and this specific usage of signatures.
    ///
    /// For example, without this, if you were to [convert your OpenPGP key
    /// to a Bitcoin key][terrible_idea] (just as an example, and also Don't
    /// Ever Do That) and someone tricked you into signing an "email" which was
    /// actually a Bitcoin transaction moving all your magic internet money to
    /// their address, it'd be a valid transaction.
    ///
    /// By adding a context, this trick becomes impossible, because the context
    /// is concatenated into the hash, which is then signed.  So, going with the
    /// previous example, if your bitcoin wallet used a context of
    /// "BitcoinWalletAppTxnSigning" and OpenPGP used a context (this is likely
    /// the least of their safety problems) of "GPGsCryptoIsntConstantTimeLol",
    /// then the signatures produced by both could never match the other, even
    /// if they signed the exact same message with the same key.
    ///
    /// Let's add a context for good measure (remember, you'll want to choose
    /// your own!):
    ///
    /// ```
    /// # extern crate ed25519_dalek;
    /// # extern crate rand;
    /// #
    /// # use ed25519_dalek::Digest;
    /// # use ed25519_dalek::Keypair;
    /// # use ed25519_dalek::Signature;
    /// # use ed25519_dalek::SignatureError;
    /// # use ed25519_dalek::Sha512;
    /// # use rand::rngs::OsRng;
    /// #
    /// # fn do_test() -> Result<Signature, SignatureError> {
    /// # let mut csprng = OsRng{};
    /// # let keypair: Keypair = Keypair::generate(&mut csprng);
    /// # let message: &[u8] = b"All I want is to pet all of the dogs.";
    /// # let mut prehashed: Sha512 = Sha512::new();
    /// # prehashed.update(message);
    /// #
    /// let context: &[u8] = b"Ed25519DalekSignPrehashedDoctest";
    ///
    /// let sig: Signature = keypair.sign_prehashed(prehashed, Some(context))?;
    /// #
    /// # Ok(sig)
    /// # }
    /// # #[cfg(feature = "std")]
    /// # fn main() {
    /// #     do_test();
    /// # }
    /// #
    /// # #[cfg(not(feature = "std"))]
    /// # fn main() { }
    /// ```
    ///
    /// [rfc8032]: https://tools.ietf.org/html/rfc8032#section-5.1
    /// [terrible_idea]: https://github.com/isislovecruft/scripts/blob/master/gpgkey2bc.py
    pub fn sign_prehashed<D>(
        &self,
        prehashed_message: D,
        context: Option<&[u8]>,
    ) -> Result<ed25519::Signature, SignatureError>
    where
        D: Digest<OutputSize = U64>,
    {
        let expanded: ExpandedSecretKey = (&self.secret).into(); // xxx thanks i hate this

        expanded.sign_prehashed(prehashed_message, &self.public, context).into()
    }

    /// Verify a signature on a message with this keypair's public key.
    pub fn verify(
        &self,
        message: &[u8],
        signature: &ed25519::Signature
    ) -> Result<(), SignatureError>
    {
        self.public.verify(message, signature)
    }

    /// Verify a `signature` on a `prehashed_message` using the Ed25519ph algorithm.
    ///
    /// # Inputs
    ///
    /// * `prehashed_message` is an instantiated hash digest with 512-bits of
    ///   output which has had the message to be signed previously fed into its
    ///   state.
    /// * `context` is an optional context string, up to 255 bytes inclusive,
    ///   which may be used to provide additional domain separation.  If not
    ///   set, this will default to an empty string.
    /// * `signature` is a purported Ed25519ph [`Signature`] on the `prehashed_message`.
    ///
    /// # Returns
    ///
    /// Returns `true` if the `signature` was a valid signature created by this
    /// `Keypair` on the `prehashed_message`.
    ///
    /// # Examples
    ///
    /// ```
    /// extern crate ed25519_dalek;
    /// extern crate rand;
    ///
    /// use ed25519_dalek::Digest;
    /// use ed25519_dalek::Keypair;
    /// use ed25519_dalek::Signature;
    /// use ed25519_dalek::SignatureError;
    /// use ed25519_dalek::Sha512;
    /// use rand::rngs::OsRng;
    ///
    /// # fn do_test() -> Result<(), SignatureError> {
    /// let mut csprng = OsRng{};
    /// let keypair: Keypair = Keypair::generate(&mut csprng);
    /// let message: &[u8] = b"All I want is to pet all of the dogs.";
    ///
    /// let mut prehashed: Sha512 = Sha512::new();
    /// prehashed.update(message);
    ///
    /// let context: &[u8] = b"Ed25519DalekSignPrehashedDoctest";
    ///
    /// let sig: Signature = keypair.sign_prehashed(prehashed, Some(context))?;
    ///
    /// // The sha2::Sha512 struct doesn't implement Copy, so we'll have to create a new one:
    /// let mut prehashed_again: Sha512 = Sha512::default();
    /// prehashed_again.update(message);
    ///
    /// let verified = keypair.public.verify_prehashed(prehashed_again, Some(context), &sig);
    ///
    /// assert!(verified.is_ok());
    ///
    /// # verified
    /// # }
    /// #
    /// # #[cfg(feature = "std")]
    /// # fn main() {
    /// #     do_test();
    /// # }
    /// #
    /// # #[cfg(not(feature = "std"))]
    /// # fn main() { }
    /// ```
    ///
    /// [rfc8032]: https://tools.ietf.org/html/rfc8032#section-5.1
    pub fn verify_prehashed<D>(
        &self,
        prehashed_message: D,
        context: Option<&[u8]>,
        signature: &ed25519::Signature,
    ) -> Result<(), SignatureError>
    where
        D: Digest<OutputSize = U64>,
    {
        self.public.verify_prehashed(prehashed_message, context, signature)
    }

    /// Strictly verify a signature on a message with this keypair's public key.
    ///
    /// # On The (Multiple) Sources of Malleability in Ed25519 Signatures
    ///
    /// This version of verification is technically non-RFC8032 compliant.  The
    /// following explains why.
    ///
    /// 1. Scalar Malleability
    ///
    /// The authors of the RFC explicitly stated that verification of an ed25519
    /// signature must fail if the scalar `s` is not properly reduced mod \ell:
    ///
    /// > To verify a signature on a message M using public key A, with F
    /// > being 0 for Ed25519ctx, 1 for Ed25519ph, and if Ed25519ctx or
    /// > Ed25519ph is being used, C being the context, first split the
    /// > signature into two 32-octet halves.  Decode the first half as a
    /// > point R, and the second half as an integer S, in the range
    /// > 0 <= s < L.  Decode the public key A as point A'.  If any of the
    /// > decodings fail (including S being out of range), the signature is
    /// > invalid.)
    ///
    /// All `verify_*()` functions within ed25519-dalek perform this check.
    ///
    /// 2. Point malleability
    ///
    /// The authors of the RFC added in a malleability check to step #3 in
    /// §5.1.7, for small torsion components in the `R` value of the signature,
    /// *which is not strictly required*, as they state:
    ///
    /// > Check the group equation \[8\]\[S\]B = \[8\]R + \[8\]\[k\]A'.  It's
    /// > sufficient, but not required, to instead check \[S\]B = R + \[k\]A'.
    ///
    /// # History of Malleability Checks
    ///
    /// As originally defined (cf. the "Malleability" section in the README of
    /// this repo), ed25519 signatures didn't consider *any* form of
    /// malleability to be an issue.  Later the scalar malleability was
    /// considered important.  Still later, particularly with interests in
    /// cryptocurrency design and in unique identities (e.g. for Signal users,
    /// Tor onion services, etc.), the group element malleability became a
    /// concern.
    ///
    /// However, libraries had already been created to conform to the original
    /// definition.  One well-used library in particular even implemented the
    /// group element malleability check, *but only for batch verification*!
    /// Which meant that even using the same library, a single signature could
    /// verify fine individually, but suddenly, when verifying it with a bunch
    /// of other signatures, the whole batch would fail!
    ///
    /// # "Strict" Verification
    ///
    /// This method performs *both* of the above signature malleability checks.
    ///
    /// It must be done as a separate method because one doesn't simply get to
    /// change the definition of a cryptographic primitive ten years
    /// after-the-fact with zero consideration for backwards compatibility in
    /// hardware and protocols which have it already have the older definition
    /// baked in.
    ///
    /// # Return
    ///
    /// Returns `Ok(())` if the signature is valid, and `Err` otherwise.
    #[allow(non_snake_case)]
    pub fn verify_strict(
        &self,
        message: &[u8],
        signature: &ed25519::Signature,
    ) -> Result<(), SignatureError>
    {
        self.public.verify_strict(message, signature)
    }
}

impl Signer<ed25519::Signature> for Keypair {
    /// Sign a message with this keypair's secret key.
    fn try_sign(&self, message: &[u8]) -> Result<ed25519::Signature, SignatureError> {
        let expanded: ExpandedSecretKey = (&self.secret).into();
        Ok(expanded.sign(&message, &self.public).into())
    }
}

impl Verifier<ed25519::Signature> for Keypair {
    /// Verify a signature on a message with this keypair's public key.
    fn verify(&self, message: &[u8], signature: &ed25519::Signature) -> Result<(), SignatureError> {
        self.public.verify(message, signature)
    }
}

#[cfg(feature = "serde")]
impl Serialize for Keypair {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let bytes = &self.to_bytes()[..];
        SerdeBytes::new(bytes).serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'d> Deserialize<'d> for Keypair {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'d>,
    {
        let bytes = <SerdeByteBuf>::deserialize(deserializer)?;
        Keypair::from_bytes(bytes.as_ref()).map_err(SerdeError::custom)
    }
}
