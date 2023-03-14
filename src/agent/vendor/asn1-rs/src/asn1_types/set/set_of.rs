use crate::*;
use alloc::vec::Vec;
use core::convert::TryFrom;
use core::iter::FromIterator;

/// The `SET OF` object is an unordered list of homogeneous types.
///
/// # Examples
///
/// ```
/// use asn1_rs::SetOf;
/// use std::iter::FromIterator;
///
/// // build set
/// let it = [2, 3, 4].iter();
/// let set = SetOf::from_iter(it);
///
/// // `set` now contains the serialized DER representation of the array
///
/// // iterate objects
/// let mut sum = 0;
/// for item in set.iter() {
///     // item has type `Result<u32>`, since parsing the serialized bytes could fail
///     sum += *item;
/// }
/// assert_eq!(sum, 9);
///
/// ```
#[derive(Debug)]
pub struct SetOf<T> {
    items: Vec<T>,
}

impl<T> SetOf<T> {
    /// Builds a `SET OF` from the provided content
    #[inline]
    pub const fn new(items: Vec<T>) -> Self {
        SetOf { items }
    }

    /// Returns the length of this `SET` (the number of items).
    #[inline]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` if this `SET` is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns an iterator over the items of the `SET`.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter()
    }
}

impl<'a, T> AsRef<[T]> for SetOf<T> {
    fn as_ref(&self) -> &[T] {
        &self.items
    }
}

impl<'a, T> IntoIterator for &'a SetOf<T> {
    type Item = &'a T;
    type IntoIter = core::slice::Iter<'a, T>;

    fn into_iter(self) -> core::slice::Iter<'a, T> {
        self.items.iter()
    }
}

impl<'a, T> IntoIterator for &'a mut SetOf<T> {
    type Item = &'a mut T;
    type IntoIter = core::slice::IterMut<'a, T>;

    fn into_iter(self) -> core::slice::IterMut<'a, T> {
        self.items.iter_mut()
    }
}

impl<T> From<SetOf<T>> for Vec<T> {
    fn from(set: SetOf<T>) -> Self {
        set.items
    }
}

impl<T> FromIterator<T> for SetOf<T> {
    fn from_iter<IT: IntoIterator<Item = T>>(iter: IT) -> Self {
        let items = iter.into_iter().collect();
        SetOf::new(items)
    }
}

impl<'a, T> TryFrom<Any<'a>> for SetOf<T>
where
    T: FromBer<'a>,
{
    type Error = Error;

    fn try_from(any: Any<'a>) -> Result<Self> {
        any.tag().assert_eq(Self::TAG)?;
        if !any.header.is_constructed() {
            return Err(Error::ConstructExpected);
        }
        let items = SetIterator::<T, BerParser>::new(any.data).collect::<Result<Vec<T>>>()?;
        Ok(SetOf::new(items))
    }
}

impl<T> CheckDerConstraints for SetOf<T>
where
    T: CheckDerConstraints,
{
    fn check_constraints(any: &Any) -> Result<()> {
        any.tag().assert_eq(Self::TAG)?;
        any.header.assert_constructed()?;
        for item in SetIterator::<Any, DerParser>::new(any.data) {
            let item = item?;
            T::check_constraints(&item)?;
        }
        Ok(())
    }
}

impl<T> DerAutoDerive for SetOf<T> {}

impl<T> Tagged for SetOf<T> {
    const TAG: Tag = Tag::Set;
}

#[cfg(feature = "std")]
impl<T> ToDer for SetOf<T>
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
