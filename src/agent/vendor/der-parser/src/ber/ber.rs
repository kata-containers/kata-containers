use super::{Class, Header, Length, Tag};
use crate::ber::bitstring_to_u64;
use crate::ber::integer::*;
use crate::error::BerError;
use crate::oid::Oid;
use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::vec::Vec;
use asn1_rs::ASN1DateTime;
use asn1_rs::Any;
#[cfg(feature = "bitvec")]
use bitvec::{order::Msb0, slice::BitSlice};
use core::convert::AsRef;
use core::convert::From;
use core::ops::Index;

/// Representation of a BER-encoded (X.690) object
///
/// A BER object is composed of a header describing the object class, type and length,
/// and the content.
///
/// Note that the content may sometimes not match the header tag (for ex when parsing IMPLICIT
/// tagged values).
#[derive(Debug, Clone, PartialEq)]
pub struct BerObject<'a> {
    pub header: Header<'a>,
    pub content: BerObjectContent<'a>,
}

/// BER object content
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::upper_case_acronyms)]
pub enum BerObjectContent<'a> {
    /// EOC (no content)
    EndOfContent,
    /// BOOLEAN: decoded value
    Boolean(bool),
    /// INTEGER: raw bytes
    ///
    /// Note: the reason to store the raw bytes is that integers have non-finite length in the
    /// spec, and also that the raw encoding is also important for some applications.
    ///
    /// To extract the number, see the `as_u64`, `as_u32`, `as_bigint` and `as_biguint` methods.
    Integer(&'a [u8]),
    /// BIT STRING: number of unused bits, and object
    BitString(u8, BitStringObject<'a>),
    /// OCTET STRING: slice
    OctetString(&'a [u8]),
    /// NULL (no content)
    Null,
    /// ENUMERATED: decoded enum number
    Enum(u64),
    /// OID
    OID(Oid<'a>),
    /// RELATIVE OID
    RelativeOID(Oid<'a>),
    /// NumericString: decoded string
    NumericString(&'a str),
    /// VisibleString: decoded string
    VisibleString(&'a str),
    /// PrintableString: decoded string
    PrintableString(&'a str),
    /// IA5String: decoded string
    IA5String(&'a str),
    /// UTF8String: decoded string
    UTF8String(&'a str),
    /// T61String: decoded string
    T61String(&'a str),
    /// VideotexString: decoded string
    VideotexString(&'a str),

    /// BmpString: decoded string
    BmpString(&'a str),
    /// UniversalString: raw object bytes
    UniversalString(&'a [u8]),

    /// SEQUENCE: list of objects
    Sequence(Vec<BerObject<'a>>),
    /// SET: list of objects
    Set(Vec<BerObject<'a>>),

    /// UTCTime: decoded string
    UTCTime(ASN1DateTime),
    /// GeneralizedTime: decoded string
    GeneralizedTime(ASN1DateTime),

    /// Object descriptor: decoded string
    ObjectDescriptor(&'a str),
    /// GraphicString: decoded string
    GraphicString(&'a str),
    /// GeneralString: decoded string
    GeneralString(&'a str),

    /// Optional object
    Optional(Option<Box<BerObject<'a>>>),
    /// Tagged object (EXPLICIT): class, tag  and content of inner object
    Tagged(Class, Tag, Box<BerObject<'a>>),

    /// Private or Unknown (for ex. unknown tag) object
    Unknown(Any<'a>),
}

impl<'a> BerObject<'a> {
    /// Build a BerObject from a header and content.
    ///
    /// Note: values are not checked, so the tag can be different from the real content, or flags
    /// can be invalid.
    #[inline]
    pub const fn from_header_and_content<'o>(
        header: Header<'o>,
        content: BerObjectContent<'o>,
    ) -> BerObject<'o> {
        BerObject { header, content }
    }

    /// Build a BerObject from its content, using default flags (no class, correct tag,
    /// and constructed flag set only for Set and Sequence)
    pub const fn from_obj(c: BerObjectContent) -> BerObject {
        let class = Class::Universal;
        let tag = c.tag();
        let constructed = matches!(tag, Tag::Sequence | Tag::Set);
        let header = Header::new(class, constructed, tag, Length::Definite(0));
        BerObject { header, content: c }
    }

    /// Build a DER integer object from a slice containing an encoded integer
    pub const fn from_int_slice(i: &'a [u8]) -> BerObject<'a> {
        let header = Header::new(Class::Universal, false, Tag::Integer, Length::Definite(0));
        BerObject {
            header,
            content: BerObjectContent::Integer(i),
        }
    }

    /// Set a tag for the BER object
    pub fn set_raw_tag(self, raw_tag: Option<&'a [u8]>) -> BerObject {
        let header = self.header.with_raw_tag(raw_tag.map(|x| x.into()));
        BerObject { header, ..self }
    }

    /// Build a DER sequence object from a vector of DER objects
    pub const fn from_seq(l: Vec<BerObject>) -> BerObject {
        BerObject::from_obj(BerObjectContent::Sequence(l))
    }

    /// Build a DER set object from a vector of DER objects
    pub const fn from_set(l: Vec<BerObject>) -> BerObject {
        BerObject::from_obj(BerObjectContent::Set(l))
    }

    /// Attempt to read a signed integer value from DER object.
    ///
    /// This can fail if the object is not an integer, or if it is too large.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use der_parser::ber::BerObject;
    /// let der_int  = BerObject::from_int_slice(b"\x80");
    /// assert_eq!(
    ///     der_int.as_i64(),
    ///     Ok(-128)
    /// );
    /// ```
    pub fn as_i64(&self) -> Result<i64, BerError> {
        self.content.as_i64()
    }

    /// Attempt to read a signed integer value from DER object.
    ///
    /// This can fail if the object is not an integer, or if it is too large.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use der_parser::ber::BerObject;
    /// let der_int  = BerObject::from_int_slice(b"\x80");
    /// assert_eq!(
    ///     der_int.as_i32(),
    ///     Ok(-128)
    /// );
    /// ```
    pub fn as_i32(&self) -> Result<i32, BerError> {
        self.content.as_i32()
    }

    /// Attempt to read integer value from DER object.
    ///
    /// This can fail if the object is not an unsigned integer, or if it is too large.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use der_parser::ber::BerObject;
    /// let der_int  = BerObject::from_int_slice(b"\x01\x00\x01");
    /// assert_eq!(
    ///     der_int.as_u64(),
    ///     Ok(0x10001)
    /// );
    /// ```
    pub fn as_u64(&self) -> Result<u64, BerError> {
        self.content.as_u64()
    }

    /// Attempt to read integer value from DER object.
    ///
    /// This can fail if the object is not an unsigned integer, or if it is too large.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # extern crate der_parser;
    /// # use der_parser::ber::{BerObject,BerObjectContent};
    /// let der_int  = BerObject::from_obj(BerObjectContent::Integer(b"\x01\x00\x01"));
    /// assert_eq!(
    ///     der_int.as_u32(),
    ///     Ok(0x10001)
    /// );
    /// ```
    pub fn as_u32(&self) -> Result<u32, BerError> {
        self.content.as_u32()
    }

    /// Attempt to read integer value from DER object.
    /// This can fail if the object is not a boolean.
    pub fn as_bool(&self) -> Result<bool, BerError> {
        self.content.as_bool()
    }

    /// Attempt to read an OID value from DER object.
    /// This can fail if the object is not an OID.
    pub fn as_oid(&self) -> Result<&Oid<'a>, BerError> {
        self.content.as_oid()
    }

    /// Attempt to read an OID value from DER object.
    /// This can fail if the object is not an OID.
    pub fn as_oid_val(&self) -> Result<Oid<'a>, BerError> {
        self.content.as_oid_val()
    }

    /// Attempt to get a reference on the content from an optional object.
    /// This can fail if the object is not optional.
    pub fn as_optional(&self) -> Result<Option<&BerObject<'a>>, BerError> {
        self.content.as_optional()
    }

    /// Attempt to get a reference on the content from a tagged object.
    /// This can fail if the object is not tagged.
    pub fn as_tagged(&self) -> Result<(Class, Tag, &BerObject<'a>), BerError> {
        self.content.as_tagged()
    }

    /// Attempt to read a reference to a BitString value from DER object.
    /// This can fail if the object is not an BitString.
    ///
    /// Note that this function returns a reference to the BitString. To get an owned value,
    /// use [`as_bitstring`](struct.BerObject.html#method.as_bitstring)
    pub fn as_bitstring_ref(&self) -> Result<&BitStringObject, BerError> {
        self.content.as_bitstring_ref()
    }

    /// Attempt to read a BitString value from DER object.
    /// This can fail if the object is not an BitString.
    pub fn as_bitstring(&self) -> Result<BitStringObject<'a>, BerError> {
        self.content.as_bitstring()
    }

    /// Constructs a shared `&BitSlice` reference over the object data, if available as slice.
    #[cfg(feature = "bitvec")]
    pub fn as_bitslice(&self) -> Result<&BitSlice<Msb0, u8>, BerError> {
        self.content.as_bitslice()
    }

    /// Attempt to extract the list of objects from a DER sequence.
    /// This can fail if the object is not a sequence.
    pub fn as_sequence(&self) -> Result<&Vec<BerObject<'a>>, BerError> {
        self.content.as_sequence()
    }

    /// Attempt to extract the list of objects from a DER set.
    /// This can fail if the object is not a set.
    pub fn as_set(&self) -> Result<&Vec<BerObject<'a>>, BerError> {
        self.content.as_set()
    }

    /// Attempt to get the content from a DER object, as a slice.
    /// This can fail if the object does not contain a type directly equivalent to a slice (e.g a
    /// sequence).
    /// This function mostly concerns string types, integers, or unknown DER objects.
    pub fn as_slice(&self) -> Result<&'a [u8], BerError> {
        self.content.as_slice()
    }

    /// Attempt to get the content from a DER object, as a str.
    /// This can fail if the object does not contain a string type.
    ///
    /// Only some string types are considered here. Other
    /// string types can be read using `as_slice`.
    pub fn as_str(&self) -> Result<&'a str, BerError> {
        self.content.as_str()
    }

    /// Get the BER object header's class.
    #[inline]
    pub const fn class(&self) -> Class {
        self.header.class()
    }

    /// Get the BER object header's tag.
    #[inline]
    pub const fn tag(&self) -> Tag {
        self.header.tag()
    }

    /// Get the BER object header's length.
    #[inline]
    pub const fn length(&self) -> Length {
        self.header.length()
    }

    /// Test if object class is Universal
    #[inline]
    pub const fn is_universal(&self) -> bool {
        self.header.is_universal()
    }
    /// Test if object class is Application
    #[inline]
    pub const fn is_application(&self) -> bool {
        self.header.is_application()
    }
    /// Test if object class is Context-specific
    #[inline]
    pub const fn is_contextspecific(&self) -> bool {
        self.header.is_contextspecific()
    }
    /// Test if object class is Private
    #[inline]
    pub fn is_private(&self) -> bool {
        self.header.is_private()
    }

    /// Test if object is primitive
    #[inline]
    pub const fn is_primitive(&self) -> bool {
        self.header.is_primitive()
    }
    /// Test if object is constructed
    #[inline]
    pub const fn is_constructed(&self) -> bool {
        self.header.is_constructed()
    }

    /// Return error if `class` is not the expected class
    #[inline]
    pub const fn assert_class(&self, class: Class) -> Result<(), BerError> {
        self.header.assert_class(class)
    }

    /// Return error if `tag` is not the expected tag
    #[inline]
    pub const fn assert_tag(&self, tag: Tag) -> Result<(), BerError> {
        self.header.assert_tag(tag)
    }

    /// Return error if object is not constructed
    #[inline]
    pub const fn assert_constructed(&self) -> Result<(), BerError> {
        self.header.assert_constructed()
    }

    /// Return error if object is not primitive
    #[inline]
    pub const fn assert_primitive(&self) -> Result<(), BerError> {
        self.header.assert_primitive()
    }
}

/// Build a DER object from an OID.
impl<'a> From<Oid<'a>> for BerObject<'a> {
    fn from(oid: Oid<'a>) -> BerObject<'a> {
        BerObject::from_obj(BerObjectContent::OID(oid))
    }
}

/// Build a DER object from a BerObjectContent.
impl<'a> From<BerObjectContent<'a>> for BerObject<'a> {
    fn from(obj: BerObjectContent<'a>) -> BerObject<'a> {
        BerObject::from_obj(obj)
    }
}

impl<'a> BerObjectContent<'a> {
    /// Attempt to read a signed integer value from this object.
    ///
    /// This can fail if the object is not an integer, or if it is too large.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use der_parser::ber::BerObject;
    /// let der_int  = BerObject::from_int_slice(b"\x80");
    /// assert_eq!(
    ///     der_int.as_i64(),
    ///     Ok(-128)
    /// );
    /// ```
    pub fn as_i64(&self) -> Result<i64, BerError> {
        if let BerObjectContent::Integer(bytes) = self {
            let result = if is_highest_bit_set(bytes) {
                <i64>::from_be_bytes(decode_array_int8(bytes)?)
            } else {
                <u64>::from_be_bytes(decode_array_uint8(bytes)?) as i64
            };
            Ok(result)
        } else {
            Err(BerError::BerValueError)
        }
    }

    /// Attempt to read a signed integer value from this object.
    ///
    /// This can fail if the object is not an integer, or if it is too large.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use der_parser::ber::BerObject;
    /// let der_int  = BerObject::from_int_slice(b"\x80");
    /// assert_eq!(
    ///     der_int.as_i32(),
    ///     Ok(-128)
    /// );
    /// ```
    pub fn as_i32(&self) -> Result<i32, BerError> {
        if let BerObjectContent::Integer(bytes) = self {
            let result = if is_highest_bit_set(bytes) {
                <i32>::from_be_bytes(decode_array_int4(bytes)?)
            } else {
                <u32>::from_be_bytes(decode_array_uint4(bytes)?) as i32
            };
            Ok(result)
        } else {
            Err(BerError::BerValueError)
        }
    }

    /// Attempt to read integer value from this object.
    ///
    /// This can fail if the object is not an unsigned integer, or if it is too large.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use der_parser::ber::BerObject;
    /// let der_int  = BerObject::from_int_slice(b"\x01\x00\x01");
    /// assert_eq!(
    ///     der_int.as_u64(),
    ///     Ok(0x10001)
    /// );
    /// ```
    pub fn as_u64(&self) -> Result<u64, BerError> {
        match self {
            BerObjectContent::Integer(i) => {
                let result = <u64>::from_be_bytes(decode_array_uint8(i)?);
                Ok(result)
            }
            BerObjectContent::BitString(ignored_bits, data) => {
                bitstring_to_u64(*ignored_bits as usize, data)
            }
            BerObjectContent::Enum(i) => Ok(*i as u64),
            _ => Err(BerError::BerTypeError),
        }
    }

    /// Attempt to read integer value from this object.
    ///
    /// This can fail if the object is not an unsigned integer, or if it is too large.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # extern crate der_parser;
    /// # use der_parser::ber::{BerObject,BerObjectContent};
    /// let der_int  = BerObject::from_obj(BerObjectContent::Integer(b"\x01\x00\x01"));
    /// assert_eq!(
    ///     der_int.as_u32(),
    ///     Ok(0x10001)
    /// );
    /// ```
    pub fn as_u32(&self) -> Result<u32, BerError> {
        match self {
            BerObjectContent::Integer(i) => {
                let result = <u32>::from_be_bytes(decode_array_uint4(i)?);
                Ok(result)
            }
            BerObjectContent::BitString(ignored_bits, data) => {
                bitstring_to_u64(*ignored_bits as usize, data).and_then(|x| {
                    if x > u64::from(core::u32::MAX) {
                        Err(BerError::IntegerTooLarge)
                    } else {
                        Ok(x as u32)
                    }
                })
            }
            BerObjectContent::Enum(i) => {
                if *i > u64::from(core::u32::MAX) {
                    Err(BerError::IntegerTooLarge)
                } else {
                    Ok(*i as u32)
                }
            }
            _ => Err(BerError::BerTypeError),
        }
    }

    pub fn as_bool(&self) -> Result<bool, BerError> {
        match *self {
            BerObjectContent::Boolean(b) => Ok(b),
            _ => Err(BerError::BerTypeError),
        }
    }

    pub fn as_oid(&self) -> Result<&Oid<'a>, BerError> {
        match *self {
            BerObjectContent::OID(ref o) => Ok(o),
            BerObjectContent::RelativeOID(ref o) => Ok(o),
            _ => Err(BerError::BerTypeError),
        }
    }

    pub fn as_oid_val(&self) -> Result<Oid<'a>, BerError> {
        self.as_oid().map(|o| o.clone())
    }

    pub fn as_optional(&self) -> Result<Option<&BerObject<'a>>, BerError> {
        match *self {
            BerObjectContent::Optional(Some(ref o)) => Ok(Some(o)),
            BerObjectContent::Optional(None) => Ok(None),
            _ => Err(BerError::BerTypeError),
        }
    }

    pub fn as_tagged(&self) -> Result<(Class, Tag, &BerObject<'a>), BerError> {
        match *self {
            BerObjectContent::Tagged(class, tag, ref o) => Ok((class, tag, o.as_ref())),
            _ => Err(BerError::BerTypeError),
        }
    }

    pub fn as_bitstring_ref(&self) -> Result<&BitStringObject, BerError> {
        match *self {
            BerObjectContent::BitString(_, ref b) => Ok(b),
            _ => Err(BerError::BerTypeError),
        }
    }

    pub fn as_bitstring(&self) -> Result<BitStringObject<'a>, BerError> {
        match *self {
            BerObjectContent::BitString(_, ref b) => Ok(b.to_owned()),
            _ => Err(BerError::BerTypeError),
        }
    }

    /// Constructs a shared `&BitSlice` reference over the object data, if available as slice.
    #[cfg(feature = "bitvec")]
    pub fn as_bitslice(&self) -> Result<&BitSlice<Msb0, u8>, BerError> {
        self.as_slice()
            .and_then(|s| BitSlice::<Msb0, _>::from_slice(s).map_err(|_| BerError::BerValueError))
    }

    pub fn as_sequence(&self) -> Result<&Vec<BerObject<'a>>, BerError> {
        match *self {
            BerObjectContent::Sequence(ref s) => Ok(s),
            _ => Err(BerError::BerTypeError),
        }
    }

    pub fn as_set(&self) -> Result<&Vec<BerObject<'a>>, BerError> {
        match *self {
            BerObjectContent::Set(ref s) => Ok(s),
            _ => Err(BerError::BerTypeError),
        }
    }

    #[rustfmt::skip]
    pub fn as_slice(&self) -> Result<&'a [u8],BerError> {
        match *self {
            BerObjectContent::NumericString(s) |
            BerObjectContent::BmpString(s) |
            BerObjectContent::VisibleString(s) |
            BerObjectContent::PrintableString(s) |
            BerObjectContent::GeneralString(s) |
            BerObjectContent::ObjectDescriptor(s) |
            BerObjectContent::GraphicString(s) |
            BerObjectContent::T61String(s) |
            BerObjectContent::VideotexString(s) |
            BerObjectContent::UTF8String(s) |
            BerObjectContent::IA5String(s) => Ok(s.as_ref()),
            BerObjectContent::Integer(s) |
            BerObjectContent::BitString(_,BitStringObject{data:s}) |
            BerObjectContent::OctetString(s) |
            BerObjectContent::UniversalString(s) => Ok(s),
            BerObjectContent::Unknown(ref any) => Ok(any.data),
            _ => Err(BerError::BerTypeError),
        }
    }

    #[rustfmt::skip]
    pub fn as_str(&self) -> Result<&'a str,BerError> {
        match *self {
            BerObjectContent::NumericString(s) |
            BerObjectContent::BmpString(s) |
            BerObjectContent::VisibleString(s) |
            BerObjectContent::PrintableString(s) |
            BerObjectContent::GeneralString(s) |
            BerObjectContent::ObjectDescriptor(s) |
            BerObjectContent::GraphicString(s) |
            BerObjectContent::T61String(s) |
            BerObjectContent::VideotexString(s) |
            BerObjectContent::UTF8String(s) |
            BerObjectContent::IA5String(s) => Ok(s),
            _ => Err(BerError::BerTypeError),
        }
    }

    #[rustfmt::skip]
    const fn tag(&self) -> Tag {
        match self {
            BerObjectContent::EndOfContent         => Tag::EndOfContent,
            BerObjectContent::Boolean(_)           => Tag::Boolean,
            BerObjectContent::Integer(_)           => Tag::Integer,
            BerObjectContent::BitString(_,_)       => Tag::BitString,
            BerObjectContent::OctetString(_)       => Tag::OctetString,
            BerObjectContent::Null                 => Tag::Null,
            BerObjectContent::Enum(_)              => Tag::Enumerated,
            BerObjectContent::OID(_)               => Tag::Oid,
            BerObjectContent::NumericString(_)     => Tag::NumericString,
            BerObjectContent::VisibleString(_)     => Tag::VisibleString,
            BerObjectContent::PrintableString(_)   => Tag::PrintableString,
            BerObjectContent::IA5String(_)         => Tag::Ia5String,
            BerObjectContent::UTF8String(_)        => Tag::Utf8String,
            BerObjectContent::RelativeOID(_)       => Tag::RelativeOid,
            BerObjectContent::T61String(_)         => Tag::T61String,
            BerObjectContent::VideotexString(_)    => Tag::VideotexString,
            BerObjectContent::BmpString(_)         => Tag::BmpString,
            BerObjectContent::UniversalString(_)   => Tag::UniversalString,
            BerObjectContent::Sequence(_)          => Tag::Sequence,
            BerObjectContent::Set(_)               => Tag::Set,
            BerObjectContent::UTCTime(_)           => Tag::UtcTime,
            BerObjectContent::GeneralizedTime(_)   => Tag::GeneralizedTime,
            BerObjectContent::ObjectDescriptor(_)  => Tag::ObjectDescriptor,
            BerObjectContent::GraphicString(_)     => Tag::GraphicString,
            BerObjectContent::GeneralString(_)     => Tag::GeneralString,
            BerObjectContent::Tagged(_,x,_) => *x,
            BerObjectContent::Unknown(any) => any.tag(),
            BerObjectContent::Optional(Some(obj))  => obj.content.tag(),
            BerObjectContent::Optional(None)       => Tag(0x00), // XXX invalid !
        }
    }
}

#[cfg(feature = "bigint")]
#[cfg_attr(docsrs, doc(cfg(feature = "bigint")))]
use num_bigint::{BigInt, BigUint};

#[cfg(feature = "bigint")]
#[cfg_attr(docsrs, doc(cfg(feature = "bigint")))]
impl<'a> BerObject<'a> {
    /// Attempt to read an integer value from this object.
    ///
    /// This can fail if the object is not an integer.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use der_parser::ber::*;
    ///
    /// let data = &[0x02, 0x03, 0x01, 0x00, 0x01];
    ///
    /// let (_, object) = parse_ber_integer(data).expect("parsing failed");
    /// # #[cfg(feature = "bigint")]
    /// assert_eq!(object.as_bigint(), Ok(65537.into()))
    /// ```
    pub fn as_bigint(&self) -> Result<BigInt, BerError> {
        match self.content {
            BerObjectContent::Integer(s) => Ok(BigInt::from_signed_bytes_be(s)),
            _ => Err(BerError::BerValueError),
        }
    }

    /// Attempt to read a positive integer value from this object.
    ///
    /// This can fail if the object is not an integer, or is negative.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use der_parser::ber::*;
    ///
    /// let data = &[0x02, 0x03, 0x01, 0x00, 0x01];
    ///
    /// let (_, object) = parse_ber_integer(data).expect("parsing failed");
    /// # #[cfg(feature = "bigint")]
    /// assert_eq!(object.as_biguint(), Ok(65537_u32.into()))
    /// ```
    pub fn as_biguint(&self) -> Result<BigUint, BerError> {
        match self.content {
            BerObjectContent::Integer(s) => {
                if is_highest_bit_set(s) {
                    return Err(BerError::IntegerNegative);
                }
                Ok(BigUint::from_bytes_be(s))
            }
            _ => Err(BerError::BerValueError),
        }
    }
}

// This is a consuming iterator
impl<'a> IntoIterator for BerObject<'a> {
    type Item = BerObject<'a>;
    type IntoIter = BerObjectIntoIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        // match self {
        //     BerObjectContent::Sequence(ref v) => (),
        //     _ => (),
        // };
        BerObjectIntoIterator { val: self, idx: 0 }
    }
}

#[derive(Debug)]
pub struct BerObjectIntoIterator<'a> {
    val: BerObject<'a>,
    idx: usize,
}

impl<'a> Iterator for BerObjectIntoIterator<'a> {
    type Item = BerObject<'a>;
    fn next(&mut self) -> Option<BerObject<'a>> {
        // let result = if self.idx < self.vec.len() {
        //     Some(self.vec[self.idx].clone())
        // } else {
        //     None
        // };
        let res = match self.val.content {
            BerObjectContent::Sequence(ref v) if self.idx < v.len() => Some(v[self.idx].clone()),
            BerObjectContent::Set(ref v) if self.idx < v.len() => Some(v[self.idx].clone()),
            _ => {
                if self.idx == 0 {
                    Some(self.val.clone())
                } else {
                    None
                }
            }
        };
        self.idx += 1;
        res
    }
}

// impl<'a> Iterator for BerObjectContent<'a> {
//     type Item = BerObjectContent<'a>;
//
//     fn next(&mut self) -> Option<BerObjectContent<'a>> {
//         None
//     }
// }

#[derive(Debug)]
pub struct BerObjectRefIterator<'a> {
    obj: &'a BerObject<'a>,
    idx: usize,
}

