use crate::*;
use alloc::borrow::Cow;
use alloc::vec::Vec;
use core::convert::TryFrom;

mod iterator;
mod sequence_of;
mod vec;

pub use iterator::*;
pub use sequence_of::*;
pub use vec::*;

/// The `SEQUENCE` object is an ordered list of heteregeneous types.
///
/// Sequences can usually be of 2 types:
/// - a list of different objects (`SEQUENCE`, usually parsed as a `struct`)
/// - a list of similar objects (`SEQUENCE OF`, usually parsed as a `Vec<T>`)
///
/// The current object covers the former. For the latter, see the [`SequenceOf`] documentation.
///
/// The `Sequence` object contains the (*unparsed*) encoded representation of its content. It provides
/// methods to parse and iterate contained objects, or convert the sequence to other types.
///
/// # Building a Sequence
///
/// To build a DER sequence:
/// - if the sequence is composed of objects of the same type, the [`Sequence::from_iter_to_der`] method can be used
/// - otherwise, the [`ToDer`] trait can be used to create content incrementally
///
/// ```
/// use asn1_rs::{Integer, Sequence, SerializeResult, ToDer};
///
/// fn build_seq<'a>() -> SerializeResult<Sequence<'a>> {
///     let mut v = Vec::new();
///     // add an Integer object (construct type):
///     let i = Integer::from_u32(4);
///     let _ = i.write_der(&mut v)?;
///     // some primitive objects also implement `ToDer`. A string will be mapped as `Utf8String`:
///     let _ = "abcd".write_der(&mut v)?;
///     // return the sequence built from the DER content
///     Ok(Sequence::new(v.into()))
/// }
///
/// let seq = build_seq().unwrap();
///
/// ```
///
/// # Examples
///
/// ```
/// use asn1_rs::{Error, Sequence};
///
/// // build sequence
/// let it = [2, 3, 4].iter();
/// let seq = Sequence::from_iter_to_der(it).unwrap();
///
/// // `seq` now contains the serialized DER representation of the array
///
/// // iterate objects
/// let mut sum = 0;
/// for item in seq.der_iter::<u32, Error>() {
///     // item has type `Result<u32>`, since parsing the serialized bytes could fail
///     sum += item.expect("parsing list item failed");
/// }
/// assert_eq!(sum, 9);
///
/// ```
///
/// Note: the above example encodes a `SEQUENCE OF INTEGER` object, the [`SequenceOf`] object could
/// be used to provide a simpler API.
///
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sequence<'a> {
    /// Serialized DER representation of the sequence content
    pub content: Cow<'a, [u8]>,
}

impl<'a> Sequence<'a> {
    /// Build a sequence, given the provided content
    pub const fn new(content: Cow<'a, [u8]>) -> Self {
        Sequence { content }
    }

