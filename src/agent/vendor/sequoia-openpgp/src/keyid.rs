use std::fmt;

#[cfg(test)]
use quickcheck::{Arbitrary, Gen};

use crate::Error;
use crate::Fingerprint;
use crate::Result;

/// A short identifier for certificates and keys.
///
/// A `KeyID` identifies a public key.  It is used, for example, to
/// reference the issuing key of a signature in its [`Issuer`]
/// subpacket.
///
/// Currently, Sequoia supports *version 4* fingerprints and Key IDs
/// only.  *Version 3* fingerprints and Key IDs were deprecated by
/// [RFC 4880] in 2007.
///
/// A *v4* `KeyID` is defined as a fragment (the lower 8 bytes) of the
/// key's fingerprint, which in turn is essentially a SHA-1 hash of
/// the public key packet.  As a general rule of thumb, you should
/// prefer the fingerprint as it is possible to create keys with a
/// colliding KeyID using a [birthday attack].
///
/// For more details about how a `KeyID` is generated, see [Section
/// 12.2 of RFC 4880].
///
/// In previous versions of OpenPGP, the Key ID used to be called
/// "long Key ID", as there even was a "short Key ID". At only 4 bytes
/// length, short Key IDs vulnerable to preimage attacks. That is, an
/// attacker can create a key with any given short Key ID in short
/// amount of time.
///
/// See also [`Fingerprint`] and [`KeyHandle`].
///
///   [RFC 4880]: https://tools.ietf.org/html/rfc4880
///   [Section 12.2 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-12.2
///   [birthday attack]: https://nullprogram.com/blog/2019/07/22/
///   [`Issuer`]: crate::packet::signature::subpacket::SubpacketValue::Issuer
///   [`Fingerprint`]: crate::Fingerprint
///   [`KeyHandle`]: crate::KeyHandle
///
/// Note: This enum cannot be exhaustively matched to allow future
/// extensions.
///
/// # Examples
///
/// ```rust
/// # fn main() -> sequoia_openpgp::Result<()> {
/// # use sequoia_openpgp as openpgp;
/// use openpgp::KeyID;
///
/// let id: KeyID = "0123 4567 89AB CDEF".parse()?;
///
/// assert_eq!("0123456789ABCDEF", id.to_hex());
/// # Ok(()) }
/// ```
#[non_exhaustive]
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub enum KeyID {
    /// Lower 8 byte SHA-1 hash.
    V4([u8;8]),
    /// Used for holding invalid keyids encountered during parsing
    /// e.g. wrong number of bytes.
    Invalid(Box<[u8]>),
}
assert_send_and_sync!(KeyID);

impl fmt::Display for KeyID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:X}", self)
    }
}

impl fmt::Debug for KeyID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("KeyID")
            .field(&self.to_string())
            .finish()
    }
}

impl fmt::UpperHex for KeyID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.convert_to_string(false))
    }
}

impl fmt::LowerHex for KeyID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut hex = self.convert_to_string(false);
        hex.make_ascii_lowercase();
        f.write_str(&hex)
    }
}

impl std::str::FromStr for KeyID {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let bytes = crate::fmt::hex::decode_pretty(s)?;

        // A KeyID is exactly 8 bytes long.
        if bytes.len() == 8 {
            Ok(KeyID::from_bytes(&bytes[..]))
        } else {
            // Maybe a fingerprint was given.  Try to parse it and
            // convert it to a KeyID.
            Ok(s.parse::<Fingerprint>()?.into())
        }
    }
}

impl From<KeyID> for Vec<u8> {
    fn from(id: KeyID) -> Self {
        let mut r = Vec::with_capacity(8);
        match id {
            KeyID::V4(ref b) => r.extend_from_slice(b),
            KeyID::Invalid(ref b) => r.extend_from_slice(b),
        }
        r
    }
}

impl From<u64> for KeyID {
    fn from(id: u64) -> Self {
        Self::new(id)
    }
}

impl From<[u8; 8]> for KeyID {
    fn from(id: [u8; 8]) -> Self {
        KeyID::from_bytes(&id[..])
    }
}

impl From<&Fingerprint> for KeyID {
    fn from(fp: &Fingerprint) -> Self {
        match fp {
            Fingerprint::V4(fp) =>
                KeyID::from_bytes(&fp[fp.len() - 8..]),
            Fingerprint::V5(fp) =>
                KeyID::Invalid(fp.iter().cloned().collect()),
            Fingerprint::Invalid(fp) => {
                KeyID::Invalid(fp.clone())
            }
        }
    }
}

