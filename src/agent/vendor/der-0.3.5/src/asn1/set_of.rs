//! ASN.1 `SET OF` support.

use crate::{
    Any, ByteSlice, Decodable, Decoder, Encodable, Encoder, Error, ErrorKind, Length, Result, Tag,
    Tagged,
};
use core::{convert::TryFrom, marker::PhantomData};

#[cfg(feature = "alloc")]
use {
    crate::Header,
    alloc::collections::{btree_set, BTreeSet},
};

/// ASN.1 `SET OF` denotes a collection of zero or more occurrences of a
/// given type.
///
/// When encoded as DER, `SET OF` is lexicographically ordered. To implement
/// that requirement, types `T` which are elements of [`SetOf`] MUST provide
/// an impl of `Ord` which ensures that the corresponding DER encodings of
/// a given type are ordered.
pub trait SetOf<'a, 'b, T>: Decodable<'a> + Encodable
where
    T: Clone + Decodable<'a> + Encodable + Ord,
{
    /// Iterator over the elements of the set.
    ///
    /// The iterator type MUST maintain the invariant that messages are
    /// lexicographically ordered.
    ///
    /// See toplevel documentation about `Ord` trait requirements for
    /// more information.
    type Iter: Iterator<Item = T>;

    /// Iterate over the elements of the set.
    fn elements(&'b self) -> Self::Iter;
}

/// ASN.1 `SET OF` backed by a byte slice containing serialized DER.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SetOfRef<'a, T>
where
    T: Clone + Decodable<'a> + Encodable + Ord,
{
    /// DER-encoded byte slice
    inner: ByteSlice<'a>,

    /// Set element type
    element_type: PhantomData<T>,
}

impl<'a, T> SetOfRef<'a, T>
where
    T: Clone + Decodable<'a> + Encodable + Ord,
{
    /// Create a new [`SetOfRef`] from a slice.
    pub fn new(slice: &'a [u8]) -> Result<Self> {
        let inner = ByteSlice::new(slice).map_err(|_| ErrorKind::Length { tag: Self::TAG })?;

        let mut decoder = Decoder::new(slice);
        let mut last_value = None;

        // Validate that we can decode all elements in the slice, and that they
        // are lexicographically ordered according to DER's rules
        while !decoder.is_finished() {
            let value: T = decoder.decode()?;

            if let Some(last) = last_value.as_ref() {
                if last >= &value {
                    return Err(ErrorKind::Noncanonical.into());
                }
            }

            last_value = Some(value);
        }

        Ok(Self {
            inner,
            element_type: PhantomData,
        })
    }

    /// Borrow the inner byte sequence.
    pub fn as_bytes(&self) -> &'a [u8] {
        self.inner.as_bytes()
    }
}

impl<'a, T> AsRef<[u8]> for SetOfRef<'a, T>
where
    T: Clone + Decodable<'a> + Encodable + Ord,
{
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl<'a, T> TryFrom<Any<'a>> for SetOfRef<'a, T>
where
    T: Clone + Decodable<'a> + Encodable + Ord,
{
    type Error = Error;

    fn try_from(any: Any<'a>) -> Result<Self> {
        any.tag().assert_eq(Tag::Set)?;
        Self::new(any.as_bytes())
    }
}

impl<'a, T> From<SetOfRef<'a, T>> for Any<'a>
where
    T: Clone + Decodable<'a> + Encodable + Ord,
{
    fn from(set: SetOfRef<'a, T>) -> Any<'a> {
        Any::from_tag_and_value(Tag::Set, set.inner)
    }
}

impl<'a, T> Encodable for SetOfRef<'a, T>
where
    T: Clone + Decodable<'a> + Encodable + Ord,
{
    fn encoded_len(&self) -> Result<Length> {
        Any::from(self.clone()).encoded_len()
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<()> {
        Any::from(self.clone()).encode(encoder)
    }
}

impl<'a, 'b, T> SetOf<'a, 'b, T> for SetOfRef<'a, T>
where
    T: Clone + Decodable<'a> + Encodable + Ord,
{
    type Iter = SetOfRefIter<'a, T>;

    fn elements(&'b self) -> Self::Iter {
        SetOfRefIter::new(self)
    }
}

impl<'a, T> Tagged for SetOfRef<'a, T>
where
    T: Clone + Decodable<'a> + Encodable + Ord,
{
    const TAG: Tag = Tag::Set;
}

/// Iterator over the elements of an [`SetOfRef`].
pub struct SetOfRefIter<'a, T>
where
    T: Clone + Decodable<'a> + Encodable + Ord,
{
    /// Decoder which iterates over the elements of the message
    decoder: Decoder<'a>,

    /// Element type
    element_type: PhantomData<T>,
}

impl<'a, T> SetOfRefIter<'a, T>
where
    T: Clone + Decodable<'a> + Encodable + Ord,
{
    pub(crate) fn new(set: &SetOfRef<'a, T>) -> Self {
        Self {
            decoder: Decoder::new(set.as_bytes()),
            element_type: PhantomData,
        }
    }
}

impl<'a, T> Iterator for SetOfRefIter<'a, T>
where
    T: Clone + Decodable<'a> + Encodable + Ord,
{
    type Item = T;

    fn next(&mut self) -> Option<T> {
        if self.decoder.is_finished() {
            None
        } else {
            Some(
                self.decoder
                    .decode()
                    .expect("SetOfRef decodable invariant violated"),
            )
        }
    }
}

#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
impl<'a, T> TryFrom<Any<'a>> for BTreeSet<T>
where
    T: Clone + Decodable<'a> + Encodable + Ord,
{
    type Error = Error;

    fn try_from(any: Any<'a>) -> Result<Self> {
        any.tag().assert_eq(Tag::Set)?;

        let mut result = BTreeSet::new();
        let mut decoder = Decoder::new(any.as_bytes());
        let mut last_value = None;

        while !decoder.is_finished() {
            let value = decoder.decode()?;

            if let Some(last) = last_value.take() {
                if last >= value {
                    return Err(ErrorKind::Noncanonical.into());
                }

                result.insert(last);
            }

            last_value = Some(value);
        }

        if let Some(last) = last_value {
            result.insert(last);
        }

        Ok(result)
    }
}

#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
impl<'a, T> Encodable for BTreeSet<T>
where
    T: Clone + Decodable<'a> + Encodable + Ord,
{
    fn encoded_len(&self) -> Result<Length> {
        btreeset_inner_len(self)?.for_tlv()
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<()> {
        Header::new(Self::TAG, btreeset_inner_len(self)?)?.encode(encoder)?;

        for value in self.iter() {
            encoder.encode(value)?;
        }

        Ok(())
    }
}

#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
impl<'a, 'b, T: 'b> SetOf<'a, 'b, T> for BTreeSet<T>
where
    T: Clone + Decodable<'a> + Encodable + Ord,
{
    type Iter = core::iter::Cloned<btree_set::Iter<'b, T>>;

    fn elements(&'b self) -> Self::Iter {
        self.iter().cloned()
    }
}

#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
impl<'a, T> Tagged for BTreeSet<T>
where
    T: Clone + Decodable<'a> + Encodable + Ord,
{
    const TAG: Tag = Tag::Set;
}

/// Get the encoded length of a [`BTreeSet`]
#[cfg(feature = "alloc")]
fn btreeset_inner_len<'a, T>(set: &BTreeSet<T>) -> Result<Length>
where
    T: Clone + Decodable<'a> + Encodable + Ord,
{
    set.iter()
        .fold(Ok(Length::ZERO), |acc, val| acc? + val.encoded_len()?)
}
