use serde::{
    de::{self, DeserializeSeed, VariantAccess, Visitor},
    Deserialize,
};
use static_assertions::assert_impl_all;

use std::{marker::PhantomData, str};

#[cfg(unix)]
use std::os::unix::io::RawFd;

#[cfg(feature = "gvariant")]
use crate::gvariant::Deserializer as GVDeserializer;
use crate::{
    container_depths::ContainerDepths, dbus::Deserializer as DBusDeserializer,
    signature_parser::SignatureParser, utils::*, Basic, DynamicDeserialize, DynamicType,
    EncodingContext, EncodingFormat, Error, ObjectPath, Result, Signature, Type,
};

#[cfg(unix)]
use crate::Fd;

/// Deserialize `T` from a given slice of bytes, containing file descriptor indices.
///
/// Please note that actual file descriptors are not part of the encoding and need to be transferred
/// via an out-of-band platform specific mechanism. The encoding only contain the indices of the
/// file descriptors and hence the reason, caller must pass a slice of file descriptors.
///
/// This function is not available on Windows.
///
/// # Examples
///
/// ```
/// use zvariant::{to_bytes_fds, from_slice_fds};
/// use zvariant::{EncodingContext, Fd};
///
/// let ctxt = EncodingContext::<byteorder::LE>::new_dbus(0);
/// let (encoded, fds) = to_bytes_fds(ctxt, &Fd::from(42)).unwrap();
/// let decoded: Fd = from_slice_fds(&encoded, Some(&fds), ctxt).unwrap();
/// assert_eq!(decoded, Fd::from(42));
/// ```
///
/// [`from_slice`]: fn.from_slice.html
#[cfg(unix)]
pub fn from_slice_fds<'d, 'r: 'd, B, T: ?Sized>(
    bytes: &'r [u8],
    fds: Option<&[RawFd]>,
    ctxt: EncodingContext<B>,
) -> Result<T>
where
    B: byteorder::ByteOrder,
    T: Deserialize<'d> + Type,
{
    let signature = T::signature();
    from_slice_fds_for_signature(bytes, fds, ctxt, &signature)
}

/// Deserialize `T` from a given slice of bytes.
///
/// If `T` is an, or (potentially) contains an [`Fd`], use [`from_slice_fds`] instead.
///
/// # Examples
///
/// ```
/// use zvariant::{to_bytes, from_slice};
/// use zvariant::EncodingContext;
///
/// let ctxt = EncodingContext::<byteorder::LE>::new_dbus(0);
/// let encoded = to_bytes(ctxt, "hello world").unwrap();
/// let decoded: &str = from_slice(&encoded, ctxt).unwrap();
/// assert_eq!(decoded, "hello world");
/// ```
///
/// [`Fd`]: struct.Fd.html
/// [`from_slice_fds`]: fn.from_slice_fds.html
pub fn from_slice<'d, 'r: 'd, B, T: ?Sized>(bytes: &'r [u8], ctxt: EncodingContext<B>) -> Result<T>
where
    B: byteorder::ByteOrder,
    T: Deserialize<'d> + Type,
{
    let signature = T::signature();
    from_slice_for_signature(bytes, ctxt, &signature)
}

