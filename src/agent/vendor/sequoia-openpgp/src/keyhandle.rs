use std::convert::TryFrom;
use std::cmp;
use std::cmp::Ordering;
use std::borrow::Borrow;

use crate::{
    Error,
    Fingerprint,
    KeyID,
    Result,
};

/// Enum representing an identifier for certificates and keys.
///
/// A `KeyHandle` contains either a [`Fingerprint`] or a [`KeyID`].
/// This is needed because signatures can reference their issuer
/// either by `Fingerprint` or by `KeyID`.
///
/// Currently, Sequoia supports *version 4* fingerprints and Key ID
/// only.  *Version 3* fingerprints and Key ID were deprecated by [RFC
/// 4880] in 2007.
///
/// A *v4* fingerprint is, essentially, a 20-byte SHA-1 hash over the
/// key's public key packet.  A *v4* Key ID is defined as the
/// fingerprint's lower 8 bytes.
///
/// For the exact definition, see [Section 12.2 of RFC 4880].
///
/// Both fingerprint and Key ID are used to identify a key, e.g., the
/// issuer of a signature.
///
///   [RFC 4880]: https://tools.ietf.org/html/rfc4880
///   [Section 12.2 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-12.2
///
/// # Examples
///
/// ```rust
/// # fn main() -> sequoia_openpgp::Result<()> {
/// # use sequoia_openpgp as openpgp;
/// use openpgp::KeyHandle;
/// use openpgp::Packet;
/// use openpgp::parse::Parse;
///
/// let p = Packet::from_bytes(
///     "-----BEGIN PGP SIGNATURE-----
/// #
/// #    wsBzBAABCgAdFiEEwD+mQRsDrhJXZGEYciO1ZnjgJSgFAlnclx8ACgkQciO1Znjg
/// #    JShldAf+NBvUTVPnVPhYM4KihWOUlup8lbD6g1IduSM5rpsGvOVb+uKF6ik+GOBB
/// #    RlMT4s183r3teFxiTkDx2pRhUz0MnOMPfbXovjF6Y93fKCOxCQWLBa0ukjNmE+ax
/// #    gu9nZ3XXDGXZW22iGE52uVjPGSfuLfqvdMy5bKHn8xow/kepuGHZwy8yn7uFv7sl
/// #    LnOBUz1FKA7iRl457XKPUhw5K7BnfRW/I2BRlnrwTDkjfXaJZC+bUTIJvm682Bvt
/// #    ZNn8zc0JucyEkuL9WXYNuZg0znDE3T7D/6+tzfEdSf706unsXFXWHf83vL2eHCcw
/// #    qhImm1lmcC+agFtWQ6/qD923LR9xmg==
/// #    =htNu
/// #    -----END PGP SIGNATURE-----" /* docstring trickery ahead:
///      // ...
///      -----END PGP SIGNATURE-----")?;
/// #    */)?;
/// if let Packet::Signature(sig) = p {
///     let issuers = sig.get_issuers();
///     assert_eq!(issuers.len(), 2);
///     assert_eq!(&issuers[0],
///                &KeyHandle::Fingerprint(
///                    "C03F A641 1B03 AE12 5764  6118 7223 B566 78E0 2528"
///                        .parse()?));
///     assert_eq!(&issuers[1],
///                &KeyHandle::KeyID("7223 B566 78E0 2528".parse()?));
/// } else {
///     unreachable!("It's a signature!");
/// }
/// # Ok(()) }
/// ```
#[derive(Debug, Clone)]
pub enum KeyHandle {
    /// A Fingerprint.
    Fingerprint(Fingerprint),
    /// A KeyID.
    KeyID(KeyID),
}
assert_send_and_sync!(KeyHandle);

impl std::fmt::Display for KeyHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            KeyHandle::Fingerprint(v) => v.fmt(f),
            KeyHandle::KeyID(v) => v.fmt(f),
        }
    }
}

impl std::fmt::UpperHex for KeyHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match &self {
            KeyHandle::Fingerprint(ref fpr) => write!(f, "{:X}", fpr),
            KeyHandle::KeyID(ref keyid) => write!(f, "{:X}", keyid),
        }
    }
}

impl std::fmt::LowerHex for KeyHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match &self {
            KeyHandle::Fingerprint(ref fpr) => write!(f, "{:x}", fpr),
            KeyHandle::KeyID(ref keyid) => write!(f, "{:x}", keyid),
        }
    }
}

