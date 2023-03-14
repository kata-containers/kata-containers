use byteorder::WriteBytesExt;
use serde::{ser, Serialize};
use static_assertions::assert_impl_all;
use std::{
    io::{Seek, Write},
    marker::PhantomData,
    str,
};

#[cfg(unix)]
use std::os::unix::io::RawFd;

#[cfg(feature = "gvariant")]
use crate::gvariant::{self, Serializer as GVSerializer};
use crate::{
    dbus::{self, Serializer as DBusSerializer},
    signature_parser::SignatureParser,
    utils::*,
    Basic, DynamicType, EncodingContext, EncodingFormat, Error, Result, Signature,
};

struct NullWriteSeek;

impl Write for NullWriteSeek {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Seek for NullWriteSeek {
    fn seek(&mut self, _pos: std::io::SeekFrom) -> std::io::Result<u64> {
        Ok(std::u64::MAX) // should never read the return value!
    }
}

/// Calculate the serialized size of `T`.
///
/// # Panics
///
/// This function will panic if the value to serialize contains file descriptors. Use
/// [`serialized_size_fds`] if `T` (potentially) contains FDs.
///
/// # Examples
///
/// ```
/// use zvariant::{EncodingContext, serialized_size};
///
/// let ctxt = EncodingContext::<byteorder::LE>::new_dbus(0);
/// let len = serialized_size(ctxt, "hello world").unwrap();
/// assert_eq!(len, 16);
///
/// let len = serialized_size(ctxt, &("hello world!", 42_u64)).unwrap();
/// assert_eq!(len, 32);
/// ```
///
/// [`serialized_size_fds`]: fn.serialized_size_fds.html
pub fn serialized_size<B, T: ?Sized>(ctxt: EncodingContext<B>, value: &T) -> Result<usize>
where
    B: byteorder::ByteOrder,
    T: Serialize + DynamicType,
{
    let mut null = NullWriteSeek;

    to_writer(&mut null, ctxt, value)
}

/// Calculate the serialized size of `T` that (potentially) contains FDs.
///
/// Returns the serialized size of `T` and the number of FDs.
///
/// This function is not available on Windows.
#[cfg(unix)]
pub fn serialized_size_fds<B, T: ?Sized>(
    ctxt: EncodingContext<B>,
    value: &T,
) -> Result<(usize, usize)>
where
    B: byteorder::ByteOrder,
    T: Serialize + DynamicType,
{
    let mut null = NullWriteSeek;

    let (len, fds) = to_writer_fds(&mut null, ctxt, value)?;
    Ok((len, fds.len()))
}

/// Serialize `T` to the given `writer`.
///
/// This function returns the number of bytes written to the given `writer`.
///
/// # Panics
///
/// This function will panic if the value to serialize contains file descriptors. Use
/// [`to_writer_fds`] if you'd want to potentially pass FDs.
///
/// # Examples
///
/// ```
/// use zvariant::{EncodingContext, from_slice, to_writer};
///
/// let ctxt = EncodingContext::<byteorder::LE>::new_dbus(0);
/// let mut cursor = std::io::Cursor::new(vec![]);
/// to_writer(&mut cursor, ctxt, &42u32).unwrap();
/// let value: u32 = from_slice(cursor.get_ref(), ctxt).unwrap();
/// assert_eq!(value, 42);
/// ```
///
/// [`to_writer_fds`]: fn.to_writer_fds.html
pub fn to_writer<B, W, T: ?Sized>(
    writer: &mut W,
    ctxt: EncodingContext<B>,
    value: &T,
) -> Result<usize>
where
    B: byteorder::ByteOrder,
    W: Write + Seek,
    T: Serialize + DynamicType,
{
    let signature = value.dynamic_signature();

    to_writer_for_signature(writer, ctxt, &signature, value)
}

/// Serialize `T` that (potentially) contains FDs, to the given `writer`.
///
/// This function returns the number of bytes written to the given `writer` and the file descriptor
/// vector, which needs to be transferred via an out-of-band platform specific mechanism.
///
/// This function is not available on Windows.
#[cfg(unix)]
pub fn to_writer_fds<B, W, T: ?Sized>(
    writer: &mut W,
    ctxt: EncodingContext<B>,
    value: &T,
) -> Result<(usize, Vec<RawFd>)>
where
    B: byteorder::ByteOrder,
    W: Write + Seek,
    T: Serialize + DynamicType,
{
    let signature = value.dynamic_signature();

    to_writer_fds_for_signature(writer, ctxt, &signature, value)
}

/// Serialize `T` as a byte vector.
///
/// See [`from_slice`] documentation for an example of how to use this function.
///
/// # Panics
///
/// This function will panic if the value to serialize contains file descriptors. Use
/// [`to_bytes_fds`] if you'd want to potentially pass FDs.
///
/// [`to_bytes_fds`]: fn.to_bytes_fds.html
/// [`from_slice`]: fn.from_slice.html#examples
pub fn to_bytes<B, T: ?Sized>(ctxt: EncodingContext<B>, value: &T) -> Result<Vec<u8>>
where
    B: byteorder::ByteOrder,
    T: Serialize + DynamicType,
{
    let mut cursor = std::io::Cursor::new(vec![]);
    to_writer(&mut cursor, ctxt, value)?;
    Ok(cursor.into_inner())
}

/// Serialize `T` that (potentially) contains FDs, as a byte vector.
///
/// The returned file descriptor needs to be transferred via an out-of-band platform specific
/// mechanism.
///
/// This function is not available on Windows.
#[cfg(unix)]
pub fn to_bytes_fds<B, T: ?Sized>(
    ctxt: EncodingContext<B>,
    value: &T,
) -> Result<(Vec<u8>, Vec<RawFd>)>
where
    B: byteorder::ByteOrder,
    T: Serialize + DynamicType,
{
    let mut cursor = std::io::Cursor::new(vec![]);
    let (_, fds) = to_writer_fds(&mut cursor, ctxt, value)?;
    Ok((cursor.into_inner(), fds))
}

/// Serialize `T` that has the given signature, to the given `writer`.
///
/// Use this function instead of [`to_writer`] if the value being serialized does not implement
/// [`Type`].
///
/// This function returns the number of bytes written to the given `writer`.
///
/// [`to_writer`]: fn.to_writer.html
/// [`Type`]: trait.Type.html
pub fn to_writer_for_signature<B, W, T: ?Sized>(
    writer: &mut W,
    ctxt: EncodingContext<B>,
    signature: &Signature<'_>,
    value: &T,
) -> Result<usize>
where
    B: byteorder::ByteOrder,
    W: Write + Seek,
    T: Serialize,
{
    #[cfg(unix)]
    {
        let (len, fds) = to_writer_fds_for_signature(writer, ctxt, signature, value)?;
        if !fds.is_empty() {
            panic!("can't serialize with FDs")
        }
        Ok(len)
    }

    #[cfg(not(unix))]
    {
        match ctxt.format() {
            EncodingFormat::DBus => {
                let mut ser = DBusSerializer::<B, W>::new(signature, writer, ctxt);
                value.serialize(&mut ser)?;
                Ok(ser.0.bytes_written)
            }
            #[cfg(feature = "gvariant")]
            EncodingFormat::GVariant => {
                let mut ser = GVSerializer::<B, W>::new(signature, writer, ctxt);
                value.serialize(&mut ser)?;
                Ok(ser.0.bytes_written)
            }
        }
    }
}

/// Serialize `T` that (potentially) contains FDs and has the given signature, to the given `writer`.
///
/// Use this function instead of [`to_writer_fds`] if the value being serialized does not implement
/// [`Type`].
///
/// This function returns the number of bytes written to the given `writer` and the file descriptor
/// vector, which needs to be transferred via an out-of-band platform specific mechanism.
///
/// This function is not available on Windows.
///
/// [`to_writer_fds`]: fn.to_writer_fds.html
/// [`Type`]: trait.Type.html
#[cfg(unix)]
pub fn to_writer_fds_for_signature<B, W, T: ?Sized>(
    writer: &mut W,
    ctxt: EncodingContext<B>,
    signature: &Signature<'_>,
    value: &T,
) -> Result<(usize, Vec<RawFd>)>
where
    B: byteorder::ByteOrder,
    W: Write + Seek,
    T: Serialize,
{
    let mut fds = vec![];
    match ctxt.format() {
        EncodingFormat::DBus => {
            let mut ser = DBusSerializer::<B, W>::new(signature, writer, &mut fds, ctxt);
            value.serialize(&mut ser)?;
            Ok((ser.0.bytes_written, fds))
        }
        #[cfg(feature = "gvariant")]
        EncodingFormat::GVariant => {
            let mut ser = GVSerializer::<B, W>::new(signature, writer, &mut fds, ctxt);
            value.serialize(&mut ser)?;
            Ok((ser.0.bytes_written, fds))
        }
    }
}

/// Serialize `T` that has the given signature, to a new byte vector.
///
/// Use this function instead of [`to_bytes`] if the value being serialized does not implement
/// [`Type`]. See [`from_slice_for_signature`] documentation for an example of how to use this
/// function.
///
/// # Panics
///
/// This function will panic if the value to serialize contains file descriptors. Use
/// [`to_bytes_fds_for_signature`] if you'd want to potentially pass FDs.
///
/// [`to_bytes`]: fn.to_bytes.html
/// [`Type`]: trait.Type.html
/// [`from_slice_for_signature`]: fn.from_slice_for_signature.html#examples
pub fn to_bytes_for_signature<B, T: ?Sized>(
    ctxt: EncodingContext<B>,
    signature: &Signature<'_>,
    value: &T,
) -> Result<Vec<u8>>
where
    B: byteorder::ByteOrder,
    T: Serialize,
{
    #[cfg(unix)]
    {
        let (bytes, fds) = to_bytes_fds_for_signature(ctxt, signature, value)?;
        if !fds.is_empty() {
            panic!("can't serialize with FDs")
        }
        Ok(bytes)
    }

    #[cfg(not(unix))]
    {
        let mut cursor = std::io::Cursor::new(vec![]);
        to_writer_for_signature(&mut cursor, ctxt, signature, value)?;
        Ok(cursor.into_inner())
    }
}

/// Serialize `T` that (potentially) contains FDs and has the given signature, to a new byte vector.
///
/// Use this function instead of [`to_bytes_fds`] if the value being serialized does not implement
/// [`Type`].
///
/// Please note that the serialized bytes only contain the indices of the file descriptors from the
/// returned file descriptor vector, which needs to be transferred via an out-of-band platform
/// specific mechanism.
///
/// This function is not available on Windows.
///
/// [`to_bytes_fds`]: fn.to_bytes_fds.html
/// [`Type`]: trait.Type.html
#[cfg(unix)]
pub fn to_bytes_fds_for_signature<B, T: ?Sized>(
    ctxt: EncodingContext<B>,
    signature: &Signature<'_>,
    value: &T,
) -> Result<(Vec<u8>, Vec<RawFd>)>
where
    B: byteorder::ByteOrder,
    T: Serialize,
{
    let mut cursor = std::io::Cursor::new(vec![]);
    let (_, fds) = to_writer_fds_for_signature(&mut cursor, ctxt, signature, value)?;
    Ok((cursor.into_inner(), fds))
}

/// Context for all our serializers and provides shared functionality.
pub(crate) struct SerializerCommon<'ser, 'sig, B, W> {
    pub(crate) ctxt: EncodingContext<B>,
    pub(crate) writer: &'ser mut W,
    pub(crate) bytes_written: usize,
    #[cfg(unix)]
    pub(crate) fds: &'ser mut Vec<RawFd>,