/// Deserialize `T` from a given slice of bytes with the given signature.
///
/// Use this function instead of [`from_slice`] if the value being deserialized does not implement
/// [`Type`]. Also, if `T` is an, or (potentially) contains an [`Fd`], use
/// [`from_slice_fds_for_signature`] instead.
///
/// # Examples
///
/// While `Type` derive supports enums, for this example, let's supposed it doesn't and we don't
/// want to manually implement `Type` trait either:
///
/// ```
/// use std::convert::TryInto;
/// use serde::{Deserialize, Serialize};
///
/// use zvariant::{to_bytes_for_signature, from_slice_for_signature};
/// use zvariant::EncodingContext;
///
/// let ctxt = EncodingContext::<byteorder::LE>::new_dbus(0);
/// #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
/// enum Unit {
///     Variant1,
///     Variant2,
///     Variant3,
/// }
///
/// let signature = "u".try_into().unwrap();
/// let encoded = to_bytes_for_signature(ctxt, &signature, &Unit::Variant2).unwrap();
/// assert_eq!(encoded.len(), 4);
/// let decoded: Unit = from_slice_for_signature(&encoded, ctxt, &signature).unwrap();
/// assert_eq!(decoded, Unit::Variant2);
///
/// #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
/// enum NewType<'s> {
///     Variant1(&'s str),
///     Variant2(&'s str),
///     Variant3(&'s str),
/// }
///
/// let signature = "(us)".try_into().unwrap();
/// let encoded =
///     to_bytes_for_signature(ctxt, &signature, &NewType::Variant2("hello")).unwrap();
/// assert_eq!(encoded.len(), 14);
/// let decoded: NewType<'_> = from_slice_for_signature(&encoded, ctxt, &signature).unwrap();
/// assert_eq!(decoded, NewType::Variant2("hello"));
///
/// #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
/// enum Structs {
///     Tuple(u8, u64),
///     Struct { y: u8, t: u64 },
/// }
///
/// // TODO: Provide convenience API to create complex signatures
/// let signature = "(u(yt))".try_into().unwrap();
/// let encoded = to_bytes_for_signature(ctxt, &signature, &Structs::Tuple(42, 42)).unwrap();
/// assert_eq!(encoded.len(), 24);
/// let decoded: Structs = from_slice_for_signature(&encoded, ctxt, &signature).unwrap();
/// assert_eq!(decoded, Structs::Tuple(42, 42));
///
/// let s = Structs::Struct { y: 42, t: 42 };
/// let encoded = to_bytes_for_signature(ctxt, &signature, &s).unwrap();
/// assert_eq!(encoded.len(), 24);
/// let decoded: Structs = from_slice_for_signature(&encoded, ctxt, &signature).unwrap();
/// assert_eq!(decoded, Structs::Struct { y: 42, t: 42 });
/// ```
///
/// [`Type`]: trait.Type.html
/// [`Fd`]: struct.Fd.html
/// [`from_slice_fds_for_signature`]: fn.from_slice_fds_for_signature.html
// TODO: Return number of bytes parsed?
pub fn from_slice_for_signature<'d, 'r: 'd, B, T: ?Sized>(
    bytes: &'r [u8],
    ctxt: EncodingContext<B>,
    signature: &Signature<'_>,
) -> Result<T>
where
    B: byteorder::ByteOrder,
    T: Deserialize<'d>,
{
    _from_slice_fds_for_signature(
        bytes,
        #[cfg(unix)]
        None,
        ctxt,
        signature,
    )
}

/// Deserialize `T` from a given slice of bytes containing file descriptor indices, with the given signature.
///
/// Please note that actual file descriptors are not part of the encoding and need to be transferred
/// via an out-of-band platform specific mechanism. The encoding only contain the indices of the
/// file descriptors and hence the reason, caller must pass a slice of file descriptors.
///
/// This function is not available on Windows.
///
/// [`from_slice`]: fn.from_slice.html
/// [`from_slice_for_signature`]: fn.from_slice_for_signature.html
// TODO: Return number of bytes parsed?
#[cfg(unix)]
pub fn from_slice_fds_for_signature<'d, 'r: 'd, B, T: ?Sized>(
    bytes: &'r [u8],
    fds: Option<&[RawFd]>,
    ctxt: EncodingContext<B>,
    signature: &Signature<'_>,
) -> Result<T>
where
    B: byteorder::ByteOrder,
    T: Deserialize<'d>,
{
    _from_slice_fds_for_signature(bytes, fds, ctxt, signature)
}

fn _from_slice_fds_for_signature<'d, 'r: 'd, B, T: ?Sized>(
    bytes: &'r [u8],
    #[cfg(unix)] fds: Option<&[RawFd]>,
    ctxt: EncodingContext<B>,
    signature: &Signature<'_>,
) -> Result<T>
where
    B: byteorder::ByteOrder,
    T: Deserialize<'d>,
{
    let mut de = match ctxt.format() {
        #[cfg(feature = "gvariant")]
        EncodingFormat::GVariant => Deserializer::GVariant(GVDeserializer::new(
            bytes,
            #[cfg(unix)]
            fds,
            signature,
            ctxt,
        )),
        EncodingFormat::DBus => Deserializer::DBus(DBusDeserializer::new(
            bytes,
            #[cfg(unix)]
            fds,
            signature,
            ctxt,
        )),
    };

    T::deserialize(&mut de)
}

/// Deserialize `T` from a given slice of bytes containing file descriptor indices, with the given
/// signature.
///
/// Please note that actual file descriptors are not part of the encoding and need to be transferred
/// via an out-of-band platform specific mechanism. The encoding only contain the indices of the
/// file descriptors and hence the reason, caller must pass a slice of file descriptors.
pub fn from_slice_for_dynamic_signature<'d, B, T>(
    bytes: &'d [u8],
    ctxt: EncodingContext<B>,
    signature: &Signature<'d>,
) -> Result<T>
where
    B: byteorder::ByteOrder,
    T: DynamicDeserialize<'d>,
{
    let seed = T::deserializer_for_signature(signature)?;

    from_slice_with_seed(bytes, ctxt, seed)
}

