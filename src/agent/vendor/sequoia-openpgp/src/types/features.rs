use std::fmt;

#[cfg(test)]
use quickcheck::{Arbitrary, Gen};

use crate::types::Bitfield;

/// Describes the features supported by an OpenPGP implementation.
///
/// The feature flags are defined in [Section 5.2.3.24 of RFC 4880],
/// and [Section 5.2.3.25 of RFC 4880bis].
///
/// [Section 5.2.3.24 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.2.3.24
/// [Section 5.2.3.25 of RFC 4880bis]: https://tools.ietf.org/html/draft-ietf-openpgp-rfc4880bis-09#section-5.2.3.25
///
/// The feature flags are set by the user's OpenPGP implementation to
/// signal to any senders what features the implementation supports.
///
/// # A note on equality
///
/// `PartialEq` compares the serialized form of the two feature sets.
/// If you prefer to compare two feature sets for semantic equality,
/// you should use [`Features::normalized_eq`].  The difference
/// between semantic equality and serialized equality is that semantic
/// equality ignores differences in the amount of padding.
///
///   [`Features::normalized_eq`]: Features::normalized_eq()
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
/// # let (cert, _) =
/// #     CertBuilder::general_purpose(None, Some("alice@example.org"))
/// #     .generate()?;
/// match cert.with_policy(p, None)?.primary_userid()?.features() {
///     Some(features) => {
///         println!("Certificate holder's supported features:");
///         assert!(features.supports_mdc());
///         assert!(!features.supports_aead());
///     }
///     None => {
///         println!("Certificate Holder did not specify any features.");
/// #       unreachable!();
///     }
/// }
/// # Ok(()) }
/// ```
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Features(Bitfield);
assert_send_and_sync!(Features);

impl fmt::Debug for Features {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Print known features first.
        let mut need_comma = false;
        if self.supports_mdc() {
            f.write_str("MDC")?;
            need_comma = true;
        }
        if self.supports_aead() {
            if need_comma { f.write_str(", ")?; }
            f.write_str("AEAD")?;
            need_comma = true;
        }

