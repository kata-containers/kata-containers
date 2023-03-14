use crate::ber::*;
use crate::*;
use alloc::borrow::Cow;
use alloc::string::String;
use core::convert::{TryFrom, TryInto};

/// The `Any` object is not strictly an ASN.1 type, but holds a generic description of any object
/// that could be encoded.
///
/// It contains a header, and either a reference to or owned data for the object content.
///
/// Note: this type is only provided in **borrowed** version (*i.e.* it cannot own the inner data).
#[derive(Clone, Debug, PartialEq)]
pub struct Any<'a> {
    /// The object header
    pub header: Header<'a>,
    /// The object contents
    pub data: &'a [u8],
}

impl<'a> Any<'a> {
    /// Create a new `Any` from BER/DER header and content
    #[inline]
    pub const fn new(header: Header<'a>, data: &'a [u8]) -> Self {
        Any { header, data }
    }

    /// Create a new `Any` from a tag, and BER/DER content
    #[inline]
    pub const fn from_tag_and_data(tag: Tag, data: &'a [u8]) -> Self {
        let constructed = matches!(tag, Tag::Sequence | Tag::Set);
        Any {
            header: Header {
                tag,
                constructed,
                class: Class::Universal,
                length: Length::Definite(data.len()),
                raw_tag: None,
            },
            data,
        }
    }

    /// Return the `Class` of this object
    #[inline]
    pub const fn class(&self) -> Class {
        self.header.class
    }

    /// Update the class of the current object
    #[inline]
    pub fn with_class(self, class: Class) -> Self {
        Any {
            header: self.header.with_class(class),
            ..self
        }
    }

    /// Return the `Tag` of this object
    #[inline]
    pub const fn tag(&self) -> Tag {
        self.header.tag
    }

    /// Update the tag of the current object
    #[inline]
    pub fn with_tag(self, tag: Tag) -> Self {
        Any {
            header: self.header.with_tag(tag),
            data: self.data,
        }
    }