impl<'a> Iterator for BerObjectRefIterator<'a> {
    type Item = &'a BerObject<'a>;
    fn next(&mut self) -> Option<&'a BerObject<'a>> {
        let res = match (*self.obj).content {
            BerObjectContent::Sequence(ref v) if self.idx < v.len() => Some(&v[self.idx]),
            BerObjectContent::Set(ref v) if self.idx < v.len() => Some(&v[self.idx]),
            _ => None,
        };
        self.idx += 1;
        res
    }
}

impl<'a> BerObject<'a> {
    pub fn ref_iter(&'a self) -> BerObjectRefIterator<'a> {
        BerObjectRefIterator { obj: self, idx: 0 }
    }
}

impl<'a> Index<usize> for BerObject<'a> {
    type Output = BerObject<'a>;

    fn index(&self, idx: usize) -> &BerObject<'a> {
        match (*self).content {
            BerObjectContent::Sequence(ref v) if idx < v.len() => &v[idx],
            BerObjectContent::Set(ref v) if idx < v.len() => &v[idx],
            _ => panic!("Try to index BerObjectContent which is not constructed"),
        }
        // XXX the following
        // self.ref_iter().nth(idx).unwrap()
        // fails with:
        // error: cannot infer an appropriate lifetime for autoref due to conflicting requirements [E0495]
        // self.ref_iter().nth(idx).unwrap()
    }
}