/// Deserialize `T` from a given slice of bytes containing file descriptor indices, with the given
/// signature.
///
/// Please note that actual file descriptors are not part of the encoding and need to be transferred
/// via an out-of-band platform specific mechanism. The encoding only contain the indices of the
/// file descriptors and hence the reason, caller must pass a slice of file descriptors.
///
/// This function is not available on Windows.
#[cfg(unix)]
pub fn from_slice_fds_for_dynamic_signature<'d, B, T>(
    bytes: &'d [u8],
    fds: Option<&[RawFd]>,
    ctxt: EncodingContext<B>,
    signature: &Signature<'d>,
) -> Result<T>
where
    B: byteorder::ByteOrder,
    T: DynamicDeserialize<'d>,
{
    let seed = T::deserializer_for_signature(signature)?;

    from_slice_fds_with_seed(bytes, fds, ctxt, seed)
}

/// Deserialize `T` from a given slice of bytes containing file descriptor indices, using the given
/// seed.
///
/// Please note that actual file descriptors are not part of the encoding and need to be transferred
/// via an out-of-band platform specific mechanism. The encoding only contain the indices of the
/// file descriptors and hence the reason, caller must pass a slice of file descriptors.
pub fn from_slice_with_seed<'d, B, S>(
    bytes: &'d [u8],
    ctxt: EncodingContext<B>,
    seed: S,
) -> Result<S::Value>
where
    B: byteorder::ByteOrder,
    S: DeserializeSeed<'d> + DynamicType,
{
    _from_slice_fds_with_seed(
        bytes,
        #[cfg(unix)]
        None,
        ctxt,
        seed,
    )
}

/// Deserialize `T` from a given slice of bytes containing file descriptor indices, using the given
/// seed.
///
/// Please note that actual file descriptors are not part of the encoding and need to be transferred
/// via an out-of-band platform specific mechanism. The encoding only contain the indices of the
/// file descriptors and hence the reason, caller must pass a slice of file descriptors.
///
/// This function is not available on Windows.
#[cfg(unix)]
pub fn from_slice_fds_with_seed<'d, B, S>(
    bytes: &'d [u8],
    fds: Option<&[RawFd]>,
    ctxt: EncodingContext<B>,
    seed: S,
) -> Result<S::Value>
where
    B: byteorder::ByteOrder,
    S: DeserializeSeed<'d> + DynamicType,
{
    _from_slice_fds_with_seed(bytes, fds, ctxt, seed)
}

fn _from_slice_fds_with_seed<'d, B, S>(
    bytes: &'d [u8],
    #[cfg(unix)] fds: Option<&[RawFd]>,
    ctxt: EncodingContext<B>,
    seed: S,
) -> Result<S::Value>
where
    B: byteorder::ByteOrder,
    S: DeserializeSeed<'d> + DynamicType,
{
    let signature = S::dynamic_signature(&seed).to_owned();

    let mut de = match ctxt.format() {
        #[cfg(feature = "gvariant")]
        EncodingFormat::GVariant => Deserializer::GVariant(GVDeserializer::new(
            bytes,
            #[cfg(unix)]
            fds,
            &signature,
            ctxt,
        )),
        EncodingFormat::DBus => Deserializer::DBus(DBusDeserializer::new(
            bytes,
            #[cfg(unix)]
            fds,
            &signature,
            ctxt,
        )),
    };

    seed.deserialize(&mut de)
}

/// Our deserialization implementation.
#[derive(Debug)]
pub(crate) struct DeserializerCommon<'de, 'sig, 'f, B> {
    pub(crate) ctxt: EncodingContext<B>,
    pub(crate) bytes: &'de [u8],

    #[cfg(unix)]
    pub(crate) fds: Option<&'f [RawFd]>,
    #[cfg(not(unix))]
    pub(crate) fds: PhantomData<&'f ()>,

    pub(crate) pos: usize,

    pub(crate) sig_parser: SignatureParser<'sig>,

    pub(crate) container_depths: ContainerDepths,

    pub(crate) b: PhantomData<B>,
}

