use std::fmt;
use std::ops::{BitAnd, BitOr};

#[cfg(test)]
use quickcheck::{Arbitrary, Gen};

use crate::types::Bitfield;

/// Describes how a key may be used, and stores additional information.
///
/// Key flags are described in [Section 5.2.3.21 of RFC 4880] and [Section 5.2.3.22
/// of RFC 4880bis].
///
/// [Section 5.2.3.21 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.21
/// [Section 5.2.3.22 of RFC 4880bis]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09#section-5.2.3.22
///
/// # A note on equality
///
/// `PartialEq` compares the serialized form of the key flag sets.  If
/// you prefer to compare two key flag sets for semantic equality, you
/// should use [`KeyFlags::normalized_eq`].  The difference between
/// semantic equality and serialized equality is that semantic
/// equality ignores differences in the amount of padding.
///
///   [`KeyFlags::normalized_eq`]: KeyFlags::normalized_eq()
///
/// # Examples
///
/// ```
/// use sequoia_openpgp as openpgp;
/// # use openpgp::Result;
/// use openpgp::cert::prelude::*;
/// use openpgp::policy::StandardPolicy;
///
/// # fn main() -> Result<()> {
/// let p = &StandardPolicy::new();
///
/// let (cert, _) =
///     CertBuilder::new()
///         .add_userid("Alice <alice@example.com>")
///         .add_transport_encryption_subkey()
///         .generate()?;
///
/// for subkey in cert.with_policy(p, None)?.keys().subkeys() {
///     // Key contains one Encryption subkey:
///     assert!(subkey.key_flags().unwrap().for_transport_encryption());
/// }
/// # Ok(()) }
/// ```
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyFlags(Bitfield);
assert_send_and_sync!(KeyFlags);

impl fmt::Debug for KeyFlags {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.for_certification() {
            f.write_str("C")?;
        }
        if self.for_signing() {
            f.write_str("S")?;
        }
        if self.for_transport_encryption() {
            f.write_str("Et")?;
        }
        if self.for_storage_encryption() {
            f.write_str("Er")?;
        }
        if self.for_authentication() {
            f.write_str("A")?;
        }
        if self.is_split_key() {
            f.write_str("D")?;
        }
        if self.is_group_key() {
            f.write_str("G")?;
        }

        let mut need_comma = false;
        for i in self.0.iter() {
            match i {
                KEY_FLAG_CERTIFY
                    | KEY_FLAG_SIGN
                    | KEY_FLAG_ENCRYPT_FOR_TRANSPORT
                    | KEY_FLAG_ENCRYPT_AT_REST
                    | KEY_FLAG_SPLIT_KEY
                    | KEY_FLAG_AUTHENTICATE
                    | KEY_FLAG_GROUP_KEY
                    => (),
                i => {
                    if need_comma { f.write_str(", ")?; }
                    write!(f, "#{}", i)?;
                    need_comma = true;
                },
            }
        }

        // Mention any padding, as equality is sensitive to this.
        let padding = self.0.padding_len();
        if padding > 0 {
            if need_comma { f.write_str(", ")?; }
            write!(f, "+padding({} bytes)", padding)?;
        }

        Ok(())
    }
}

impl BitAnd for &KeyFlags {
    type Output = KeyFlags;

    fn bitand(self, rhs: Self) -> KeyFlags {
        let l = self.as_slice();
        let r = rhs.as_slice();

        let mut c = Vec::with_capacity(std::cmp::min(l.len(), r.len()));
        for (l, r) in l.iter().zip(r.iter()) {
            c.push(l & r);
        }

        KeyFlags(c.into())
    }
}

impl BitOr for &KeyFlags {
    type Output = KeyFlags;

    fn bitor(self, rhs: Self) -> KeyFlags {
        let l = self.as_slice();
        let r = rhs.as_slice();

        // Make l the longer one.
        let (l, r) = if l.len() > r.len() {
            (l, r)
        } else {
            (r, l)
        };

        let mut l = l.to_vec();
        for (i, r) in r.iter().enumerate() {
            l[i] |= r;
        }

        KeyFlags(l.into())
    }
}

