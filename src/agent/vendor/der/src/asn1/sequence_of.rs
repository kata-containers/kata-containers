//! ASN.1 `SEQUENCE OF` support.

use crate::{
    arrayvec, ArrayVec, Decodable, DecodeValue, Decoder, DerOrd, Encodable, EncodeValue, Encoder,
    ErrorKind, FixedTag, Length, Result, Tag, ValueOrd,
};
use core::cmp::Ordering;

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

/// ASN.1 `SEQUENCE OF` backed by an array.
///
/// This type implements an append-only `SEQUENCE OF` type which is stack-based
/// and does not depend on `alloc` support.
// TODO(tarcieri): use `ArrayVec` when/if it's merged into `core`
// See: https://github.com/rust-lang/rfcs/pull/2990
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SequenceOf<T, const N: usize> {
    inner: ArrayVec<T, N>,
}

impl<T, const N: usize> SequenceOf<T, N> {
    /// Create a new [`SequenceOf`].
    pub fn new() -> Self {
        Self {
            inner: ArrayVec::new(),
        }
    }

    /// Add an element to this [`SequenceOf`].
    pub fn add(&mut self, element: T) -> Result<()> {
        self.inner.add(element)
    }

    /// Get an element of this [`SequenceOf`].
    pub fn get(&self, index: usize) -> Option<&T> {
        self.inner.get(index)
    }

    /// Iterate over the elements in this [`SequenceOf`].
    pub fn iter(&self) -> SequenceOfIter<'_, T> {
        SequenceOfIter {
            inner: self.inner.iter(),
        }
    }

    /// Is this [`SequenceOf`] empty?
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Number of elements in this [`SequenceOf`].
    pub fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<T, const N: usize> Default for SequenceOf<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, T, const N: usize> DecodeValue<'a> for SequenceOf<T, N>
where
    T: Decodable<'a>,
{
    fn decode_value(decoder: &mut Decoder<'a>, length: Length) -> Result<Self> {
        let end_pos = (decoder.position() + length)?;
        let mut sequence_of = Self::new();

        while decoder.position() < end_pos {
            sequence_of.add(decoder.decode()?)?;
        }

        if decoder.position() != end_pos {
            decoder.error(ErrorKind::Length { tag: Self::TAG });
        }

        Ok(sequence_of)
    }
}

impl<T, const N: usize> EncodeValue for SequenceOf<T, N>
where
    T: Encodable,
{
    fn value_len(&self) -> Result<Length> {
        self.iter()
            .fold(Ok(Length::ZERO), |len, elem| len + elem.encoded_len()?)
    }

    fn encode_value(&self, encoder: &mut Encoder<'_>) -> Result<()> {
        for elem in self.iter() {
            elem.encode(encoder)?;
        }

        Ok(())
    }
}

impl<T, const N: usize> FixedTag for SequenceOf<T, N> {
    const TAG: Tag = Tag::Sequence;
}

impl<T, const N: usize> ValueOrd for SequenceOf<T, N>
where
    T: DerOrd,
{
    fn value_cmp(&self, other: &Self) -> Result<Ordering> {
        value_cmp(self.iter(), other.iter())
    }
}

/// Iterator over the elements of an [`SequenceOf`].
#[derive(Clone, Debug)]
pub struct SequenceOfIter<'a, T> {
    /// Inner iterator.
    inner: arrayvec::Iter<'a, T>,
}

impl<'a, T> Iterator for SequenceOfIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        self.inner.next()
    }
}

impl<'a, T, const N: usize> DecodeValue<'a> for [T; N]
where
    T: Decodable<'a>,
{
    fn decode_value(decoder: &mut Decoder<'a>, length: Length) -> Result<Self> {
        SequenceOf::decode_value(decoder, length)?
            .inner
            .try_into_array()
    }
}

impl<T, const N: usize> EncodeValue for [T; N]
where
    T: Encodable,
{
    fn value_len(&self) -> Result<Length> {
        self.iter()
            .fold(Ok(Length::ZERO), |len, elem| len + elem.encoded_len()?)
    }

    fn encode_value(&self, encoder: &mut Encoder<'_>) -> Result<()> {
        for elem in self {
            elem.encode(encoder)?;
        }

        Ok(())
    }
}

impl<T, const N: usize> FixedTag for [T; N] {
    const TAG: Tag = Tag::Sequence;
}

impl<T, const N: usize> ValueOrd for [T; N]
where
    T: DerOrd,
{
    fn value_cmp(&self, other: &Self) -> Result<Ordering> {
        value_cmp(self.iter(), other.iter())
    }
}

#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
impl<'a, T> DecodeValue<'a> for Vec<T>
where
    T: Decodable<'a>,
{
    fn decode_value(decoder: &mut Decoder<'a>, length: Length) -> Result<Self> {
        let end_pos = (decoder.position() + length)?;
        let mut sequence_of = Self::new();

        while decoder.position() < end_pos {
            sequence_of.push(decoder.decode()?);
        }

        if decoder.position() != end_pos {
            decoder.error(ErrorKind::Length { tag: Self::TAG });
        }

        Ok(sequence_of)
    }
}

#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
impl<T> EncodeValue for Vec<T>
where
    T: Encodable,
{
    fn value_len(&self) -> Result<Length> {
        self.iter()
            .fold(Ok(Length::ZERO), |len, elem| len + elem.encoded_len()?)
    }

    fn encode_value(&self, encoder: &mut Encoder<'_>) -> Result<()> {
        for elem in self {
            elem.encode(encoder)?;
        }

        Ok(())
    }
}

#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
impl<T> FixedTag for Vec<T> {
    const TAG: Tag = Tag::Sequence;
}

#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
impl<T> ValueOrd for Vec<T>
where
    T: DerOrd,
{
    fn value_cmp(&self, other: &Self) -> Result<Ordering> {
        value_cmp(self.iter(), other.iter())
    }
}

/// Compare two `SEQUENCE OF`s by value.
fn value_cmp<'a, I, T: 'a>(a: I, b: I) -> Result<Ordering>
where
    I: Iterator<Item = &'a T>,
    T: DerOrd,
{
    for (value1, value2) in a.zip(b) {
        match value1.der_cmp(value2)? {
            Ordering::Equal => (),
            other => return Ok(other),
        }
    }

    Ok(Ordering::Equal)
}