/// Our deserialization implementation.
///
/// Using this deserializer involves an redirection to the actual deserializer. It's best
/// to use the serialization functions, e.g [`crate::to_bytes`] or specific serializers,
/// [`crate::dbus::Deserializer`] or [`crate::zvariant::Deserializer`].
pub enum Deserializer<'ser, 'sig, 'f, B> {
    DBus(DBusDeserializer<'ser, 'sig, 'f, B>),
    #[cfg(feature = "gvariant")]
    GVariant(GVDeserializer<'ser, 'sig, 'f, B>),
}

assert_impl_all!(Deserializer<'_, '_, '_, u8>: Send, Sync, Unpin);

impl<'de, 'sig, 'f, B> Deserializer<'de, 'sig, 'f, B>
where
    B: byteorder::ByteOrder,
{
    /// Create a Deserializer struct instance.
    ///
    /// On Windows, there is no `fds` argument.
    pub fn new<'r: 'de>(
        bytes: &'r [u8],
        #[cfg(unix)] fds: Option<&'f [RawFd]>,
        signature: &Signature<'sig>,
        ctxt: EncodingContext<B>,
    ) -> Self {
        match ctxt.format() {
            #[cfg(feature = "gvariant")]
            EncodingFormat::GVariant => Self::GVariant(GVDeserializer::new(
                bytes,
                #[cfg(unix)]
                fds,
                signature,
                ctxt,
            )),
            EncodingFormat::DBus => Self::DBus(DBusDeserializer::new(
                bytes,
                #[cfg(unix)]
                fds,
                signature,
                ctxt,
            )),
        }
    }
}

impl<'de, 'sig, 'f, B> DeserializerCommon<'de, 'sig, 'f, B>
where
    B: byteorder::ByteOrder,
{
    #[cfg(unix)]
    pub fn get_fd(&self, idx: u32) -> Result<i32> {
        self.fds
            .and_then(|fds| fds.get(idx as usize))
            .copied()
            .ok_or(Error::UnknownFd)
    }

    pub fn parse_padding(&mut self, alignment: usize) -> Result<usize> {
        let padding = padding_for_n_bytes(self.abs_pos(), alignment);
        if padding > 0 {
            if self.pos + padding > self.bytes.len() {
                return Err(serde::de::Error::invalid_length(
                    self.bytes.len(),
                    &format!(">= {}", self.pos + padding).as_str(),
                ));
            }

            for i in 0..padding {
                let byte = self.bytes[self.pos + i];
                if byte != 0 {
                    return Err(Error::PaddingNot0(byte));
                }
            }
            self.pos += padding;
        }

        Ok(padding)
    }

    pub fn prep_deserialize_basic<T>(&mut self) -> Result<()>
    where
        T: Basic,
    {
        self.sig_parser.skip_char()?;
        self.parse_padding(T::alignment(self.ctxt.format()))?;

        Ok(())
    }

    pub fn next_slice(&mut self, len: usize) -> Result<&'de [u8]> {
        if self.pos + len > self.bytes.len() {
            return Err(serde::de::Error::invalid_length(
                self.bytes.len(),
                &format!(">= {}", self.pos + len).as_str(),
            ));
        }

        let slice = &self.bytes[self.pos..self.pos + len];
        self.pos += len;

        Ok(slice)
    }

    pub fn next_const_size_slice<T>(&mut self) -> Result<&[u8]>
    where
        T: Basic,
    {
        self.prep_deserialize_basic::<T>()?;

        self.next_slice(T::alignment(self.ctxt.format()))
    }

    pub fn abs_pos(&self) -> usize {
        self.ctxt.position() + self.pos
    }
}

macro_rules! deserialize_method {
    ($method:ident($($arg:ident: $type:ty),*)) => {
        #[inline]
        fn $method<V>(self, $($arg: $type,)* visitor: V) -> Result<V::Value>
        where
            V: Visitor<'de>,
        {
            match self {
                #[cfg(feature = "gvariant")]
                Deserializer::GVariant(de) => {
                    de.$method($($arg,)* visitor)
                }
                Deserializer::DBus(de) => {
                    de.$method($($arg,)* visitor)
                }
            }
        }
    }
}

impl<'de, 'd, 'sig, 'f, B> de::Deserializer<'de> for &'d mut Deserializer<'de, 'sig, 'f, B>
where
    B: byteorder::ByteOrder,
{
    type Error = Error;

    deserialize_method!(deserialize_any());
    deserialize_method!(deserialize_bool());
    deserialize_method!(deserialize_i8());
    deserialize_method!(deserialize_i16());
    deserialize_method!(deserialize_i32());
    deserialize_method!(deserialize_i64());
    deserialize_method!(deserialize_u8());
    deserialize_method!(deserialize_u16());
    deserialize_method!(deserialize_u32());
    deserialize_method!(deserialize_u64());
    deserialize_method!(deserialize_f32());
    deserialize_method!(deserialize_f64());
    deserialize_method!(deserialize_char());
    deserialize_method!(deserialize_str());
    deserialize_method!(deserialize_string());
    deserialize_method!(deserialize_bytes());
    deserialize_method!(deserialize_byte_buf());
    deserialize_method!(deserialize_option());
    deserialize_method!(deserialize_unit());
    deserialize_method!(deserialize_unit_struct(n: &'static str));
    deserialize_method!(deserialize_newtype_struct(n: &'static str));
    deserialize_method!(deserialize_seq());
    deserialize_method!(deserialize_map());
    deserialize_method!(deserialize_tuple(n: usize));
    deserialize_method!(deserialize_tuple_struct(n: &'static str, l: usize));
    deserialize_method!(deserialize_struct(
        n: &'static str,
        f: &'static [&'static str]
    ));
    deserialize_method!(deserialize_enum(
        n: &'static str,
        f: &'static [&'static str]
    ));
    deserialize_method!(deserialize_identifier());
    deserialize_method!(deserialize_ignored_any());

    fn is_human_readable(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub(crate) enum ValueParseStage {
    Signature,
    Value,
    Done,
}

pub(crate) fn deserialize_any<'de, 'sig, 'f, B, D, V>(
    de: D,
    next_char: char,
    visitor: V,
) -> Result<V::Value>
where
    D: de::Deserializer<'de, Error = Error>,
    V: Visitor<'de>,
    B: byteorder::ByteOrder,
{
    match next_char {
        u8::SIGNATURE_CHAR => de.deserialize_u8(visitor),
        bool::SIGNATURE_CHAR => de.deserialize_bool(visitor),
        i16::SIGNATURE_CHAR => de.deserialize_i16(visitor),
        u16::SIGNATURE_CHAR => de.deserialize_u16(visitor),
        i32::SIGNATURE_CHAR => de.deserialize_i32(visitor),
        #[cfg(unix)]
        Fd::SIGNATURE_CHAR => de.deserialize_i32(visitor),
        u32::SIGNATURE_CHAR => de.deserialize_u32(visitor),
        i64::SIGNATURE_CHAR => de.deserialize_i64(visitor),
        u64::SIGNATURE_CHAR => de.deserialize_u64(visitor),
        f64::SIGNATURE_CHAR => de.deserialize_f64(visitor),
        <&str>::SIGNATURE_CHAR | ObjectPath::SIGNATURE_CHAR | Signature::SIGNATURE_CHAR => {
            de.deserialize_str(visitor)
        }
        VARIANT_SIGNATURE_CHAR => de.deserialize_seq(visitor),
        ARRAY_SIGNATURE_CHAR => de.deserialize_seq(visitor),
        STRUCT_SIG_START_CHAR => de.deserialize_seq(visitor),
        #[cfg(feature = "gvariant")]
        MAYBE_SIGNATURE_CHAR => de.deserialize_option(visitor),
        c => Err(de::Error::invalid_value(
            de::Unexpected::Char(c),
            &"a valid signature character",
        )),
    }
}

pub(crate) trait GetDeserializeCommon<'de, 'sig, 'f, B>
where
    B: byteorder::ByteOrder,
{
    fn common_mut<'d>(self) -> &'d mut DeserializerCommon<'de, 'sig, 'f, B>
    where
        Self: 'd;
}

// Enum handling is very generic so it can be here and specific deserializers can use this.
pub(crate) struct Enum<B, D> {
    pub(crate) de: D,
    pub(crate) name: &'static str,
    pub(crate) phantom: PhantomData<B>,
}

impl<'de, 'sig, 'f, B, D> VariantAccess<'de> for Enum<B, D>
where
    B: byteorder::ByteOrder,
    D: de::Deserializer<'de, Error = Error> + GetDeserializeCommon<'de, 'sig, 'f, B>,
{
    type Error = Error;

    fn unit_variant(self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value>
    where
        T: DeserializeSeed<'de>,
    {
        seed.deserialize(self.de)
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        de::Deserializer::deserialize_struct(self.de, self.name, &[], visitor)
    }

    fn struct_variant<V>(self, fields: &'static [&'static str], visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        de::Deserializer::deserialize_struct(self.de, self.name, fields, visitor)
    }
}