    pub(crate) sig_parser: SignatureParser<'sig>,

    pub(crate) value_sign: Option<Signature<'static>>,

    pub(crate) b: PhantomData<B>,
}

/// Our serialization implementation.
///
/// Using this serializer involves an redirection to the actual serializer. It's best to use the
/// serialization functions, e.g [`to_bytes`] or specific serializers, [`dbus::Serializer`] or
/// [`zvariant::Serializer`].
pub enum Serializer<'ser, 'sig, B, W> {
    DBus(DBusSerializer<'ser, 'sig, B, W>),
    #[cfg(feature = "gvariant")]
    GVariant(GVSerializer<'ser, 'sig, B, W>),
}

assert_impl_all!(Serializer<'_, '_, i32, i32>: Send, Sync, Unpin);

impl<'ser, 'sig, B, W> Serializer<'ser, 'sig, B, W>
where
    B: byteorder::ByteOrder,
    W: Write + Seek,
{
    /// Create a Serializer struct instance.
    pub fn new<'w: 'ser, 'f: 'ser>(
        signature: &Signature<'sig>,
        writer: &'w mut W,
        #[cfg(unix)] fds: &'f mut Vec<RawFd>,
        ctxt: EncodingContext<B>,
    ) -> Self {
        match ctxt.format() {
            #[cfg(feature = "gvariant")]
            EncodingFormat::GVariant => Self::GVariant(GVSerializer::new(
                signature,
                writer,
                #[cfg(unix)]
                fds,
                ctxt,
            )),
            EncodingFormat::DBus => Self::DBus(DBusSerializer::new(
                signature,
                writer,
                #[cfg(unix)]
                fds,
                ctxt,
            )),
        }
    }

    /// Unwrap the `Writer` reference from the `Serializer`.
    #[inline]
    pub fn into_inner(self) -> &'ser mut W {
        match self {
            #[cfg(feature = "gvariant")]
            Self::GVariant(ser) => ser.0.writer,
            Self::DBus(ser) => ser.0.writer,
        }
    }
}