/// BitString wrapper
#[derive(Clone, Debug, PartialEq)]
pub struct BitStringObject<'a> {
    pub data: &'a [u8],
}

impl<'a> BitStringObject<'a> {
    /// Test if bit `bitnum` is set
    pub fn is_set(&self, bitnum: usize) -> bool {
        let byte_pos = bitnum / 8;
        if byte_pos >= self.data.len() {
            return false;
        }
        let b = 7 - (bitnum % 8);
        (self.data[byte_pos] & (1 << b)) != 0
    }

    /// Constructs a shared `&BitSlice` reference over the object data.
    #[cfg(feature = "bitvec")]
    pub fn as_bitslice(&self) -> Option<&BitSlice<Msb0, u8>> {
        BitSlice::<Msb0, _>::from_slice(self.data).ok()
    }
}

impl<'a> AsRef<[u8]> for BitStringObject<'a> {
    fn as_ref(&self) -> &[u8] {
        self.data
    }
}

#[cfg(test)]
mod tests {
    use crate::ber::*;
    use crate::oid::*;

    #[test]
    fn test_der_as_u64() {
        let der_obj = BerObject::from_int_slice(b"\x01\x00\x02");
        assert_eq!(der_obj.as_u64(), Ok(0x10002));
    }

    #[test]
    fn test_ber_as_u64_bitstring() {
        let (_, ber_obj) = parse_ber_bitstring(b"\x03\x04\x06\x6e\x5d\xc0").unwrap();
        assert_eq!(ber_obj.as_u64(), Ok(0b011011100101110111));

        let (_, ber_obj_with_nonzero_padding) =
            parse_ber_bitstring(b"\x03\x04\x06\x6e\x5d\xe0").unwrap();
        assert_eq!(
            ber_obj_with_nonzero_padding.as_u64(),
            Ok(0b011011100101110111)
        );
    }