impl From<KeyID> for KeyHandle {
    fn from(i: KeyID) -> Self {
        KeyHandle::KeyID(i)
    }
}

impl From<&KeyID> for KeyHandle {
    fn from(i: &KeyID) -> Self {
        KeyHandle::KeyID(i.clone())
    }
}

impl From<KeyHandle> for KeyID {
    fn from(i: KeyHandle) -> Self {
        match i {
            KeyHandle::Fingerprint(i) => i.into(),
            KeyHandle::KeyID(i) => i,
        }
    }
}

impl From<&KeyHandle> for KeyID {
    fn from(i: &KeyHandle) -> Self {
        match i {
            KeyHandle::Fingerprint(i) => i.clone().into(),
            KeyHandle::KeyID(i) => i.clone(),
        }
    }
}

impl From<Fingerprint> for KeyHandle {
    fn from(i: Fingerprint) -> Self {
        KeyHandle::Fingerprint(i)
    }
}

impl From<&Fingerprint> for KeyHandle {
    fn from(i: &Fingerprint) -> Self {
        KeyHandle::Fingerprint(i.clone())
    }
}

impl TryFrom<KeyHandle> for Fingerprint {
    type Error = anyhow::Error;
    fn try_from(i: KeyHandle) -> Result<Self> {
        match i {
            KeyHandle::Fingerprint(i) => Ok(i),
            KeyHandle::KeyID(i) => Err(Error::InvalidOperation(
                format!("Cannot convert keyid {} to fingerprint", i)).into()),
        }
    }
}

impl TryFrom<&KeyHandle> for Fingerprint {
    type Error = anyhow::Error;
    fn try_from(i: &KeyHandle) -> Result<Self> {
        match i {
            KeyHandle::Fingerprint(i) => Ok(i.clone()),
            KeyHandle::KeyID(i) => Err(Error::InvalidOperation(
                format!("Cannot convert keyid {} to fingerprint", i)).into()),
        }
    }
}

impl PartialOrd for KeyHandle {
    fn partial_cmp(&self, other: &KeyHandle) -> Option<Ordering> {
        let a = self.as_bytes();
        let b = other.as_bytes();

        let l = cmp::min(a.len(), b.len());

        // Do a little endian comparison so that for v4 keys (where
        // the KeyID is a suffix of the Fingerprint) equivalent KeyIDs
        // and Fingerprints sort next to each other.
        for (a, b) in a[a.len()-l..].iter().zip(b[b.len()-l..].iter()) {
            let cmp = a.cmp(b);
            if cmp != Ordering::Equal {
                return Some(cmp);
            }
        }

        if a.len() == b.len() {
            Some(Ordering::Equal)
        } else {
            // One (a KeyID) is the suffix of the other (a
            // Fingerprint).
            None
        }
    }
}

impl PartialEq for KeyHandle {
    fn eq(&self, other: &Self) -> bool {
        self.partial_cmp(other) == Some(Ordering::Equal)
    }
}

impl std::str::FromStr for KeyHandle {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let bytes = &crate::fmt::hex::decode_pretty(s)?[..];
        match Fingerprint::from_bytes(bytes) {
            fpr @ Fingerprint::Invalid(_) => {
                match KeyID::from_bytes(bytes) {
                    // If it can't be parsed as either a Fingerprint or a
                    // KeyID, return Fingerprint::Invalid.
                    KeyID::Invalid(_) => Ok(fpr.into()),
                    kid => Ok(kid.into()),
                }
            }
            fpr => Ok(fpr.into()),
        }
    }
}