impl<'ser, 'sig, B, W> SerializerCommon<'ser, 'sig, B, W>
where
    B: byteorder::ByteOrder,
    W: Write + Seek,
{
    #[cfg(unix)]
    pub(crate) fn add_fd(&mut self, fd: RawFd) -> u32 {
        if let Some(idx) = self.fds.iter().position(|&x| x == fd) {
            return idx as u32;
        }
        let idx = self.fds.len();
        self.fds.push(fd);

        idx as u32
    }

    pub(crate) fn add_padding(&mut self, alignment: usize) -> Result<usize> {
        let padding = padding_for_n_bytes(self.abs_pos(), alignment);
        if padding > 0 {
            let byte = [0_u8; 1];
            for _ in 0..padding {
                self.write_all(&byte).map_err(Error::Io)?;
            }
        }

        Ok(padding)
    }

    pub(crate) fn prep_serialize_basic<T>(&mut self) -> Result<()>
    where
        T: Basic,
    {
        self.sig_parser.skip_char()?;
        self.add_padding(T::alignment(self.ctxt.format()))?;

        Ok(())
    }

    /// This starts the enum serialization.
    ///
    /// It's up to the caller to do the rest: serialize the variant payload and skip the `).
    pub(crate) fn prep_serialize_enum_variant(&mut self, variant_index: u32) -> Result<()> {
        // Encode enum variants as a struct with first field as variant index
        let signature = self.sig_parser.next_signature()?;
        if self.sig_parser.next_char() != STRUCT_SIG_START_CHAR {
            return Err(Error::SignatureMismatch(
                signature.to_owned(),
                format!("expected `{}`", STRUCT_SIG_START_CHAR),
            ));
        }

        let alignment = alignment_for_signature(&signature, self.ctxt.format());
        self.add_padding(alignment)?;

        // Now serialize the veriant index.
        self.write_u32::<B>(variant_index).map_err(Error::Io)?;

        // Skip the `(`, `u`.
        self.sig_parser.skip_chars(2)?;

        Ok(())
    }

    fn abs_pos(&self) -> usize {
        self.ctxt.position() + self.bytes_written
    }
}