impl AsRef<KeyFlags> for KeyFlags {
    fn as_ref(&self) -> &KeyFlags {
        self
    }
}

impl KeyFlags {
    /// Creates a new instance from `bits`.
    pub fn new<B: AsRef<[u8]>>(bits: B) -> Self {
        Self(bits.as_ref().to_vec().into())
    }

    /// Returns a new `KeyFlags` with all capabilities disabled.
    pub fn empty() -> Self {
        KeyFlags::new(&[])
    }

    /// Returns a slice containing the raw values.
    pub(crate) fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }

    /// Compares two key flag sets for semantic equality.
    ///
    /// `KeyFlags`' implementation of `PartialEq` compares two key
    /// flag sets for serialized equality.  That is, the `PartialEq`
    /// implementation considers two key flag sets to *not* be equal
    /// if they have different amounts of padding.  This comparison
    /// function ignores padding.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::types::KeyFlags;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let a = KeyFlags::new(&[0x1]);
    /// let b = KeyFlags::new(&[0x1, 0x0]);
    ///
    /// assert!(a != b);
    /// assert!(a.normalized_eq(&b));
    /// # Ok(()) }
    /// ```
    pub fn normalized_eq(&self, other: &Self) -> bool {
        self.0.normalized_eq(&other.0)
    }

    /// Returns whether the specified key flag is set.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::types::KeyFlags;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// // Key flags 0 and 2.
    /// let kf = KeyFlags::new(&[0x5]);
    ///
    /// assert!(kf.get(0));
    /// assert!(! kf.get(1));
    /// assert!(kf.get(2));
    /// assert!(! kf.get(3));
    /// assert!(! kf.get(8));
    /// assert!(! kf.get(80));
    /// # assert!(kf.for_certification());
    /// # Ok(()) }
    /// ```
    pub fn get(&self, bit: usize) -> bool {
        self.0.get(bit)
    }

    /// Sets the specified key flag.
    ///
    /// This also clears any padding (trailing NUL bytes).
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::types::KeyFlags;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let kf = KeyFlags::empty().set(0).set(2);
    ///
    /// assert!(kf.get(0));
    /// assert!(! kf.get(1));
    /// assert!(kf.get(2));
    /// assert!(! kf.get(3));
    /// # assert!(kf.for_certification());
    /// # Ok(()) }
    /// ```
    pub fn set(self, bit: usize) -> Self {
        Self(self.0.set(bit))
    }

    /// Clears the specified key flag.
    ///
    /// This also clears any padding (trailing NUL bytes).
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::types::KeyFlags;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let kf = KeyFlags::empty().set(0).set(2).clear(2);
    ///
    /// assert!(kf.get(0));
    /// assert!(! kf.get(1));
    /// assert!(! kf.get(2));
    /// assert!(! kf.get(3));
    /// # assert!(kf.for_certification());
    /// # Ok(()) }
    /// ```
    pub fn clear(self, bit: usize) -> Self {
        Self(self.0.clear(bit))
    }

    /// This key may be used to certify other keys.
    pub fn for_certification(&self) -> bool {
        self.get(KEY_FLAG_CERTIFY)
    }

    /// Declares that this key may be used to certify other keys.
    pub fn set_certification(self) -> Self {
        self.set(KEY_FLAG_CERTIFY)
    }

    /// Declares that this key may not be used to certify other keys.
    pub fn clear_certification(self) -> Self {
        self.clear(KEY_FLAG_CERTIFY)
    }

    /// This key may be used to sign data.
    pub fn for_signing(&self) -> bool {
        self.get(KEY_FLAG_SIGN)
    }

    /// Declares that this key may be used to sign data.
    pub fn set_signing(self) -> Self {
        self.set(KEY_FLAG_SIGN)
    }

    /// Declares that this key may not be used to sign data.
    pub fn clear_signing(self) -> Self {
        self.clear(KEY_FLAG_SIGN)
    }

    /// This key may be used to encrypt communications.
    pub fn for_transport_encryption(&self) -> bool {
        self.get(KEY_FLAG_ENCRYPT_FOR_TRANSPORT)
    }

    /// Declares that this key may be used to encrypt communications.
    pub fn set_transport_encryption(self) -> Self {
        self.set(KEY_FLAG_ENCRYPT_FOR_TRANSPORT)
    }

    /// Declares that this key may not be used to encrypt communications.
    pub fn clear_transport_encryption(self) -> Self {
        self.clear(KEY_FLAG_ENCRYPT_FOR_TRANSPORT)
    }

    /// This key may be used to encrypt storage.
    pub fn for_storage_encryption(&self) -> bool {
        self.get(KEY_FLAG_ENCRYPT_AT_REST)
    }

    /// Declares that this key may be used to encrypt storage.
    pub fn set_storage_encryption(self) -> Self {
        self.set(KEY_FLAG_ENCRYPT_AT_REST)
    }

    /// Declares that this key may not be used to encrypt storage.
    pub fn clear_storage_encryption(self) -> Self {
        self.clear(KEY_FLAG_ENCRYPT_AT_REST)
    }

    /// This key may be used for authentication.
    pub fn for_authentication(&self) -> bool {
        self.get(KEY_FLAG_AUTHENTICATE)
    }

    /// Declares that this key may be used for authentication.
    pub fn set_authentication(self) -> Self {
        self.set(KEY_FLAG_AUTHENTICATE)
    }

    /// Declares that this key may not be used for authentication.
    pub fn clear_authentication(self) -> Self {
        self.clear(KEY_FLAG_AUTHENTICATE)
    }

    /// The private component of this key may have been split
    /// using a secret-sharing mechanism.
    pub fn is_split_key(&self) -> bool {
        self.get(KEY_FLAG_SPLIT_KEY)
    }

    /// Declares that the private component of this key may have been
    /// split using a secret-sharing mechanism.
    pub fn set_split_key(self) -> Self {
        self.set(KEY_FLAG_SPLIT_KEY)
    }

    /// Declares that the private component of this key has not been
    /// split using a secret-sharing mechanism.
    pub fn clear_split_key(self) -> Self {
        self.clear(KEY_FLAG_SPLIT_KEY)
    }

    /// The private component of this key may be in possession of more
    /// than one person.
    pub fn is_group_key(&self) -> bool {
        self.get(KEY_FLAG_GROUP_KEY)
    }

    /// Declares that the private component of this key is in
    /// possession of more than one person.
    pub fn set_group_key(self) -> Self {
        self.set(KEY_FLAG_GROUP_KEY)
    }

    /// Declares that the private component of this key should not be
    /// in possession of more than one person.
    pub fn clear_group_key(self) -> Self {
        self.clear(KEY_FLAG_GROUP_KEY)
    }

    /// Returns whether no flags are set.
    pub fn is_empty(&self) -> bool {
        self.as_slice().iter().all(|b| *b == 0)
    }
}

