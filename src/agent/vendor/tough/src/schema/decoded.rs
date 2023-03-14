//! Provides a wrapper and traits for abstracting over decoded keys or different types.

use crate::schema::error::{self, Error};
use crate::schema::spki;
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use snafu::ResultExt;
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::ops::Deref;
use std::str::FromStr;

/// A wrapper around a `Vec<u8>` that contains bytes decoded from an original type `T` (e.g.
/// hex-encoded bytes or a PEM-encoded public key). The original encoded `T` is also stored so it
/// can be used during serialization to verify signatures.
///
/// Other than `Deserialize` and `Serialize`, traits implemented for `Decoded` are equivalent to
/// those for `Vec<u8>`.
#[derive(Debug, Clone)]
pub struct Decoded<T> {
    bytes: Vec<u8>,
    original: String,
    spooky: PhantomData<T>,
}

impl<T> Decoded<T> {
    /// Consume this object and return its decoded bytes.
    ///
    /// You can copy the data with [`to_vec`].
    ///
    /// [`to_vec`]: https://doc.rust-lang.org/std/primitive.slice.html#method.to_vec
    pub fn into_vec(self) -> Vec<u8> {
        self.bytes
    }
}

impl<T: Encode> From<Vec<u8>> for Decoded<T> {
    fn from(b: Vec<u8>) -> Self {
        let original = T::encode(&b);
        Self {
            bytes: b,
            original,
            spooky: PhantomData,
        }
    }
}

// =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=

/// A trait that represents how data can be converted from a string to bytes.
///
/// Generally structs that implement `Decode` will be unit-like structs that just implement the one
/// required method.
pub trait Decode {
    /// Convert a string to bytes.
    ///
    /// The "error" string returned from this method will immediately be wrapped into a
    /// [`serde::de::Error`].
    fn decode(s: &str) -> Result<Vec<u8>, Error>;
}

/// A trait that represents how data can be converted from bytes to a string.
///
/// Generally structs that implement `Decode` will be unit-like structs that just implement the one
/// required method.
pub trait Encode {
    /// Convert bytes to a string. This method cannot fail.
    fn encode(b: &[u8]) -> String;
}

/// [`Decode`]/[`Encode`] implementation for hex-encoded strings.
#[derive(Debug, Clone, Copy)]
pub struct Hex;

impl Decode for Hex {
    fn decode(s: &str) -> Result<Vec<u8>, Error> {
        hex::decode(s).context(error::HexDecodeSnafu)
    }
}

impl Encode for Hex {
    fn encode(b: &[u8]) -> String {
        hex::encode(b)
    }
}

// =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=

/// [`Decode`]/[`Encode`] implementation for PEM-encoded RSA public keys.
#[derive(Debug, Clone, Copy)]
pub struct RsaPem;

impl Decode for RsaPem {
    fn decode(s: &str) -> Result<Vec<u8>, Error> {
        spki::decode(spki::OID_RSA_ENCRYPTION, None, s)
    }
}

impl Encode for RsaPem {
    fn encode(b: &[u8]) -> String {
        spki::encode(spki::OID_RSA_ENCRYPTION, None, b)
    }
}

/// [`Decode`]/[`Encode`] implementation for PEM-encoded ECDSA public keys.
#[derive(Debug, Clone, Copy)]
pub struct EcdsaPem;

impl Decode for EcdsaPem {
    fn decode(s: &str) -> Result<Vec<u8>, Error> {
        spki::decode(
            spki::OID_EC_PUBLIC_KEY,
            Some(spki::OID_EC_PARAM_SECP256R1),
            s,
        )
    }
}

impl Encode for EcdsaPem {
    fn encode(b: &[u8]) -> String {
        spki::encode(
            spki::OID_EC_PUBLIC_KEY,
            Some(spki::OID_EC_PARAM_SECP256R1),
            b,
        )
    }
}

/// [`Decode`]/[`Encode`] implementation for ECDSA public keys.
/// This is a flexible implementation, it will try to decode the key assuming
/// it is PEM encoded, if the decode fails it will then try to decode it
/// assuming it's Hex encoded.
/// The official TUF specification suggests ECDSA keys to be PEM encoded,
/// however the go-tuf implementation encodes them as Hex numbers.
/// This flexible decoder tries to cover both cases in a transparent way.
#[derive(Debug, Clone, Copy)]
pub struct EcdsaFlex {}

impl Decode for EcdsaFlex {
    fn decode(s: &str) -> Result<Vec<u8>, Error> {
        if s.starts_with("-----BEGIN ") {
            EcdsaPem::decode(s)
        } else {
            Hex::decode(s)
        }
    }
}

impl Encode for EcdsaFlex {
    fn encode(b: &[u8]) -> String {
        EcdsaPem::encode(b)
    }
}

// =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=

impl<'de, T: Decode> Deserialize<'de> for Decoded<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let original = String::deserialize(deserializer)?;
        Ok(Self {
            bytes: T::decode(&original).map_err(D::Error::custom)?,
            original,
            spooky: PhantomData,
        })
    }
}

impl<T> Serialize for Decoded<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.original)
    }
}

// =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=

impl<T> AsRef<[u8]> for Decoded<T> {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

impl<T> Deref for Decoded<T> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.bytes
    }
}

impl<T: Decode> FromStr for Decoded<T> {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self {
            bytes: T::decode(s)?,
            original: s.to_owned(),
            spooky: PhantomData,
        })
    }
}

impl<T> PartialEq<[u8]> for Decoded<T> {
    fn eq(&self, other: &[u8]) -> bool {
        self.bytes.eq(&other)
    }
}

impl<T> PartialEq<Vec<u8>> for Decoded<T> {
    fn eq(&self, other: &Vec<u8>) -> bool {
        self.bytes.eq(other)
    }
}

impl<T> PartialEq for Decoded<T> {
    fn eq(&self, other: &Self) -> bool {
        self.bytes.eq(&other.bytes)
    }
}

impl<T> Eq for Decoded<T> {}

impl<T> PartialOrd for Decoded<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.bytes.partial_cmp(&other.bytes)
    }
}

impl<T> Ord for Decoded<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.bytes.cmp(&other.bytes)
    }
}

impl<T> Hash for Decoded<T> {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        self.bytes.hash(hasher);
    }
}