    /// Get the bytes representation of the *content*
    #[inline]
    pub fn as_bytes(&'a self) -> &'a [u8] {
        self.data
    }

    #[inline]
    pub fn parse_ber<T>(&'a self) -> ParseResult<'a, T>
    where
        T: FromBer<'a>,
    {
        T::from_ber(self.data)
    }

    /// Parse a BER value and apply the provided parsing function to content
    ///
    /// After parsing, the sequence object and header are discarded.
    pub fn from_ber_and_then<F, T, E>(
        class: Class,
        tag: u32,
        bytes: &'a [u8],
        op: F,
    ) -> ParseResult<'a, T, E>
    where
        F: FnOnce(&'a [u8]) -> ParseResult<T, E>,
        E: From<Error>,
    {
        let (rem, any) = Any::from_ber(bytes).map_err(Err::convert)?;
        any.tag()
            .assert_eq(Tag(tag))
            .map_err(|e| nom::Err::Error(e.into()))?;
        any.class()
            .assert_eq(class)
            .map_err(|e| nom::Err::Error(e.into()))?;
        let (_, res) = op(any.data)?;
        Ok((rem, res))
    }

    /// Parse a DER value and apply the provided parsing function to content
    ///
    /// After parsing, the sequence object and header are discarded.
    pub fn from_der_and_then<F, T, E>(
        class: Class,
        tag: u32,
        bytes: &'a [u8],
        op: F,
    ) -> ParseResult<'a, T, E>
    where
        F: FnOnce(&'a [u8]) -> ParseResult<T, E>,
        E: From<Error>,
    {
        let (rem, any) = Any::from_der(bytes).map_err(Err::convert)?;
        any.tag()
            .assert_eq(Tag(tag))
            .map_err(|e| nom::Err::Error(e.into()))?;
        any.class()
            .assert_eq(class)
            .map_err(|e| nom::Err::Error(e.into()))?;
        let (_, res) = op(any.data)?;
        Ok((rem, res))
    }

    #[inline]
    pub fn parse_der<T>(&'a self) -> ParseResult<'a, T>
    where
        T: FromDer<'a>,
    {
        T::from_der(self.data)
    }

    /// Get the content following a BER header
    #[inline]
    pub fn parse_ber_content<'i>(i: &'i [u8], header: &'_ Header) -> ParseResult<'i, &'i [u8]> {
        header.parse_ber_content(i)
    }

    /// Get the content following a DER header
    #[inline]
    pub fn parse_der_content<'i>(i: &'i [u8], header: &'_ Header) -> ParseResult<'i, &'i [u8]> {
        header.assert_definite()?;
        ber_get_object_content(i, header, 8)
    }
}

macro_rules! impl_any_into {
    (IMPL $sname:expr, $fn_name:ident => $ty:ty, $asn1:expr) => {
        #[doc = "Attempt to convert object to `"]
        #[doc = $sname]
        #[doc = "` (ASN.1 type: `"]
        #[doc = $asn1]
        #[doc = "`)."]
        pub fn $fn_name(self) -> Result<$ty> {
            self.try_into()
        }
    };
    ($fn_name:ident => $ty:ty, $asn1:expr) => {
        impl_any_into! {
            IMPL stringify!($ty), $fn_name => $ty, $asn1
        }
    };
}

macro_rules! impl_any_as {
    (IMPL $sname:expr, $fn_name:ident => $ty:ty, $asn1:expr) => {
        #[doc = "Attempt to create ASN.1 type `"]
        #[doc = $asn1]
        #[doc = "` from this object."]
        #[inline]
        pub fn $fn_name(&self) -> Result<$ty> {
            TryFrom::try_from(self)
        }
    };
    ($fn_name:ident => $ty:ty, $asn1:expr) => {
        impl_any_as! {
            IMPL stringify!($ty), $fn_name => $ty, $asn1
        }
    };
}

impl<'a> Any<'a> {
    impl_any_into!(bitstring => BitString<'a>, "BIT STRING");
    impl_any_into!(bmpstring => BmpString<'a>, "BmpString");
    impl_any_into!(bool => bool, "BOOLEAN");
    impl_any_into!(boolean => Boolean, "BOOLEAN");
    impl_any_into!(embedded_pdv => EmbeddedPdv<'a>, "EMBEDDED PDV");
    impl_any_into!(enumerated => Enumerated, "ENUMERATED");
    impl_any_into!(generalizedtime => GeneralizedTime, "GeneralizedTime");
    impl_any_into!(generalstring => GeneralString<'a>, "GeneralString");
    impl_any_into!(graphicstring => GraphicString<'a>, "GraphicString");
    impl_any_into!(i8 => i8, "INTEGER");
    impl_any_into!(i16 => i16, "INTEGER");
    impl_any_into!(i32 => i32, "INTEGER");
    impl_any_into!(i64 => i64, "INTEGER");
    impl_any_into!(i128 => i128, "INTEGER");
    impl_any_into!(ia5string => Ia5String<'a>, "IA5String");
    impl_any_into!(integer => Integer<'a>, "INTEGER");
    impl_any_into!(null => Null, "NULL");
    impl_any_into!(numericstring => NumericString<'a>, "NumericString");
    impl_any_into!(objectdescriptor => ObjectDescriptor<'a>, "ObjectDescriptor");
    impl_any_into!(octetstring => OctetString<'a>, "OCTET STRING");
    impl_any_into!(oid => Oid<'a>, "OBJECT IDENTIFIER");
    /// Attempt to convert object to `Oid` (ASN.1 type: `RELATIVE-OID`).
    pub fn relative_oid(self) -> Result<Oid<'a>> {
        self.header.assert_tag(Tag::RelativeOid)?;
        let asn1 = Cow::Borrowed(self.data);
        Ok(Oid::new_relative(asn1))
    }
    impl_any_into!(printablestring => PrintableString<'a>, "PrintableString");
    // XXX REAL
    impl_any_into!(sequence => Sequence<'a>, "SEQUENCE");
    impl_any_into!(set => Set<'a>, "SET");
    impl_any_into!(str => &'a str, "UTF8String");
    impl_any_into!(string => String, "UTF8String");
    impl_any_into!(teletexstring => TeletexString<'a>, "TeletexString");
    impl_any_into!(u8 => u8, "INTEGER");
    impl_any_into!(u16 => u16, "INTEGER");
    impl_any_into!(u32 => u32, "INTEGER");
    impl_any_into!(u64 => u64, "INTEGER");
    impl_any_into!(u128 => u128, "INTEGER");
    impl_any_into!(universalstring => UniversalString<'a>, "UniversalString");
    impl_any_into!(utctime => UtcTime, "UTCTime");
    impl_any_into!(utf8string => Utf8String<'a>, "UTF8String");
    impl_any_into!(videotexstring => VideotexString<'a>, "VideotexString");
    impl_any_into!(visiblestring => VisibleString<'a>, "VisibleString");

    impl_any_as!(as_bitstring => BitString, "BITSTRING");
    impl_any_as!(as_bool => bool, "BOOLEAN");
    impl_any_as!(as_boolean => Boolean, "BOOLEAN");
    impl_any_as!(as_embedded_pdv => EmbeddedPdv, "EMBEDDED PDV");
    impl_any_as!(as_endofcontent => EndOfContent, "END OF CONTENT (not a real ASN.1 type)");
    impl_any_as!(as_enumerated => Enumerated, "ENUMERATED");
    impl_any_as!(as_generalizedtime => GeneralizedTime, "GeneralizedTime");
    impl_any_as!(as_generalstring => GeneralizedTime, "GeneralString");
    impl_any_as!(as_graphicstring => GraphicString, "GraphicString");
    impl_any_as!(as_i8 => i8, "INTEGER");
    impl_any_as!(as_i16 => i16, "INTEGER");
    impl_any_as!(as_i32 => i32, "INTEGER");
    impl_any_as!(as_i64 => i64, "INTEGER");
    impl_any_as!(as_i128 => i128, "INTEGER");
    impl_any_as!(as_ia5string => Ia5String, "IA5String");
    impl_any_as!(as_integer => Integer, "INTEGER");
    impl_any_as!(as_null => Null, "NULL");
    impl_any_as!(as_numericstring => NumericString, "NumericString");
    impl_any_as!(as_objectdescriptor => ObjectDescriptor, "OBJECT IDENTIFIER");
    impl_any_as!(as_octetstring => OctetString, "OCTET STRING");
    impl_any_as!(as_oid => Oid, "OBJECT IDENTIFIER");
    /// Attempt to create ASN.1 type `RELATIVE-OID` from this object.
    pub fn as_relative_oid(&self) -> Result<Oid<'a>> {
        self.header.assert_tag(Tag::RelativeOid)?;
        let asn1 = Cow::Borrowed(self.data);
        Ok(Oid::new_relative(asn1))
    }
    impl_any_as!(as_printablestring => PrintableString, "PrintableString");
    impl_any_as!(as_sequence => Sequence, "SEQUENCE");
    impl_any_as!(as_set => Set, "SET");
    impl_any_as!(as_str => &str, "UTF8String");
    impl_any_as!(as_string => String, "UTF8String");
    impl_any_as!(as_teletexstring => TeletexString, "TeletexString");
    impl_any_as!(as_u8 => u8, "INTEGER");
    impl_any_as!(as_u16 => u16, "INTEGER");
    impl_any_as!(as_u32 => u32, "INTEGER");
    impl_any_as!(as_u64 => u64, "INTEGER");
    impl_any_as!(as_u128 => u128, "INTEGER");
    impl_any_as!(as_universalstring => UniversalString, "UniversalString");
    impl_any_as!(as_utctime => UtcTime, "UTCTime");
    impl_any_as!(as_utf8string => Utf8String, "UTF8String");
    impl_any_as!(as_videotexstring => VideotexString, "VideotexString");
    impl_any_as!(as_visiblestring => VisibleString, "VisibleString");

    /// Attempt to create an `Option<T>` from this object.
    pub fn as_optional<'b, T>(&'b self) -> Result<Option<T>>
    where
        T: TryFrom<&'b Any<'a>, Error = Error>,
        'a: 'b,
    {
        match TryFrom::try_from(self) {
            Ok(t) => Ok(Some(t)),
            Err(Error::UnexpectedTag { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Attempt to create a tagged value (EXPLICIT) from this object.
    pub fn as_tagged_explicit<T, E, const CLASS: u8, const TAG: u32>(
        &self,
    ) -> Result<TaggedValue<T, E, Explicit, CLASS, TAG>, E>
    where
        T: FromBer<'a, E>,
        E: From<Error>,
    {
        TryFrom::try_from(self)
    }

    /// Attempt to create a tagged value (IMPLICIT) from this object.
    pub fn as_tagged_implicit<T, E, const CLASS: u8, const TAG: u32>(
        &self,
    ) -> Result<TaggedValue<T, E, Implicit, CLASS, TAG>, E>
    where
        T: TryFrom<Any<'a>, Error = E>,
        T: Tagged,
        E: From<Error>,
    {
        TryFrom::try_from(self)
    }
}

impl<'a> FromBer<'a> for Any<'a> {
    fn from_ber(bytes: &'a [u8]) -> ParseResult<Self> {
        let (i, header) = Header::from_ber(bytes)?;
        let (i, data) = ber_get_object_content(i, &header, MAX_RECURSION)?;
        Ok((i, Any { header, data }))
    }
}

impl<'a> FromDer<'a> for Any<'a> {
    fn from_der(bytes: &'a [u8]) -> ParseResult<Self> {
        let (i, header) = Header::from_der(bytes)?;
        // X.690 section 10.1: The definite form of length encoding shall be used
        header.length.assert_definite()?;
        let (i, data) = ber_get_object_content(i, &header, MAX_RECURSION)?;
        Ok((i, Any { header, data }))
    }
}

impl CheckDerConstraints for Any<'_> {
    fn check_constraints(any: &Any) -> Result<()> {
        any.header.length().assert_definite()?;
        // if len < 128, must use short form (10.1: minimum number of octets)
        Ok(())
    }
}

impl DerAutoDerive for Any<'_> {}

impl DynTagged for Any<'_> {
    fn tag(&self) -> Tag {
        self.tag()
    }
}

// impl<'a> ToStatic for Any<'a> {
//     type Owned = Any<'static>;

//     fn to_static(&self) -> Self::Owned {
//         Any {
//             header: self.header.to_static(),
//             data: Cow::Owned(self.data.to_vec()),
//         }
//     }
// }

#[cfg(feature = "std")]
impl ToDer for Any<'_> {
    fn to_der_len(&self) -> Result<usize> {
        let hdr_len = self.header.to_der_len()?;
        Ok(hdr_len + self.data.len())
    }

    fn write_der_header(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        // create fake header to have correct length
        let header = Header::new(
            self.header.class,
            self.header.constructed,
            self.header.tag,
            Length::Definite(self.data.len()),
        );
        let sz = header.write_der_header(writer)?;
        Ok(sz)
    }

    fn write_der_content(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        writer.write(self.data).map_err(Into::into)
    }

    /// Similar to using `to_der`, but uses header without computing length value
    fn write_der_raw(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        let sz = self.header.write_der_header(writer)?;
        let sz = sz + writer.write(self.data)?;
        Ok(sz)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use hex_literal::hex;

    #[test]
    fn methods_any() {
        let header = Header::new_simple(Tag::Integer);
        let any = Any::new(header, &[])
            .with_class(Class::ContextSpecific)
            .with_tag(Tag(0));
        assert_eq!(any.as_bytes(), &[]);

        let input = &hex! {"80 03 02 01 01"};
        let (_, any) = Any::from_ber(input).expect("parsing failed");

        let (_, r) = any.parse_ber::<Integer>().expect("parse_ber failed");
        assert_eq!(r.as_u32(), Ok(1));
        let (_, r) = any.parse_der::<Integer>().expect("parse_der failed");
        assert_eq!(r.as_u32(), Ok(1));

        let header = &any.header;
        let (_, content) = Any::parse_ber_content(&input[2..], header).unwrap();
        assert_eq!(content.len(), 3);
        let (_, content) = Any::parse_der_content(&input[2..], header).unwrap();
        assert_eq!(content.len(), 3);

        let (_, any) = Any::from_der(&input[2..]).unwrap();
        Any::check_constraints(&any).unwrap();
        assert_eq!(<Any as DynTagged>::tag(&any), any.tag());
        let int = any.integer().unwrap();
        assert_eq!(int.as_u16(), Ok(1));
    }
}