impl<'ser, 'sig, B, W> Write for SerializerCommon<'ser, 'sig, B, W>
where
    B: byteorder::ByteOrder,
    W: Write + Seek,
{
    /// Write `buf` and increment internal bytes written counter.
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.writer.write(buf).map(|n| {
            self.bytes_written += n;

            n
        })
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

macro_rules! serialize_method {
    ($method:ident($($arg:ident: $type:ty),*)) => {
        serialize_method!(; $method($($arg: $type),*) => () =);
    };
    ($($generic:ident),* ; $method:ident($($arg:ident: $type:ty),*)) => {
        serialize_method!($($generic),*; $method($($arg: $type),*) => () =);
    };
    ($($generic:ident),* ; $method:ident($($arg:ident: $type:ty),*) => $ret:ty = $($map:ident)*) => {
        #[inline]
        fn $method<$($generic),*>(self, $($arg: $type),*) -> Result<$ret>
        where
            $($generic: ?Sized + Serialize),*
        {
            match self {
                #[cfg(feature = "gvariant")]
                Serializer::GVariant(ser) => {
                    ser.$method($($arg),*)$(.map($map::GVariant))*
                }
                Serializer::DBus(ser) => {
                    ser.$method($($arg),*)$(.map($map::DBus))*
                }
            }
        }
    }
}

impl<'ser, 'sig, 'b, B, W> ser::Serializer for &'b mut Serializer<'ser, 'sig, B, W>
where
    B: byteorder::ByteOrder,
    W: Write + Seek,
{
    type Ok = ();
    type Error = Error;

    type SerializeSeq = SeqSerializer<'ser, 'sig, 'b, B, W>;
    type SerializeTuple = StructSerializer<'ser, 'sig, 'b, B, W>;
    type SerializeTupleStruct = StructSerializer<'ser, 'sig, 'b, B, W>;
    type SerializeTupleVariant = StructSerializer<'ser, 'sig, 'b, B, W>;
    type SerializeMap = SeqSerializer<'ser, 'sig, 'b, B, W>;
    type SerializeStruct = StructSerializer<'ser, 'sig, 'b, B, W>;
    type SerializeStructVariant = StructSerializer<'ser, 'sig, 'b, B, W>;

    serialize_method!(serialize_bool(b: bool));
    serialize_method!(serialize_i8(i: i8));
    serialize_method!(serialize_i16(i: i16));
    serialize_method!(serialize_i32(i: i32));
    serialize_method!(serialize_i64(i: i64));
    serialize_method!(serialize_u8(u: u8));
    serialize_method!(serialize_u16(u: u16));
    serialize_method!(serialize_u32(u: u32));
    serialize_method!(serialize_u64(u: u64));
    serialize_method!(serialize_f32(f: f32));
    serialize_method!(serialize_f64(f: f64));
    serialize_method!(serialize_char(c: char));
    serialize_method!(serialize_str(s: &str));
    serialize_method!(serialize_bytes(b: &[u8]));
    serialize_method!(T; serialize_some(v: &T));
    serialize_method!(serialize_none());
    serialize_method!(serialize_unit());
    serialize_method!(serialize_unit_struct(s: &'static str));
    serialize_method!(serialize_unit_variant(
        n: &'static str,
        i: u32,
        v: &'static str
    ));
    serialize_method!(T; serialize_newtype_struct(n: &'static str, v: &T));
    serialize_method!(T; serialize_newtype_variant(n: &'static str, i: u32, va: &'static str, v: &T));
    serialize_method!(; serialize_seq(l: Option<usize>) => Self::SerializeSeq = SeqSerializer);
    serialize_method!(; serialize_tuple_variant(
        n: &'static str,
        i: u32,
        v: &'static str,
        l: usize
    ) => Self::SerializeTupleVariant = StructSerializer);
    serialize_method!(;serialize_struct_variant(
        n: &'static str,
        i: u32,
        v: &'static str,
        l: usize
    ) => Self::SerializeStructVariant = StructSerializer);
    serialize_method!(; serialize_tuple(l: usize) => Self::SerializeTuple = StructSerializer);
    serialize_method!(; serialize_tuple_struct(
        n: &'static str,
        l: usize
    ) => Self::SerializeTupleStruct = StructSerializer);
    serialize_method!(; serialize_map(l: Option<usize>) => Self::SerializeMap = SeqSerializer);
    serialize_method!(; serialize_struct(
        n: &'static str,
        l: usize
    ) => Self::SerializeStruct = StructSerializer);

    fn is_human_readable(&self) -> bool {
        false
    }
}

macro_rules! serialize_impl {
    ($trait:ident, $impl:ident, $($method:ident($($arg:ident: $type:ty),*))+) => {
        impl<'ser, 'sig, 'b, B, W> ser::$trait for $impl<'ser, 'sig, 'b, B, W>
        where
            B: byteorder::ByteOrder,
            W: Write + Seek,
        {
            type Ok = ();
            type Error = Error;

            $(
                fn $method<T>(&mut self, $($arg: $type),*) -> Result<()>
                where
                    T: ?Sized + Serialize,
                {
                    match self {
                        #[cfg(feature = "gvariant")]
                        $impl::GVariant(ser) => ser.$method($($arg),*),
                        $impl::DBus(ser) => ser.$method($($arg),*),
                    }
                }
            )*

            fn end(self) -> Result<()> {
                match self {
                    #[cfg(feature = "gvariant")]
                    $impl::GVariant(ser) => ser.end(),
                    $impl::DBus(ser) => ser.end(),
                }
            }
        }
    }
}

#[doc(hidden)]
pub enum SeqSerializer<'ser, 'sig, 'b, B, W> {
    DBus(dbus::SeqSerializer<'ser, 'sig, 'b, B, W>),
    #[cfg(feature = "gvariant")]
    GVariant(gvariant::SeqSerializer<'ser, 'sig, 'b, B, W>),
}

serialize_impl!(SerializeSeq, SeqSerializer, serialize_element(value: &T));

#[doc(hidden)]
pub enum StructSerializer<'ser, 'sig, 'b, B, W> {
    DBus(dbus::StructSeqSerializer<'ser, 'sig, 'b, B, W>),
    #[cfg(feature = "gvariant")]
    GVariant(gvariant::StructSeqSerializer<'ser, 'sig, 'b, B, W>),
}

serialize_impl!(SerializeTuple, StructSerializer, serialize_element(v: &T));
serialize_impl!(
    SerializeTupleStruct,
    StructSerializer,
    serialize_field(v: &T)
);
serialize_impl!(
    SerializeTupleVariant,
    StructSerializer,
    serialize_field(v: &T)
);
serialize_impl!(
    SerializeStruct,
    StructSerializer,
    serialize_field(k: &'static str, v: &T)
);
serialize_impl!(
    SerializeStructVariant,
    StructSerializer,
    serialize_field(k: &'static str, v: &T)
);
serialize_impl!(SerializeMap, SeqSerializer, serialize_key(v: &T) serialize_value(v: &T));