    /// Consume the sequence and return the content
    #[inline]
    pub fn into_content(self) -> Cow<'a, [u8]> {
        self.content
    }

    /// Apply the parsing function to the sequence content, consuming the sequence
    ///
    /// Note: this function expects the caller to take ownership of content.
    /// In some cases, handling the lifetime of objects is not easy (when keeping only references on
    /// data). Other methods are provided (depending on the use case):
    /// - [`Sequence::parse`] takes a reference on the sequence data, but does not consume it,
    /// - [`Sequence::from_der_and_then`] does the parsing of the sequence and applying the function
    ///   in one step, ensuring there are only references (and dropping the temporary sequence).
    pub fn and_then<U, F, E>(self, op: F) -> ParseResult<'a, U, E>
    where
        F: FnOnce(Cow<'a, [u8]>) -> ParseResult<U, E>,
    {
        op(self.content)
    }

    /// Same as [`Sequence::from_der_and_then`], but using BER encoding (no constraints).
    pub fn from_ber_and_then<U, F, E>(bytes: &'a [u8], op: F) -> ParseResult<'a, U, E>
    where
        F: FnOnce(&'a [u8]) -> ParseResult<U, E>,
        E: From<Error>,
    {
        let (rem, seq) = Sequence::from_ber(bytes).map_err(Err::convert)?;
        let data = match seq.content {
            Cow::Borrowed(b) => b,
            // Since 'any' is built from 'bytes', it is borrowed by construction
            Cow::Owned(_) => unreachable!(),
        };
        let (_, res) = op(data)?;
        Ok((rem, res))
    }

    /// Parse a DER sequence and apply the provided parsing function to content
    ///
    /// After parsing, the sequence object and header are discarded.
    ///
    /// ```
    /// use asn1_rs::{FromDer, ParseResult, Sequence};
    ///
    /// // Parse a SEQUENCE {
    /// //      a INTEGER (0..255),
    /// //      b INTEGER (0..4294967296)
    /// // }
    /// // and return only `(a,b)
    /// fn parser(i: &[u8]) -> ParseResult<(u8, u32)> {
    ///     Sequence::from_der_and_then(i, |i| {
    ///             let (i, a) = u8::from_der(i)?;
    ///             let (i, b) = u32::from_der(i)?;
    ///             Ok((i, (a, b)))
    ///         }
    ///     )
    /// }
    /// ```
    pub fn from_der_and_then<U, F, E>(bytes: &'a [u8], op: F) -> ParseResult<'a, U, E>
    where
        F: FnOnce(&'a [u8]) -> ParseResult<U, E>,
        E: From<Error>,
    {
        let (rem, seq) = Sequence::from_der(bytes).map_err(Err::convert)?;
        let data = match seq.content {
            Cow::Borrowed(b) => b,
            // Since 'any' is built from 'bytes', it is borrowed by construction
            Cow::Owned(_) => unreachable!(),
        };
        let (_, res) = op(data)?;
        Ok((rem, res))
    }

    /// Apply the parsing function to the sequence content (non-consuming version)
    pub fn parse<F, T, E>(&'a self, mut f: F) -> ParseResult<'a, T, E>
    where
        F: FnMut(&'a [u8]) -> ParseResult<'a, T, E>,
    {
        let input: &[u8] = &self.content;
        f(input)
    }

    /// Apply the parsing function to the sequence content (consuming version)
    ///
    /// Note: to parse and apply a parsing function in one step, use the
    /// [`Sequence::from_der_and_then`] method.
    ///
    /// # Limitations
    ///
    /// This function fails if the sequence contains `Owned` data, because the parsing function
    /// takes a reference on data (which is dropped).
    pub fn parse_into<F, T, E>(self, mut f: F) -> ParseResult<'a, T, E>
    where
        F: FnMut(&'a [u8]) -> ParseResult<'a, T, E>,
        E: From<Error>,
    {
        match self.content {
            Cow::Borrowed(b) => f(b),
            _ => Err(nom::Err::Error(Error::LifetimeError.into())),
        }
    }

    /// Return an iterator over the sequence content, attempting to decode objects as BER
    ///
    /// This method can be used when all objects from the sequence have the same type.
    pub fn ber_iter<T, E>(&'a self) -> SequenceIterator<'a, T, BerParser, E>
    where
        T: FromBer<'a, E>,
    {
        SequenceIterator::new(&self.content)
    }

    /// Return an iterator over the sequence content, attempting to decode objects as DER
    ///
    /// This method can be used when all objects from the sequence have the same type.
    pub fn der_iter<T, E>(&'a self) -> SequenceIterator<'a, T, DerParser, E>
    where
        T: FromDer<'a, E>,
    {
        SequenceIterator::new(&self.content)
    }

    /// Attempt to parse the sequence as a `SEQUENCE OF` items (BER), and return the parsed items as a `Vec`.
    pub fn ber_sequence_of<T, E>(&'a self) -> Result<Vec<T>, E>
    where
        T: FromBer<'a, E>,
        E: From<Error>,
    {
        self.ber_iter().collect()
    }

    /// Attempt to parse the sequence as a `SEQUENCE OF` items (DER), and return the parsed items as a `Vec`.
    pub fn der_sequence_of<T, E>(&'a self) -> Result<Vec<T>, E>
    where
        T: FromDer<'a, E>,
        E: From<Error>,
    {
        self.der_iter().collect()
    }

    /// Attempt to parse the sequence as a `SEQUENCE OF` items (BER) (consuming input),
    /// and return the parsed items as a `Vec`.
    ///
    /// Note: if `Self` is an `Owned` object, the data will be duplicated (causing allocations) into separate objects.
    pub fn into_ber_sequence_of<T, U, E>(self) -> Result<Vec<T>, E>
    where
        for<'b> T: FromBer<'b, E>,
        E: From<Error>,
        T: ToStatic<Owned = T>,
    {
        match self.content {
            Cow::Borrowed(bytes) => SequenceIterator::<T, BerParser, E>::new(bytes).collect(),
            Cow::Owned(data) => {
                let v1 = SequenceIterator::<T, BerParser, E>::new(&data)
                    .collect::<Result<Vec<T>, E>>()?;
                let v2 = v1.iter().map(|t| t.to_static()).collect::<Vec<_>>();
                Ok(v2)
            }
        }
    }

    /// Attempt to parse the sequence as a `SEQUENCE OF` items (DER) (consuming input),
    /// and return the parsed items as a `Vec`.
    ///
    /// Note: if `Self` is an `Owned` object, the data will be duplicated (causing allocations) into separate objects.
    pub fn into_der_sequence_of<T, U, E>(self) -> Result<Vec<T>, E>
    where
        for<'b> T: FromDer<'b, E>,
        E: From<Error>,
        T: ToStatic<Owned = T>,
    {
        match self.content {
            Cow::Borrowed(bytes) => SequenceIterator::<T, DerParser, E>::new(bytes).collect(),
            Cow::Owned(data) => {
                let v1 = SequenceIterator::<T, DerParser, E>::new(&data)
                    .collect::<Result<Vec<T>, E>>()?;
                let v2 = v1.iter().map(|t| t.to_static()).collect::<Vec<_>>();
                Ok(v2)
            }
        }
    }

    pub fn into_der_sequence_of_ref<T, E>(self) -> Result<Vec<T>, E>
    where
        T: FromDer<'a, E>,
        E: From<Error>,
    {
        match self.content {
            Cow::Borrowed(bytes) => SequenceIterator::<T, DerParser, E>::new(bytes).collect(),
            Cow::Owned(_) => Err(Error::LifetimeError.into()),
        }
    }
}

impl<'a> ToStatic for Sequence<'a> {
    type Owned = Sequence<'static>;

    fn to_static(&self) -> Self::Owned {
        Sequence {
            content: Cow::Owned(self.content.to_vec()),
        }
    }
}

impl<'a, T, U> ToStatic for Vec<T>
where
    T: ToStatic<Owned = U>,
    U: 'static,
{
    type Owned = Vec<U>;

    fn to_static(&self) -> Self::Owned {
        self.iter().map(|t| t.to_static()).collect()
    }
}

impl<'a> AsRef<[u8]> for Sequence<'a> {
    fn as_ref(&self) -> &[u8] {
        &self.content
    }
}

impl<'a> TryFrom<Any<'a>> for Sequence<'a> {
    type Error = Error;

    fn try_from(any: Any<'a>) -> Result<Sequence<'a>> {
        TryFrom::try_from(&any)
    }
}

impl<'a, 'b> TryFrom<&'b Any<'a>> for Sequence<'a> {
    type Error = Error;

    fn try_from(any: &'b Any<'a>) -> Result<Sequence<'a>> {
        any.tag().assert_eq(Self::TAG)?;
        any.header.assert_constructed()?;
        Ok(Sequence {
            content: Cow::Borrowed(any.data),
        })
    }
}

impl<'a> CheckDerConstraints for Sequence<'a> {
    fn check_constraints(_any: &Any) -> Result<()> {
        // TODO: iterate on ANY objects and check constraints? -> this will not be exhaustive
        // test, for ex INTEGER encoding will not be checked
        Ok(())
    }
}

impl<'a> DerAutoDerive for Sequence<'a> {}

impl<'a> Tagged for Sequence<'a> {
    const TAG: Tag = Tag::Sequence;
}

#[cfg(feature = "std")]
impl ToDer for Sequence<'_> {
    fn to_der_len(&self) -> Result<usize> {
        let sz = self.content.len();
        if sz < 127 {
            // 1 (class+tag) + 1 (length) + len
            Ok(2 + sz)
        } else {
            // 1 (class+tag) + n (length) + len
            let n = Length::Definite(sz).to_der_len()?;
            Ok(1 + n + sz)
        }
    }

    fn write_der_header(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        let header = Header::new(
            Class::Universal,
            true,
            Self::TAG,
            Length::Definite(self.content.len()),
        );
        header.write_der_header(writer).map_err(Into::into)
    }

    fn write_der_content(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        writer.write(&self.content).map_err(Into::into)
    }
}

#[cfg(feature = "std")]
impl<'a> Sequence<'a> {
    /// Attempt to create a `Sequence` from an iterator over serializable objects (to DER)
    ///
    /// # Examples
    ///
    /// ```
    /// use asn1_rs::Sequence;
    ///
    /// // build sequence
    /// let it = [2, 3, 4].iter();
    /// let seq = Sequence::from_iter_to_der(it).unwrap();
    /// ```
    pub fn from_iter_to_der<T, IT>(it: IT) -> SerializeResult<Self>
    where
        IT: Iterator<Item = T>,
        T: ToDer,
        T: Tagged,
    {
        let mut v = Vec::new();
        for item in it {
            let item_v = <T as ToDer>::to_der_vec(&item)?;
            v.extend_from_slice(&item_v);
        }
        Ok(Sequence {
            content: Cow::Owned(v),
        })
    }
}