impl From<Fingerprint> for KeyID {
    fn from(fp: Fingerprint) -> Self {
        match fp {
            Fingerprint::V4(fp) =>
                KeyID::from_bytes(&fp[fp.len() - 8..]),
            Fingerprint::V5(fp) =>
                KeyID::Invalid(fp.into()),
            Fingerprint::Invalid(fp) => {
                KeyID::Invalid(fp)
            }
        }
    }
}

impl KeyID {
    /// Converts a `u64` to a `KeyID`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # extern crate sequoia_openpgp as openpgp;
    /// use openpgp::KeyID;
    ///
    /// let keyid = KeyID::new(0x0123456789ABCDEF);
    /// ```
    pub fn new(data: u64) -> KeyID {
        let bytes = data.to_be_bytes();
        Self::from_bytes(&bytes[..])
    }

    /// Converts the `KeyID` to a `u64` if possible.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// # extern crate sequoia_openpgp as openpgp;
    /// use openpgp::KeyID;
    ///
    /// let keyid = KeyID::new(0x0123456789ABCDEF);
    ///
    /// assert_eq!(keyid.as_u64()?, 0x0123456789ABCDEF);
    /// # Ok(()) }
    /// ```
    pub fn as_u64(&self) -> Result<u64> {
        match &self {
            KeyID::V4(ref b) =>
                Ok(u64::from_be_bytes(*b)),
            KeyID::Invalid(_) =>
                Err(Error::InvalidArgument("Invalid KeyID".into()).into()),
        }
    }

    /// Creates a `KeyID` from a big endian byte slice.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// # extern crate sequoia_openpgp as openpgp;
    /// use openpgp::KeyID;
    ///
    /// let keyid: KeyID = "0123 4567 89AB CDEF".parse()?;
    ///
    /// let bytes = [0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF];
    /// assert_eq!(KeyID::from_bytes(&bytes), keyid);
    /// # Ok(()) }
    /// ```
    pub fn from_bytes(raw: &[u8]) -> KeyID {
        if raw.len() == 8 {
            let mut keyid : [u8; 8] = Default::default();
            keyid.copy_from_slice(raw);
            KeyID::V4(keyid)
        } else {
            KeyID::Invalid(raw.to_vec().into_boxed_slice())
        }
    }