    #[test]
    fn test_der_seq_iter() {
        let der_obj = BerObject::from_obj(BerObjectContent::Sequence(vec![
            BerObject::from_int_slice(b"\x01\x00\x01"),
            BerObject::from_int_slice(b"\x01\x00\x00"),
        ]));
        let expected_values = vec![
            BerObject::from_int_slice(b"\x01\x00\x01"),
            BerObject::from_int_slice(b"\x01\x00\x00"),
        ];

        for (idx, v) in der_obj.ref_iter().enumerate() {
            // println!("v: {:?}", v);
            assert_eq!((*v), expected_values[idx]);
        }
    }

    #[test]
    fn test_der_from_oid() {
        let obj: BerObject = Oid::from(&[1, 2]).unwrap().into();
        let expected = BerObject::from_obj(BerObjectContent::OID(Oid::from(&[1, 2]).unwrap()));

        assert_eq!(obj, expected);
    }

    #[test]
    fn test_der_bitstringobject() {
        let obj = BitStringObject {
            data: &[0x0f, 0x00, 0x40],
        };
        assert!(!obj.is_set(0));
        assert!(obj.is_set(7));
        assert!(!obj.is_set(9));
        assert!(obj.is_set(17));
    }

    #[cfg(feature = "bitvec")]
    #[test]
    fn test_der_bitslice() {
        use std::string::String;
        let obj = BitStringObject {
            data: &[0x0f, 0x00, 0x40],
        };
        let slice = obj.as_bitslice().expect("as_bitslice");
        assert_eq!(slice.get(0).as_deref(), Some(&false));
        assert_eq!(slice.get(7).as_deref(), Some(&true));
        assert_eq!(slice.get(9).as_deref(), Some(&false));
        assert_eq!(slice.get(17).as_deref(), Some(&true));
        let s = slice.iter().fold(String::with_capacity(24), |mut acc, b| {
            acc += if *b { "1" } else { "0" };
            acc
        });
        assert_eq!(&s, "000011110000000001000000");
    }

    #[test]
    fn test_der_bistringobject_asref() {
        fn assert_equal<T: AsRef<[u8]>>(s: T, b: &[u8]) {
            assert_eq!(s.as_ref(), b);
        }
        let b: &[u8] = &[0x0f, 0x00, 0x40];
        let obj = BitStringObject { data: b };
        assert_equal(obj, b);
    }

    #[cfg(feature = "bigint")]
    #[test]
    fn test_der_to_bigint() {
        let obj = BerObject::from_obj(BerObjectContent::Integer(b"\x01\x00\x01"));
        let expected = ::num_bigint::BigInt::from(0x10001);

        assert_eq!(obj.as_bigint(), Ok(expected));
    }

    #[cfg(feature = "bigint")]
    #[test]
    fn test_der_to_biguint() {
        let obj = BerObject::from_obj(BerObjectContent::Integer(b"\x01\x00\x01"));
        let expected = ::num_bigint::BigUint::from(0x10001_u32);

        assert_eq!(obj.as_biguint(), Ok(expected));
    }
}