        // Now print any unknown features.
        for i in self.0.iter() {
            match i {
                FEATURE_FLAG_MDC => (),
                FEATURE_FLAG_AEAD => (),
                i => {
                    if need_comma { f.write_str(", ")?; }
                    write!(f, "#{}", i)?;
                    need_comma = true;
                }
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

impl Features {
    /// Creates a new instance from `bytes`.
    ///
    /// This does not remove any trailing padding from `bytes`.
    pub fn new<B>(bytes: B) -> Self
        where B: AsRef<[u8]>
    {
        Features(bytes.as_ref().to_vec().into())
    }

    /// Returns an empty feature set.
    pub fn empty() -> Self {
        Self::new(&[][..])
    }

    /// Returns a feature set describing Sequoia's capabilities.
    pub fn sequoia() -> Self {
        let v : [u8; 1] = [ 0 ];

        Self::new(&v[..]).set_mdc()
    }

    /// Compares two feature sets for semantic equality.
    ///
    /// `Features`' implementation of `PartialEq` compares two feature
    /// sets for serialized equality.  That is, the `PartialEq`
    /// implementation considers two feature sets to *not* be equal if
    /// they have different amounts of padding.  This comparison
    /// function ignores padding.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::types::Features;
    ///
    /// # fn main() -> Result<()> {
    /// let a = Features::new(&[0x1]);
    /// let b = Features::new(&[0x1, 0x0]);
    ///
    /// assert!(a != b);
    /// assert!(a.normalized_eq(&b));
    /// # Ok(()) }
    /// ```
    pub fn normalized_eq(&self, other: &Self) -> bool {
        self.0.normalized_eq(&other.0)
    }

    /// Returns a slice containing the raw values.
    pub(crate) fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }

    /// Returns whether the specified feature flag is set.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::types::Features;
    ///
    /// # fn main() -> Result<()> {
    /// // Feature flags 0 and 2.
    /// let f = Features::new(&[0x5]);
    ///
    /// assert!(f.get(0));
    /// assert!(! f.get(1));
    /// assert!(f.get(2));
    /// assert!(! f.get(3));
    /// assert!(! f.get(8));
    /// assert!(! f.get(80));
    /// # assert!(f.supports_mdc());
    /// # assert!(! f.supports_aead());
    /// # Ok(()) }
    /// ```
    pub fn get(&self, bit: usize) -> bool {
        self.0.get(bit)
    }

    /// Sets the specified feature flag.
    ///
    /// This also clears any padding (trailing NUL bytes).
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::types::Features;
    ///
    /// # fn main() -> Result<()> {
    /// let f = Features::empty().set(0).set(2);
    ///
    /// assert!(f.get(0));
    /// assert!(! f.get(1));
    /// assert!(f.get(2));
    /// assert!(! f.get(3));
    /// # assert!(f.supports_mdc());
    /// # assert!(! f.supports_aead());
    /// # Ok(()) }
    /// ```
    pub fn set(self, bit: usize) -> Self {
        Self(self.0.set(bit))
    }

    /// Clears the specified feature flag.
    ///
    /// This also clears any padding (trailing NUL bytes).
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::types::Features;
    ///
    /// # fn main() -> Result<()> {
    /// let f = Features::empty().set(0).set(2).clear(2);
    ///
    /// assert!(f.get(0));
    /// assert!(! f.get(1));
    /// assert!(! f.get(2));
    /// assert!(! f.get(3));
    /// # assert!(f.supports_mdc());
    /// # assert!(! f.supports_aead());
    /// # Ok(()) }
    /// ```
    pub fn clear(self, bit: usize) -> Self {
        Self(self.0.clear(bit))
    }

    /// Returns whether the MDC feature flag is set.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::types::Features;
    ///
    /// # fn main() -> Result<()> {
    /// let f = Features::empty();
    ///
    /// assert!(! f.supports_mdc());
    /// # Ok(()) }
    /// ```
    pub fn supports_mdc(&self) -> bool {
        self.get(FEATURE_FLAG_MDC)
    }

    /// Sets the MDC feature flag.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::types::Features;
    ///
    /// # fn main() -> Result<()> {
    /// let f = Features::empty().set_mdc();
    ///
    /// assert!(f.supports_mdc());
    /// # assert!(f.get(0));
    /// # Ok(()) }
    /// ```
    pub fn set_mdc(self) -> Self {
        self.set(FEATURE_FLAG_MDC)
    }

    /// Clears the MDC feature flag.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::types::Features;
    ///
    /// # fn main() -> Result<()> {
    /// let f = Features::new(&[0x1]);
    /// assert!(f.supports_mdc());
    ///
    /// let f = f.clear_mdc();
    /// assert!(! f.supports_mdc());
    /// # Ok(()) }
    /// ```
    pub fn clear_mdc(self) -> Self {
        self.clear(FEATURE_FLAG_MDC)
    }

    /// Returns whether the AEAD feature flag is set.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::types::Features;
    ///
    /// # fn main() -> Result<()> {
    /// let f = Features::empty();
    ///
    /// assert!(! f.supports_aead());
    /// # Ok(()) }
    /// ```
    pub fn supports_aead(&self) -> bool {
        self.get(FEATURE_FLAG_AEAD)
    }

    /// Sets the AEAD feature flag.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::types::Features;
    ///
    /// # fn main() -> Result<()> {
    /// let f = Features::empty().set_aead();
    ///
    /// assert!(f.supports_aead());
    /// # assert!(f.get(1));
    /// # Ok(()) }
    /// ```
    pub fn set_aead(self) -> Self {
        self.set(FEATURE_FLAG_AEAD)
    }

    /// Clears the AEAD feature flag.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::types::Features;
    ///
    /// # fn main() -> Result<()> {
    /// let f = Features::new(&[0x2]);
    /// assert!(f.supports_aead());
    ///
    /// let f = f.clear_aead();
    /// assert!(! f.supports_aead());
    /// # Ok(()) }
    /// ```
    pub fn clear_aead(self) -> Self {
        self.clear(FEATURE_FLAG_AEAD)
    }
}

/// Modification Detection (packets 18 and 19).
const FEATURE_FLAG_MDC: usize = 0;

/// AEAD Encrypted Data Packet (packet 20) and version 5 Symmetric-Key
/// Encrypted Session Key Packets (packet 3).
const FEATURE_FLAG_AEAD: usize = 1;

#[cfg(test)]
impl Arbitrary for Features {
    fn arbitrary(g: &mut Gen) -> Self {
        Self::new(Vec::arbitrary(g))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    quickcheck! {
        fn roundtrip(val: Features) -> bool {
            let mut q = Features::new(val.as_slice());
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

    #[test]
    fn set_clear() {
        let a = Features::new(&[ 0x5, 0x1, 0x0, 0xff ]);
        let b = Features::new(&[])
            .set(0).set(2)
            .set(8)
            .set(24).set(25).set(26).set(27).set(28).set(29).set(30).set(31);
        assert_eq!(a, b);

        // Clear a bit and make sure they are not equal.
        let b = b.clear(0);
        assert!(a != b);
        assert!(! a.normalized_eq(&b));
        let b = b.set(0);
        assert_eq!(a, b);
        assert!(a.normalized_eq(&b));

        let b = b.clear(8);
        assert!(a != b);
        assert!(! a.normalized_eq(&b));
        let b = b.set(8);
        assert_eq!(a, b);
        assert!(a.normalized_eq(&b));

        let b = b.clear(31);
        assert!(a != b);
        assert!(! a.normalized_eq(&b));
        let b = b.set(31);
        assert_eq!(a, b);
        assert!(a.normalized_eq(&b));

        // Add a bit.
        let a = a.set(10);
        assert!(a != b);
        assert!(! a.normalized_eq(&b));
        let b = b.set(10);
        assert_eq!(a, b);
        assert!(a.normalized_eq(&b));

        let a = a.set(32);
        assert!(a != b);
        assert!(! a.normalized_eq(&b));
        let b = b.set(32);
        assert_eq!(a, b);
        assert!(a.normalized_eq(&b));

        let a = a.set(1000);
        assert!(a != b);
        assert!(! a.normalized_eq(&b));
        let b = b.set(1000);
        assert_eq!(a, b);
        assert!(a.normalized_eq(&b));
    }

    #[test]
    fn known() {
        let a = Features::empty().set_mdc();
        let b = Features::new(&[ 0x1 ]);
        assert_eq!(a, b);
        assert!(a.normalized_eq(&b));

        let a = Features::empty().set_aead();
        let b = Features::new(&[ 0x2 ]);
        assert_eq!(a, b);
        assert!(a.normalized_eq(&b));

        let a = Features::empty().set_mdc().set_aead();
        let b = Features::new(&[ 0x1 | 0x2 ]);
        assert_eq!(a, b);
        assert!(a.normalized_eq(&b));
    }
}