    /// Returns a reference to the raw `KeyID` as a byte slice in big
    /// endian representation.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use sequoia_openpgp as openpgp;
    /// use openpgp::KeyID;
    ///
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// let keyid: KeyID = "0123 4567 89AB CDEF".parse()?;
    ///
    /// assert_eq!(keyid.as_bytes(), [0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF]);
    /// # Ok(()) }
    /// ```
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            KeyID::V4(ref id) => id,
            KeyID::Invalid(ref id) => id,
        }
    }

    /// Creates a wildcard `KeyID`.
    ///
    /// Refer to [Section 5.1 of RFC 4880] for details.
    ///
    ///   [Section 5.1 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.1
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use sequoia_openpgp as openpgp;
    /// use openpgp::KeyID;
    ///
    /// assert_eq!(KeyID::wildcard(), KeyID::new(0x0000000000000000));
    /// ```
    pub fn wildcard() -> Self {
        Self::from_bytes(&[0u8; 8][..])
    }

    /// Returns `true` if this is the wildcard `KeyID`.
    ///
    /// Refer to [Section 5.1 of RFC 4880] for details.
    ///
    ///   [Section 5.1 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.1
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use sequoia_openpgp as openpgp;
    /// use openpgp::KeyID;
    ///
    /// assert!(KeyID::new(0x0000000000000000).is_wildcard());
    /// ```
    pub fn is_wildcard(&self) -> bool {
        self.as_bytes().iter().all(|b| *b == 0)
    }

    /// Converts this `KeyID` to its canonical hexadecimal
    /// representation.
    ///
    /// This representation is always uppercase and without spaces and
    /// is suitable for stable key identifiers.
    ///
    /// The output of this function is exactly the same as formatting
    /// this object with the `:X` format specifier.
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// # use sequoia_openpgp as openpgp;
    /// use openpgp::KeyID;
    ///
    /// let keyid: KeyID = "fb3751f1587daef1".parse()?;
    ///
    /// assert_eq!("FB3751F1587DAEF1", keyid.to_hex());
    /// assert_eq!(format!("{:X}", keyid), keyid.to_hex());
    /// # Ok(()) }
    /// ```
    pub fn to_hex(&self) -> String {
        format!("{:X}", self)
    }

    /// Converts this `KeyID` to its hexadecimal representation with
    /// spaces.
    ///
    /// This representation is always uppercase and with spaces
    /// grouping the hexadecimal digits into groups of four.  It is
    /// suitable for manual comparison of Key IDs.
    ///
    /// Note: The spaces will hinder other kind of use cases.  For
    /// example, it is harder to select the whole Key ID for copying,
    /// and it has to be quoted when used as a command line argument.
    /// Only use this form for displaying a Key ID with the intent of
    /// manual comparisons.
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// # use sequoia_openpgp as openpgp;
    /// let keyid: openpgp::KeyID = "fb3751f1587daef1".parse()?;
    ///
    /// assert_eq!("FB37 51F1 587D AEF1", keyid.to_spaced_hex());
    /// # Ok(()) }
    /// ```
    pub fn to_spaced_hex(&self) -> String {
        self.convert_to_string(true)
    }

    /// Parses the hexadecimal representation of an OpenPGP `KeyID`.
    ///
    /// This function is the reverse of `to_hex`. It also accepts
    /// other variants of the `keyID` notation including lower-case
    /// letters, spaces and optional leading `0x`.
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// # use sequoia_openpgp as openpgp;
    /// use openpgp::KeyID;
    ///
    /// let keyid = KeyID::from_hex("0xfb3751f1587daef1")?;
    ///
    /// assert_eq!("FB3751F1587DAEF1", keyid.to_hex());
    /// # Ok(()) }
    /// ```
    pub fn from_hex(s: &str) -> std::result::Result<Self, anyhow::Error> {
        std::str::FromStr::from_str(s)
    }

    /// Common code for the above functions.
    fn convert_to_string(&self, pretty: bool) -> String {
        let raw = match self {
            KeyID::V4(ref fp) => &fp[..],
            KeyID::Invalid(ref fp) => &fp[..],
        };

        // We currently only handle V4 Key IDs, which look like:
        //
        //   AACB 3243 6300 52D9
        //
        // Since we have no idea how to format an invalid Key ID, just
        // format it like a V4 fingerprint and hope for the best.

        let mut output = Vec::with_capacity(
            // Each byte results in to hex characters.
            raw.len() * 2
            + if pretty {
                // Every 2 bytes of output, we insert a space.
                raw.len() / 2
            } else { 0 });

        for (i, b) in raw.iter().enumerate() {
            if pretty && i > 0 && i % 2 == 0 {
                output.push(b' ');
            }

            let top = b >> 4;
            let bottom = b & 0xFu8;

            if top < 10u8 {
                output.push(b'0' + top)
            } else {
                output.push(b'A' + (top - 10u8))
            }

            if bottom < 10u8 {
                output.push(b'0' + bottom)
            } else {
                output.push(b'A' + (bottom - 10u8))
            }
        }

        // We know the content is valid UTF-8.
        String::from_utf8(output).unwrap()
    }
}

#[cfg(test)]
impl Arbitrary for KeyID {
    fn arbitrary(g: &mut Gen) -> Self {
        KeyID::new(u64::arbitrary(g))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    quickcheck! {
        fn u64_roundtrip(id: u64) -> bool {
            KeyID::new(id).as_u64().unwrap() == id
        }
    }

    #[test]
    fn from_hex() {
        "FB3751F1587DAEF1".parse::<KeyID>().unwrap();
        "39D100AB67D5BD8C04010205FB3751F1587DAEF1".parse::<KeyID>()
            .unwrap();
        "0xFB3751F1587DAEF1".parse::<KeyID>().unwrap();
        "0x39D100AB67D5BD8C04010205FB3751F1587DAEF1".parse::<KeyID>()
            .unwrap();
        "FB37 51F1 587D AEF1".parse::<KeyID>().unwrap();
        "39D1 00AB 67D5 BD8C 0401  0205 FB37 51F1 587D AEF1".parse::<KeyID>()
            .unwrap();
        "GB3751F1587DAEF1".parse::<KeyID>().unwrap_err();
        "EFB3751F1587DAEF1".parse::<KeyID>().unwrap_err();
        "%FB3751F1587DAEF1".parse::<KeyID>().unwrap_err();
        assert_match!(KeyID::Invalid(_) = "587DAEF1".parse().unwrap());
        assert_match!(KeyID::Invalid(_) = "0x587DAEF1".parse().unwrap());
    }

    #[test]
    fn hex_formatting() {
        let keyid = "FB3751F1587DAEF1".parse::<KeyID>().unwrap();
        assert_eq!(format!("{:X}", keyid), "FB3751F1587DAEF1");
        assert_eq!(format!("{:x}", keyid), "fb3751f1587daef1");
    }
}
