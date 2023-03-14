use std::marker::PhantomData;

use static_assertions::assert_impl_all;

/// The encoding format.
///
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum EncodingFormat {
    /// [D-Bus](https://dbus.freedesktop.org/doc/dbus-specification.html#message-protocol-marshaling)
    /// format.
    DBus,
    /// [GVariant](https://developer.gnome.org/glib/stable/glib-GVariant.html) format.
    #[cfg(feature = "gvariant")]
    GVariant,
}

assert_impl_all!(EncodingFormat: Send, Sync, Unpin);

impl Default for EncodingFormat {
    fn default() -> Self {
        EncodingFormat::DBus
    }
}

impl std::fmt::Display for EncodingFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EncodingFormat::DBus => write!(f, "D-Bus"),
            #[cfg(feature = "gvariant")]
            EncodingFormat::GVariant => write!(f, "GVariant"),
        }
    }
}

/// The encoding context to use with the [serialization and deserialization] API.
///
/// This type is generic over the [ByteOrder] trait. Moreover, the encoding is dependent on the
/// position of the encoding in the entire message and hence the need to [specify] the byte
/// position of the data being serialized or deserialized. Simply pass `0` if serializing or
/// deserializing to or from the beginning of message, or the preceding bytes end on an 8-byte
/// boundary.
///
/// # Examples
///
/// ```
/// use byteorder::LE;
///
/// use zvariant::EncodingContext as Context;
/// use zvariant::{from_slice, to_bytes};
///
/// let str_vec = vec!["Hello", "World"];
/// let ctxt = Context::<LE>::new_dbus(0);
/// let encoded = to_bytes(ctxt, &str_vec).unwrap();
///
/// // Let's decode the 2nd element of the array only
/// let ctxt = Context::<LE>::new_dbus(14);
/// let decoded: &str = from_slice(&encoded[14..], ctxt).unwrap();
/// assert_eq!(decoded, "World");
/// ```
///
/// [serialization and deserialization]: index.html#functions
/// [ByteOrder]: https://docs.rs/byteorder/1.3.4/byteorder/trait.ByteOrder.html
/// [specify]: #method.new
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct EncodingContext<B> {
    format: EncodingFormat,
    position: usize,

    b: PhantomData<B>,
}

assert_impl_all!(EncodingContext<byteorder::NativeEndian>: Send, Sync, Unpin);

impl<B> EncodingContext<B>
where
    B: byteorder::ByteOrder,
{
    /// Create a new encoding context.
    pub fn new(format: EncodingFormat, position: usize) -> Self {
        Self {
            format,
            position,
            b: PhantomData,
        }
    }

    /// Convenient wrapper for [`new`] to create a context for D-Bus format.
    ///
    /// [`new`]: #method.new
    pub fn new_dbus(position: usize) -> Self {
        Self::new(EncodingFormat::DBus, position)
    }

    /// Convenient wrapper for [`new`] to create a context for GVariant format.
    ///
    /// [`new`]: #method.new
    #[cfg(feature = "gvariant")]
    pub fn new_gvariant(position: usize) -> Self {
        Self::new(EncodingFormat::GVariant, position)
    }

    /// The [`EncodingFormat`] of this context.
    ///
    /// [`EncodingFormat`]: enum.EncodingFormat.html
    pub fn format(self) -> EncodingFormat {
        self.format
    }

    /// The byte position of the value to be encoded or decoded, in the entire message.
    pub fn position(self) -> usize {
        self.position
    }
}
