use crate::*;
use alloc::vec::Vec;
use core::convert::TryFrom;
use core::iter::FromIterator;

/// The `SEQUENCE OF` object is an ordered list of homogeneous types.
///
/// # Examples
///
/// ```
/// use asn1_rs::SequenceOf;
/// use std::iter::FromIterator;
///
/// // build set
/// let it = [2, 3, 4].iter();
/// let seq = SequenceOf::from_iter(it);
///
/// // `seq` now contains the serialized DER representation of the array
///
/// // iterate objects
/// let mut sum = 0;
/// for item in seq.iter() {
///     // item has type `Result<u32>`, since parsing the serialized bytes could fail
///     sum += *item;
/// }
/// assert_eq!(sum, 9);
///
/// ```
#[derive(Debug)]
pub struct SequenceOf<T> {
    pub(crate) items: Vec<T>,
}

impl<T> SequenceOf<T> {
    /// Builds a `SEQUENCE OF` from the provided content
    #[inline]
    pub const fn new(items: Vec<T>) -> Self {
        SequenceOf { items }
    }

    /// Returns the length of this `SEQUENCE` (the number of items).
    #[inline]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` if this `SEQUENCE` is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns an iterator over the items of the `SEQUENCE`.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter()
    }
}

impl<'a, T> AsRef<[T]> for SequenceOf<T> {
    fn as_ref(&self) -> &[T] {
        &self.items
    }
}

impl<'a, T> IntoIterator for &'a SequenceOf<T> {
    type Item = &'a T;
    type IntoIter = core::slice::Iter<'a, T>;

    fn into_iter(self) -> core::slice::Iter<'a, T> {
        self.items.iter()
    }
}

impl<'a, T> IntoIterator for &'a mut SequenceOf<T> {
    type Item = &'a mut T;
    type IntoIter = core::slice::IterMut<'a, T>;

    fn into_iter(self) -> core::slice::IterMut<'a, T> {
        self.items.iter_mut()
    }
}

impl<T> From<SequenceOf<T>> for Vec<T> {
    fn from(set: SequenceOf<T>) -> Self {
        set.items
    }
}

impl<T> FromIterator<T> for SequenceOf<T> {
    fn from_iter<IT: IntoIterator<Item = T>>(iter: IT) -> Self {
        let items = iter.into_iter().collect();
        SequenceOf::new(items)
    }
}

impl<'a, T> TryFrom<Any<'a>> for SequenceOf<T>
where
    T: FromBer<'a>,
{
    type Error = Error;

    fn try_from(any: Any<'a>) -> Result<Self> {
        any.tag().assert_eq(Self::TAG)?;
        if !any.header.is_constructed() {
            return Err(Error::ConstructExpected);
        }
        let items = SequenceIterator::<T, BerParser>::new(any.data).collect::<Result<Vec<T>>>()?;
        Ok(SequenceOf::new(items))
    }
}

impl<T> CheckDerConstraints for SequenceOf<T>
where
    T: CheckDerConstraints,
{
    fn check_constraints(any: &Any) -> Result<()> {
        any.tag().assert_eq(Self::TAG)?;
        any.header.assert_constructed()?;
        for item in SequenceIterator::<Any, DerParser>::new(any.data) {
            let item = item?;
            T::check_constraints(&item)?;
        }
        Ok(())
    }
}

impl<T> DerAutoDerive for SequenceOf<T> {}

impl<T> Tagged for SequenceOf<T> {
    const TAG: Tag = Tag::Sequence;
}

#[cfg(feature = "std")]
impl<T> ToDer for SequenceOf<T>
where
    T: ToDer,
{
    fn to_der_len(&self) -> Result<usize> {
        self.items.to_der_len()
    }

    fn write_der_header(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        self.items.write_der_header(writer)
    }

    fn write_der_content(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        self.items.write_der_content(writer)
    }
}
