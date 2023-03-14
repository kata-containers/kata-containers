use crate::{ASN1Parser, BerParser, DerParser, Error, FromBer, FromDer};
use core::marker::PhantomData;

/// An Iterator over binary data, parsing elements of type `T`
///
/// This helps parsing `SEQUENCE OF` items of type `T`. The type of parser
/// (BER/DER) is specified using the generic parameter `F` of this struct.
///
/// Note: the iterator must start on the sequence *contents*, not the sequence itself.
///
/// # Examples
///
/// ```rust
/// use asn1_rs::{DerParser, Integer, SequenceIterator};
///
/// let data = &[0x30, 0x6, 0x2, 0x1, 0x1, 0x2, 0x1, 0x2];
/// for (idx, item) in SequenceIterator::<Integer, DerParser>::new(&data[2..]).enumerate() {
///     let item = item.unwrap(); // parsing could have failed
///     let i = item.as_u32().unwrap(); // integer can be negative, or too large to fit into u32
///     assert_eq!(i as usize, idx + 1);
/// }
/// ```
#[derive(Debug)]
pub struct SequenceIterator<'a, T, F, E = Error>
where
    F: ASN1Parser,
{
    data: &'a [u8],
    has_error: bool,
    _t: PhantomData<T>,
    _f: PhantomData<F>,
    _e: PhantomData<E>,
}

impl<'a, T, F, E> SequenceIterator<'a, T, F, E>
where
    F: ASN1Parser,
{
    pub fn new(data: &'a [u8]) -> Self {
        SequenceIterator {
            data,
            has_error: false,
            _t: PhantomData,
            _f: PhantomData,
            _e: PhantomData,
        }
    }
}

impl<'a, T, E> Iterator for SequenceIterator<'a, T, BerParser, E>
where
    T: FromBer<'a, E>,
    E: From<Error>,
{
    type Item = Result<T, E>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.has_error || self.data.is_empty() {
            return None;
        }
        match T::from_ber(self.data) {
            Ok((rem, obj)) => {
                self.data = rem;
                Some(Ok(obj))
            }
            Err(nom::Err::Error(e)) | Err(nom::Err::Failure(e)) => {
                self.has_error = true;
                Some(Err(e))
            }

            Err(nom::Err::Incomplete(n)) => {
                self.has_error = true;
                Some(Err(Error::Incomplete(n).into()))
            }
        }
    }
}

impl<'a, T, E> Iterator for SequenceIterator<'a, T, DerParser, E>
where
    T: FromDer<'a, E>,
    E: From<Error>,
{
    type Item = Result<T, E>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.has_error || self.data.is_empty() {
            return None;
        }
        match T::from_der(self.data) {
            Ok((rem, obj)) => {
                self.data = rem;
                Some(Ok(obj))
            }
            Err(nom::Err::Error(e)) | Err(nom::Err::Failure(e)) => {
                self.has_error = true;
                Some(Err(e))
            }

            Err(nom::Err::Incomplete(n)) => {
                self.has_error = true;
                Some(Err(Error::Incomplete(n).into()))
            }
        }
    }
}
