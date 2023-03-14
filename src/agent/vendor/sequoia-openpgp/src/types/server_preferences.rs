use std::fmt;

#[cfg(test)]
use quickcheck::{Arbitrary, Gen};

use crate::types::Bitfield;

/// Describes preferences regarding key servers.
///
/// Key server preferences are specified in [Section 5.2.3.17 of RFC 4880] and
/// [Section 5.2.3.18 of RFC 4880bis].
///
/// [Section 5.2.3.17 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.17
/// [Section 5.2.3.18 of RFC 4880bis]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09#section-5.2.3.18
///
/// The keyserver preferences are set by the user's OpenPGP
/// implementation to communicate them to any peers.
///
/// # A note on equality
///
/// `PartialEq` compares the serialized form of the two key server
/// preference sets.  If you prefer to compare two key server
/// preference sets for semantic equality, you should use
/// [`KeyServerPreferences::normalized_eq`].  The difference between
/// semantic equality and serialized equality is that semantic
/// equality ignores differences in the amount of padding.
///
///   [`KeyServerPreferences::normalized_eq`]: KeyServerPreferences::normalized_eq()
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
///     CertBuilder::general_purpose(None, Some("alice@example.org"))
///     .generate()?;
///
/// match cert.with_policy(p, None)?.primary_userid()?.key_server_preferences() {
///     Some(preferences) => {
///         println!("Certificate holder's keyserver preferences:");
///         assert!(preferences.no_modify());
/// #       unreachable!();
///     }
///     None => {
///         println!("Certificate Holder did not specify any key server preferences.");
///     }
/// }
/// # Ok(()) }
/// ```
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyServerPreferences(Bitfield);
assert_send_and_sync!(KeyServerPreferences);

impl fmt::Debug for KeyServerPreferences {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut need_comma = false;
        if self.no_modify() {
            f.write_str("no modify")?;
            need_comma = true;
        }

        for i in self.0.iter() {
            match i {
                KEYSERVER_PREFERENCE_NO_MODIFY => (),
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

impl KeyServerPreferences {
    /// Creates a new instance from `bits`.
    pub fn new<B: AsRef<[u8]>>(bits: B) -> Self {
        KeyServerPreferences(bits.as_ref().to_vec().into())
    }

    /// Returns an empty key server preference set.
    pub fn empty() -> Self {
        Self::new(&[])
    }

    /// Returns a slice containing the raw values.
    pub(crate) fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }

    /// Compares two key server preference sets for semantic equality.
    ///
    /// `KeyServerPreferences`' implementation of `PartialEq` compares
    /// two key server preference sets for serialized equality.  That
    /// is, the `PartialEq` implementation considers two key server
    /// preference sets to *not* be equal if they have different
    /// amounts of padding.  This comparison function ignores padding.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::types::KeyServerPreferences;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let a = KeyServerPreferences::new(&[0x1]);
    /// let b = KeyServerPreferences::new(&[0x1, 0x0]);
    ///
    /// assert!(a != b);
    /// assert!(a.normalized_eq(&b));
    /// # Ok(()) }
    /// ```
    pub fn normalized_eq(&self, other: &Self) -> bool {
        self.0.normalized_eq(&other.0)
    }

    /// Returns whether the specified keyserver preference flag is set.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::types::KeyServerPreferences;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// // Keyserver Preferences flags 0 and 2.
    /// let ksp = KeyServerPreferences::new(&[0x5]);
    ///
    /// assert!(ksp.get(0));
    /// assert!(! ksp.get(1));
    /// assert!(ksp.get(2));
    /// assert!(! ksp.get(3));
    /// assert!(! ksp.get(8));
    /// assert!(! ksp.get(80));
    /// # assert!(! ksp.no_modify());
    /// # Ok(()) }
    /// ```
    pub fn get(&self, bit: usize) -> bool {
        self.0.get(bit)
    }

    /// Sets the specified keyserver preference flag.
    ///
    /// This also clears any padding (trailing NUL bytes).
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::types::KeyServerPreferences;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let ksp = KeyServerPreferences::empty().set(0).set(2);
    ///
    /// assert!(ksp.get(0));
    /// assert!(! ksp.get(1));
    /// assert!(ksp.get(2));
    /// assert!(! ksp.get(3));
    /// # assert!(! ksp.no_modify());
    /// # Ok(()) }
    /// ```
    pub fn set(self, bit: usize) -> Self {
        Self(self.0.set(bit))
    }

    /// Clears the specified keyserver preference flag.
    ///
    /// This also clears any padding (trailing NUL bytes).
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::types::KeyServerPreferences;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let ksp = KeyServerPreferences::empty().set(0).set(2).clear(2);
    ///
    /// assert!(ksp.get(0));
    /// assert!(! ksp.get(1));
    /// assert!(! ksp.get(2));
    /// assert!(! ksp.get(3));
    /// # assert!(! ksp.no_modify());
    /// # Ok(()) }
    /// ```
    pub fn clear(self, bit: usize) -> Self {
        Self(self.0.clear(bit))
    }

    /// Returns whether the certificate's owner requests that the
    /// certificate is not modified.
    ///
    /// If this flag is set, the certificate's owner requests that the
    /// certificate should only be changed by the owner and the key
    /// server's operator.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::types::KeyServerPreferences;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let ksp = KeyServerPreferences::empty();
    /// assert!(! ksp.no_modify());
    /// # Ok(()) }
    /// ```
    pub fn no_modify(&self) -> bool {
        self.get(KEYSERVER_PREFERENCE_NO_MODIFY)
    }

    /// Requests that the certificate is not modified.
    ///
    /// See [`KeyServerPreferences::no_modify`].
    ///
    ///   [`KeyServerPreferences::no_modify`]: KeyServerPreferences::no_modify()
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::types::KeyServerPreferences;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let ksp = KeyServerPreferences::empty().set_no_modify();
    /// assert!(ksp.no_modify());
    /// # Ok(()) }
    /// ```
    pub fn set_no_modify(self) -> Self {
        self.set(KEYSERVER_PREFERENCE_NO_MODIFY)
    }

    /// Clears the request that the certificate is not modified.
    ///
    /// See [`KeyServerPreferences::no_modify`].
    ///
    ///   [`KeyServerPreferences::no_modify`]: KeyServerPreferences::no_modify()
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::types::KeyServerPreferences;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// let ksp = KeyServerPreferences::new(&[0x80][..]);
    /// assert!(ksp.no_modify());
    /// let ksp = ksp.clear_no_modify();
    /// assert!(! ksp.no_modify());
    /// # Ok(()) }
    /// ```
    pub fn clear_no_modify(self) -> Self {
        self.clear(KEYSERVER_PREFERENCE_NO_MODIFY)
    }
}

/// The key holder requests that this key only be modified or updated
/// by the key holder or an administrator of the key server.
const KEYSERVER_PREFERENCE_NO_MODIFY: usize = 7;

#[cfg(test)]
impl Arbitrary for KeyServerPreferences {
    fn arbitrary(g: &mut Gen) -> Self {
        Self::new(Vec::arbitrary(g))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basics() -> crate::Result<()> {
        let p = KeyServerPreferences::empty();
        assert_eq!(p.no_modify(), false);
        let p = KeyServerPreferences::new(&[]);
        assert_eq!(p.no_modify(), false);
        let p = KeyServerPreferences::new(&[0xff]);
        assert_eq!(p.no_modify(), true);
        Ok(())
    }

    quickcheck! {
        fn roundtrip(val: KeyServerPreferences) -> bool {
            let mut q = KeyServerPreferences::new(val.as_slice());
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