/// This key may be used to certify other keys.
const KEY_FLAG_CERTIFY: usize = 0;

/// This key may be used to sign data.
const KEY_FLAG_SIGN: usize = 1;

/// This key may be used to encrypt communications.
const KEY_FLAG_ENCRYPT_FOR_TRANSPORT: usize = 2;

/// This key may be used to encrypt storage.
const KEY_FLAG_ENCRYPT_AT_REST: usize = 3;

/// The private component of this key may have been split by a
/// secret-sharing mechanism.
const KEY_FLAG_SPLIT_KEY: usize = 4;

/// This key may be used for authentication.
const KEY_FLAG_AUTHENTICATE: usize = 5;

/// The private component of this key may be in the possession of more
/// than one person.
const KEY_FLAG_GROUP_KEY: usize = 7;

#[cfg(test)]
impl Arbitrary for KeyFlags {
    fn arbitrary(g: &mut Gen) -> Self {
        Self::new(Vec::arbitrary(g))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    quickcheck! {
        fn roundtrip(val: KeyFlags) -> bool {
            let mut q = KeyFlags::new(&val.as_slice());
            assert_eq!(val, q);
            assert!(val.normalized_eq(&q));

            // Add some padding to q.  Make sure they are still equal.
            q.0.raw.push(0);
            assert!(val != q);
            assert!(val.normalized_eq(&q));

            q.0.raw.push(0);
            assert!(val != q);
            assert!(val.normalized_eq(&q));

            true
        }
    }
}