impl KeyHandle {
    /// Returns the raw identifier as a byte slice.
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            KeyHandle::Fingerprint(i) => i.as_bytes(),
            KeyHandle::KeyID(i) => i.as_bytes(),
        }
    }

    /// Returns whether `self` and `other` could be aliases of each
    /// other.
    ///
    /// `KeyHandle`'s `PartialEq` implementation cannot assert that a
    /// `Fingerprint` and a `KeyID` are equal, because distinct
    /// fingerprints may have the same `KeyID`, and `PartialEq` must
    /// be [transitive], i.e.,
    ///
    /// ```text
    /// a == b and b == c implies a == c.
    /// ```
    ///
    /// [transitive]: std::cmp::PartialEq
    ///
    /// That is, if `fpr1` and `fpr2` are distinct fingerprints with the
    /// same key ID then:
    ///
    /// ```text
    /// fpr1 == keyid and fpr2 == keyid, but fpr1 != fpr2.
    /// ```
    ///
    /// In these cases (and only these cases) `KeyHandle`'s
    /// `PartialOrd` implementation returns `None` to correctly
    /// indicate that a comparison is not possible.
    ///
    /// This definition of equality makes searching for a given
    /// `KeyHandle` using `PartialEq` awkward.  This function fills
    /// that gap.  It answers the question: given two `KeyHandles`,
    /// could they be aliases?  That is, it implements the desired,
    /// non-transitive equality relation:
    ///
    /// ```
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// # use sequoia_openpgp as openpgp;
    /// # use openpgp::Fingerprint;
    /// # use openpgp::KeyID;
    /// # use openpgp::KeyHandle;
    /// #
    /// # let fpr1 : KeyHandle
    /// #     = "8F17 7771 18A3 3DDA 9BA4  8E62 AACB 3243 6300 52D9"
    /// #       .parse::<Fingerprint>()?.into();
    /// #
    /// # let fpr2 : KeyHandle
    /// #     = "0123 4567 8901 2345 6789  0123 AACB 3243 6300 52D9"
    /// #       .parse::<Fingerprint>()?.into();
    /// #
    /// # let keyid : KeyHandle = "AACB 3243 6300 52D9".parse::<KeyID>()?
    /// #     .into();
    /// #
    /// // fpr1 and fpr2 are different fingerprints with the same KeyID.
    /// assert!(! fpr1.eq(&fpr2));
    /// assert!(fpr1.aliases(&keyid));
    /// assert!(fpr2.aliases(&keyid));
    /// assert!(! fpr1.aliases(&fpr2));
    /// # Ok(()) }
    /// ```
    pub fn aliases<H>(&self, other: H) -> bool
        where H: Borrow<KeyHandle>
    {
        // This works, because the PartialOrd implementation only
        // returns None if one value is a fingerprint and the other is
        // a key id that matches the fingerprint's key id.
        self.partial_cmp(other.borrow()).unwrap_or(Ordering::Equal)
            == Ordering::Equal
    }

    /// Returns whether the KeyHandle is invalid.
    ///
    /// A KeyHandle is invalid if the `Fingerprint` or `KeyID` that it
    /// contains is invalid.
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::Fingerprint;
    /// use openpgp::KeyID;
    /// use openpgp::KeyHandle;
    ///
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// // A perfectly valid fingerprint:
    /// let kh : KeyHandle = "8F17 7771 18A3 3DDA 9BA4  8E62 AACB 3243 6300 52D9"
    ///     .parse()?;
    /// assert!(! kh.is_invalid());
    ///
    /// // But, V3 fingerprints are invalid.
    /// let kh : KeyHandle = "9E 94 45 13 39 83 5F 70 7B E7 D8 ED C4 BE 5A A6"
    ///     .parse()?;
    /// assert!(kh.is_invalid());
    ///
    /// // A perfectly valid Key ID:
    /// let kh : KeyHandle = "AACB 3243 6300 52D9"
    ///     .parse()?;
    /// assert!(! kh.is_invalid());
    ///
    /// // But, short Key IDs are invalid:
    /// let kh : KeyHandle = "6300 52D9"
    ///     .parse()?;
    /// assert!(kh.is_invalid());
    /// # Ok(()) }
    /// ```
    pub fn is_invalid(&self) -> bool {
        matches!(self,
                 KeyHandle::Fingerprint(Fingerprint::Invalid(_))
                 | KeyHandle::KeyID(KeyID::Invalid(_)))
    }

    /// Converts this `KeyHandle` to its canonical hexadecimal
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
    /// use openpgp::KeyHandle;
    ///
    /// let h: KeyHandle =
    ///     "0123 4567 89AB CDEF 0123 4567 89AB CDEF 0123 4567".parse()?;
    ///
    /// assert_eq!("0123456789ABCDEF0123456789ABCDEF01234567", h.to_hex());
    /// assert_eq!(format!("{:X}", h), h.to_hex());
    /// # Ok(()) }
    /// ```
    pub fn to_hex(&self) -> String {
        format!("{:X}", self)
    }

    /// Converts this `KeyHandle` to its hexadecimal representation
    /// with spaces.
    ///
    /// This representation is always uppercase and with spaces
    /// grouping the hexadecimal digits into groups of four.  It is
    /// only suitable for manual comparison of key handles.
    ///
    /// Note: The spaces will hinder other kind of use cases.  For
    /// example, it is harder to select the whole key handle for
    /// copying, and it has to be quoted when used as a command line
    /// argument.  Only use this form for displaying a key handle with
    /// the intent of manual comparisons.
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// # use sequoia_openpgp as openpgp;
    /// use openpgp::KeyHandle;
    ///
    /// let h: KeyHandle =
    ///     "0123 4567 89AB CDEF 0123 4567 89AB CDEF 0123 4567".parse()?;
    ///
    /// assert_eq!("0123 4567 89AB CDEF 0123  4567 89AB CDEF 0123 4567",
    ///            h.to_spaced_hex());
    /// # Ok(()) }
    /// ```
    pub fn to_spaced_hex(&self) -> String {
        match self {
            KeyHandle::Fingerprint(v) => v.to_spaced_hex(),
            KeyHandle::KeyID(v) => v.to_spaced_hex(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upper_hex_formatting() {
        let handle = KeyHandle::Fingerprint(Fingerprint::V4([1, 2, 3, 4, 5, 6, 7,
            8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20]));
        assert_eq!(format!("{:X}", handle), "0102030405060708090A0B0C0D0E0F1011121314");

        let handle = KeyHandle::Fingerprint(Fingerprint::Invalid(Box::new([10, 2, 3, 4])));
        assert_eq!(format!("{:X}", handle), "0A020304");

        let handle = KeyHandle::KeyID(KeyID::V4([10, 2, 3, 4, 5, 6, 7, 8]));
        assert_eq!(format!("{:X}", handle), "0A02030405060708");

        let handle = KeyHandle::KeyID(KeyID::Invalid(Box::new([10, 2])));
        assert_eq!(format!("{:X}", handle), "0A02");
    }

    #[test]
    fn lower_hex_formatting() {
        let handle = KeyHandle::Fingerprint(Fingerprint::V4([1, 2, 3, 4, 5, 6, 7,
            8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20]));
        assert_eq!(format!("{:x}", handle), "0102030405060708090a0b0c0d0e0f1011121314");

        let handle = KeyHandle::Fingerprint(Fingerprint::Invalid(Box::new([10, 2, 3, 4])));
        assert_eq!(format!("{:x}", handle), "0a020304");

        let handle = KeyHandle::KeyID(KeyID::V4([10, 2, 3, 4, 5, 6, 7, 8]));
        assert_eq!(format!("{:x}", handle), "0a02030405060708");

        let handle = KeyHandle::KeyID(KeyID::Invalid(Box::new([10, 2])));
        assert_eq!(format!("{:x}", handle), "0a02");
    }

    #[test]
    fn parse() -> Result<()> {
        let handle: KeyHandle =
            "0123 4567 89AB CDEF 0123 4567 89AB CDEF 0123 4567".parse()?;
        assert_match!(&KeyHandle::Fingerprint(Fingerprint::V4(_)) = &handle);
        assert_eq!(handle.as_bytes(),
                   [0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0x01, 0x23,
                    0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0x01, 0x23, 0x45, 0x67]);

        let handle: KeyHandle = "89AB CDEF 0123 4567".parse()?;
        assert_match!(&KeyHandle::KeyID(KeyID::V4(_)) = &handle);
        assert_eq!(handle.as_bytes(),
                   [0x89, 0xAB, 0xCD, 0xEF, 0x01, 0x23, 0x45, 0x67]);

        // Invalid handles are parsed as invalid Fingerprints, not
        // invalid KeyIDs.
        let handle: KeyHandle = "4567 89AB CDEF 0123 4567".parse()?;
        assert_match!(&KeyHandle::Fingerprint(Fingerprint::Invalid(_)) = &handle);
        assert_eq!(handle.as_bytes(),
                   [0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0x01, 0x23, 0x45, 0x67]);

        let handle: Result<KeyHandle> = "INVALID CHARACTERS".parse();
        assert!(handle.is_err());

        Ok(())
    }
}
