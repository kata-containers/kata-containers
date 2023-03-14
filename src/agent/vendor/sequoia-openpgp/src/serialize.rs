//! Packet serialization infrastructure.
//!
//! OpenPGP defines a binary representation suitable for storing and
//! communicating OpenPGP data structures (see [Section 3 ff. of RFC
//! 4880]).  Serialization is the process of creating the binary
//! representation.
//!
//!   [Section 3 ff. of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-3
//!
//! There are two interfaces to serialize OpenPGP data.  Which one is
//! applicable depends on whether or not the packet structure is
//! already assembled in memory, with all information already in place
//! (e.g. because it was previously parsed).
//!
//! If it is, you can use the [`Serialize`] or [`SerializeInto`]
//! trait.  Otherwise, please use our [streaming serialization
//! interface].
//!
//!   [streaming serialization interface]: stream
//!
//! # Streaming serialization
//!
//! The [streaming serialization interface] is the preferred way to
//! create OpenPGP messages (see [Section 11.3 of RFC 4880]).  It is
//! ergonomic, yet flexible enough to accommodate most use cases.  It
//! requires little buffering, minimizing the memory footprint of the
//! operation.
//!
//! This example demonstrates how to create the simplest possible
//! OpenPGP message (see [Section 11.3 of RFC 4880]) containing just a
//! literal data packet (see [Section 5.9 of RFC 4880]):
//!
//!   [Section 11.3 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-11.3
//!   [Section 5.9 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-5.9
//!
//! ```
//! # fn main() -> sequoia_openpgp::Result<()> {
//! use std::io::Write;
//! use sequoia_openpgp as openpgp;
//! use openpgp::serialize::stream::{Message, LiteralWriter};
//!
//! let mut o = vec![];
//! {
//!     let message = Message::new(&mut o);
//!     let mut w = LiteralWriter::new(message).build()?;
//!     w.write_all(b"Hello world.")?;
//!     w.finalize()?;
//! }
//! assert_eq!(b"\xcb\x12b\x00\x00\x00\x00\x00Hello world.", o.as_slice());
//! # Ok(()) }
//! ```
//!
//! For a more complete example, see the [streaming examples].
//!
//!   [streaming examples]: stream#examples
//!
//! # Serializing objects
//!
//! The traits [`Serialize`] and [`SerializeInto`] provide a mechanism
//! to serialize OpenPGP data structures.  [`Serialize`] writes to
//! [`io::Write`]rs, while [`SerializeInto`] writes into pre-allocated
//! buffers, computes the size of the serialized representation, and
//! provides a convenient method to create byte vectors with the
//! serialized form.
//!
//!   [`io::Write`]: std::io::Write
//!
//! To prevent accidentally serializing data structures that are not
//! commonly exchanged between OpenPGP implementations, [`Serialize`]
//! and [`SerializeInto`] is only implemented for types like
//! [`Packet`], [`Cert`], and [`Message`], but not for packet bodies
//! like [`Signature`].
//!
//!   [`Packet`]: super::Packet
//!   [`Cert`]: super::Cert
//!   [`Message`]: super::Message
//!   [`Signature`]: crate::packet::Signature
//!
//! This example demonstrates how to serialize a literal data packet
//! (see [Section 5.9 of RFC 4880]):
//!
//! ```
//! # fn main() -> sequoia_openpgp::Result<()> {
//! use sequoia_openpgp as openpgp;
//! use openpgp::packet::{Literal, Packet};
//! use openpgp::serialize::{Serialize, SerializeInto};
//!
//! let mut l = Literal::default();
//! l.set_body(b"Hello world.".to_vec());
//!
//! // Add packet framing.
//! let p = Packet::from(l);
//!
//! // Using Serialize.
//! let mut b = vec![];
//! p.serialize(&mut b)?;
//! assert_eq!(b"\xcb\x12b\x00\x00\x00\x00\x00Hello world.", b.as_slice());
//!
//! // Using SerializeInto.
//! let b = p.to_vec()?;
//! assert_eq!(b"\xcb\x12b\x00\x00\x00\x00\x00Hello world.", b.as_slice());
//! # Ok(()) }
//! ```
//!
//! # Marshalling objects
//!
//! The traits [`Marshal`] and [`MarshalInto`] provide a mechanism to
//! serialize all OpenPGP data structures in this crate, even those
//! not commonly interchanged between OpenPGP implementations.  For
//! example, it allows the serialization of unframed packet bodies:
//!
//!
//! ```
//! # fn main() -> sequoia_openpgp::Result<()> {
//! use sequoia_openpgp as openpgp;
//! use openpgp::packet::Literal;
//! use openpgp::serialize::{Marshal, MarshalInto};
//!
//! let mut l = Literal::default();
//! l.set_body(b"Hello world.".to_vec());
//!
//! // Using Marshal.
//! let mut b = vec![];
//! l.serialize(&mut b)?;
//! assert_eq!(b"b\x00\x00\x00\x00\x00Hello world.", b.as_slice());
//!
//! // Using MarshalInto.
//! let b = l.to_vec()?;
//! assert_eq!(b"b\x00\x00\x00\x00\x00Hello world.", b.as_slice());
//! # Ok(()) }
//! ```

use std::io::{self, Write};
use std::cmp;
use std::convert::{TryFrom, TryInto};
use std::ops::Deref;

use super::*;

mod cert;
pub use self::cert::TSK;
mod cert_armored;
pub mod stream;
use crate::crypto::S2K;
use crate::packet::header::{
    BodyLength,
    CTB,
    CTBNew,
    CTBOld,
};
use crate::packet::signature::subpacket::{
    SubpacketArea, Subpacket, SubpacketValue, SubpacketLength
};
use crate::packet::prelude::*;
use crate::packet::signature::Signature3;
use crate::seal;
use crate::types::{
    RevocationKey,
    Timestamp,
};

// Whether to trace the modules execution (on stderr).
const TRACE : bool = false;

/// Serializes OpenPGP data structures.
///
/// This trait provides the same interface as the [`Marshal`] trait (in
/// fact, it is just a wrapper around that trait), but only data
/// structures that it makes sense to export implement it.
///
///
/// Having a separate trait for data structures that it makes sense to
/// export avoids an easy-to-make and hard-to-debug bug: inadvertently
/// exporting an OpenPGP data structure without any framing
/// information.
///
/// This bug is easy to make, because Rust infers types, which means
/// that it is often not clear from the immediate context exactly what
/// is being serialized.  This bug is hard to debug, because errors
/// parsing data that has been incorrectly exported, are removed from
/// the serialization code.
///
/// The following example shows how to correctly export a revocation
/// certificate.  It should make clear how easy it is to forget to
/// convert a bare signature into an OpenPGP packet before serializing
/// it:
///
/// ```
/// # use sequoia_openpgp as openpgp;
/// # use openpgp::Result;
/// use openpgp::cert::prelude::*;
/// use openpgp::Packet;
/// use openpgp::serialize::Serialize;
///
/// # fn main() -> Result<()> {
/// let (_cert, rev) =
///     CertBuilder::general_purpose(None, Some("alice@example.org"))
///     .generate()?;
/// let rev : Packet = rev.into();
/// # let output = &mut Vec::new();
/// rev.serialize(output)?;
/// # Ok(())
/// # }
/// ```
///
/// Note: if you `use` both `Serialize` and [`Marshal`], then, because
/// they both have the same methods, and all data structures that
/// implement `Serialize` also implement [`Marshal`], you will have to
/// use the Universal Function Call Syntax (UFCS) to call the methods
/// on those objects, for example:
///
/// ```
/// # use sequoia_openpgp as openpgp;
/// # use openpgp::Result;
/// # use openpgp::cert::prelude::*;
/// # use openpgp::Packet;
/// # use openpgp::serialize::Serialize;
/// #
/// # fn main() -> Result<()> {
/// # let (_cert, rev) =
/// #     CertBuilder::general_purpose(None, Some("alice@example.org"))
/// #     .generate()?;
/// # let rev : Packet = rev.into();
/// # let output = &mut Vec::new();
/// Serialize::serialize(&rev, output)?;
/// # Ok(())
/// # }
/// ```
///
/// If you really needed [`Marshal`], we strongly recommend importing it
/// in as small a scope as possible to avoid this, and to avoid
/// accidentally exporting data without the required framing.
pub trait Serialize : Marshal {
    /// Writes a serialized version of the object to `o`.
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        Marshal::serialize(self, o)
    }

    /// Exports a serialized version of the object to `o`.
    ///
    /// This is similar to [`serialize(..)`], with these exceptions:
    ///
    ///   - It is an error to export a [`Signature`] if it is marked
    ///     as non-exportable.
    ///   - When exporting a [`Cert`], non-exportable signatures are
    ///     not exported, and any component bound merely by
    ///     non-exportable signatures is not exported.
    ///
    ///   [`serialize(..)`]: Serialize::serialize
    ///   [`Signature`]: crate::packet::Signature
    ///   [`Cert`]: super::Cert
    fn export(&self, o: &mut dyn std::io::Write) -> Result<()> {
        Marshal::export(self, o)
    }
}

/// Serializes OpenPGP data structures.
///
/// This trait provides the same interface as [`Serialize`], but is
/// implemented for all data structures that can be serialized.
///
///
/// In general, you should prefer the [`Serialize`] trait, as it is only
/// implemented for data structures that are normally exported.  See
/// the documentation for [`Serialize`] for more details.
///
/// # Sealed trait
///
/// This trait is [sealed] and cannot be implemented for types outside this crate.
/// Therefore it can be extended in a non-breaking way.
/// If you want to implement the trait inside the crate
/// you also need to implement the `seal::Sealed` marker trait.
///
/// [sealed]: https://rust-lang.github.io/api-guidelines/future-proofing.html#sealed-traits-protect-against-downstream-implementations-c-sealed
pub trait Marshal: seal::Sealed {
    /// Writes a serialized version of the object to `o`.
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()>;

    /// Exports a serialized version of the object to `o`.
    ///
    /// This is similar to [`serialize(..)`], with these exceptions:
    ///
    ///   - It is an error to export a [`Signature`] if it is marked
    ///     as non-exportable.
    ///   - When exporting a [`Cert`], non-exportable signatures are
    ///     not exported, and any component bound merely by
    ///     non-exportable signatures is not exported.
    ///
    ///   [`serialize(..)`]: Marshal::serialize
    ///   [`Signature`]: crate::packet::Signature
    ///   [`Cert`]: super::Cert
    fn export(&self, o: &mut dyn std::io::Write) -> Result<()> {
        self.serialize(o)
    }
}

/// Serializes OpenPGP data structures into pre-allocated buffers.
///
/// This trait provides the same interface as [`MarshalInto`], but is
/// only implemented for data structures that can be serialized.
///
///
/// In general, you should prefer this trait to [`MarshalInto`], as it
/// is only implemented for data structures that are normally
/// exported.  See the documentation for [`Serialize`] for more details.
///
pub trait SerializeInto : MarshalInto {
    /// Computes the maximal length of the serialized representation.
    ///
    /// # Errors
    ///
    /// If serialization would fail, this function underestimates the
    /// length.
    fn serialized_len(&self) -> usize {
        MarshalInto::serialized_len(self)
    }

    /// Serializes into the given buffer.
    ///
    /// Returns the length of the serialized representation.
    ///
    /// # Errors
    ///
    /// If the length of the given slice is smaller than the maximal
    /// length computed by `serialized_len()`, this function returns
    /// [`Error::InvalidArgument`].
    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        MarshalInto::serialize_into(self, buf)
    }

    /// Serializes the packet to a vector.
    fn to_vec(&self) -> Result<Vec<u8>> {
        MarshalInto::to_vec(self)
    }

    /// Exports into the given buffer.
    ///
    /// This is similar to [`serialize_into(..)`], with these
    /// exceptions:
    ///
    ///   - It is an error to export a [`Signature`] if it is marked
    ///     as non-exportable.
    ///   - When exporting a [`Cert`], non-exportable signatures are
    ///     not exported, and any component bound merely by
    ///     non-exportable signatures is not exported.
    ///
    ///   [`serialize_into(..)`]: SerializeInto::serialize_into
    ///   [`Signature`]: crate::packet::Signature
    ///   [`Cert`]: super::Cert
    ///
    /// Returns the length of the serialized representation.
    ///
    /// # Errors
    ///
    /// If the length of the given slice is smaller than the maximal
    /// length computed by `serialized_len()`, this function returns
    /// [`Error::InvalidArgument`].
    fn export_into(&self, buf: &mut [u8]) -> Result<usize> {
        MarshalInto::export_into(self, buf)
    }

    /// Exports to a vector.
    ///
    /// This is similar to [`to_vec()`], with these exceptions:
    ///
    ///   - It is an error to export a [`Signature`] if it is marked
    ///     as non-exportable.
    ///   - When exporting a [`Cert`], non-exportable signatures are
    ///     not exported, and any component bound merely by
    ///     non-exportable signatures is not exported.
    ///
    ///   [`to_vec()`]: SerializeInto::to_vec()
    ///   [`Signature`]: crate::packet::Signature
    ///   [`Cert`]: super::Cert
    fn export_to_vec(&self) -> Result<Vec<u8>> {
        MarshalInto::export_to_vec(self)
    }
}

/// Serializes OpenPGP data structures into pre-allocated buffers.
///
/// This trait provides the same interface as [`SerializeInto`], but is
/// implemented for all data structures that can be serialized.
///
///
/// In general, you should prefer the [`SerializeInto`] trait, as it is
/// only implemented for data structures that are normally exported.
/// See the documentation for [`Serialize`] for more details.
///
///
/// # Sealed trait
///
/// This trait is [sealed] and cannot be implemented for types outside this crate.
/// Therefore it can be extended in a non-breaking way.
/// If you want to implement the trait inside the crate
/// you also need to implement the `seal::Sealed` marker trait.
///
/// [sealed]: https://rust-lang.github.io/api-guidelines/future-proofing.html#sealed-traits-protect-against-downstream-implementations-c-sealed
pub trait MarshalInto : seal::Sealed {
    /// Computes the maximal length of the serialized representation.
    ///
    /// # Errors
    ///
    /// If serialization would fail, this function underestimates the
    /// length.
    fn serialized_len(&self) -> usize;

    /// Serializes into the given buffer.
    ///
    /// Returns the length of the serialized representation.
    ///
    /// # Errors
    ///
    /// If the length of the given slice is smaller than the maximal
    /// length computed by `serialized_len()`, this function returns
    /// [`Error::InvalidArgument`].
    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize>;

    /// Serializes the packet to a vector.
    fn to_vec(&self) -> Result<Vec<u8>> {
        let mut o = vec![0; self.serialized_len()];
        let len = self.serialize_into(&mut o[..])?;
        vec_truncate(&mut o, len);
        o.shrink_to_fit();
        Ok(o)
    }

    /// Exports into the given buffer.
    ///
    /// This is similar to [`serialize_into(..)`], with these
    /// exceptions:
    ///
    ///   - It is an error to export a [`Signature`] if it is marked
    ///     as non-exportable.
    ///   - When exporting a [`Cert`], non-exportable signatures are
    ///     not exported, and any component bound merely by
    ///     non-exportable signatures is not exported.
    ///
    ///   [`serialize_into(..)`]: MarshalInto::serialize_into
    ///   [`Signature`]: crate::packet::Signature
    ///   [`Cert`]: super::Cert
    ///
    /// Returns the length of the serialized representation.
    ///
    /// # Errors
    ///
    /// If the length of the given slice is smaller than the maximal
    /// length computed by `serialized_len()`, this function returns
    /// [`Error::InvalidArgument`].
    fn export_into(&self, buf: &mut [u8]) -> Result<usize> {
        self.serialize_into(buf)
    }

    /// Exports to a vector.
    ///
    /// This is similar to [`to_vec()`], with these exceptions:
    ///
    ///   - It is an error to export a [`Signature`] if it is marked
    ///     as non-exportable.
    ///   - When exporting a [`Cert`], non-exportable signatures are
    ///     not exported, and any component bound merely by
    ///     non-exportable signatures is not exported.
    ///
    ///   [`to_vec()`]: MarshalInto::to_vec()
    ///   [`Signature`]: crate::packet::Signature
    ///   [`Cert`]: super::Cert
    fn export_to_vec(&self) -> Result<Vec<u8>> {
        let mut o = vec![0; self.serialized_len()];
        let len = self.export_into(&mut o[..])?;
        vec_truncate(&mut o, len);
        o.shrink_to_fit();
        Ok(o)
    }
}

trait NetLength {
    /// Computes the maximal length of the serialized representation
    /// without framing.
    ///
    /// # Errors
    ///
    /// If serialization would fail, this function underestimates the
    /// length.
    fn net_len(&self) -> usize;

    /// Computes the maximal length of the serialized representation
    /// with framing.
    ///
    /// # Errors
    ///
    /// If serialization would fail, this function underestimates the
    /// length.
    fn gross_len(&self) -> usize {
        let net = self.net_len();

        1 // CTB
            + BodyLength::Full(net as u32).serialized_len()
            + net
    }
}

/// Provides a generic implementation for SerializeInto::serialize_into.
///
/// For now, we express SerializeInto using Serialize.  In the future,
/// we may provide implementations not relying on Serialize for a
/// no_std configuration of this crate.
fn generic_serialize_into(o: &dyn Marshal, serialized_len: usize,
                          buf: &mut [u8])
                          -> Result<usize> {
    let buf_len = buf.len();
    let mut cursor = ::std::io::Cursor::new(buf);
    match o.serialize(&mut cursor) {
        Ok(_) => (),
        Err(e) => {
            let short_write =
                if let Some(ioe) = e.downcast_ref::<io::Error>() {
                    ioe.kind() == io::ErrorKind::WriteZero
                } else {
                    false
                };
            return if short_write {
                if buf_len >= serialized_len {
                    let mut b = Vec::new();
                    let need_len = o.serialize(&mut b).map(|_| b.len());
                    panic!("o.serialized_len() = {} underestimated required \
                            space, need {:?}", serialized_len, need_len);
                }
                Err(Error::InvalidArgument(
                    format!("Invalid buffer size, expected {}, got {}",
                            serialized_len, buf_len)).into())
            } else {
                Err(e)
            }
        }
    };
    Ok(cursor.position() as usize)
}


/// Provides a generic implementation for SerializeInto::export_into.
///
/// For now, we express SerializeInto using Serialize.  In the future,
/// we may provide implementations not relying on Serialize for a
/// no_std configuration of this crate.
fn generic_export_into(o: &dyn Marshal, serialized_len: usize,
                       buf: &mut [u8])
                       -> Result<usize> {
    let buf_len = buf.len();
    let mut cursor = ::std::io::Cursor::new(buf);
    match o.export(&mut cursor) {
        Ok(_) => (),
        Err(e) => {
            let short_write =
                if let Some(ioe) = e.downcast_ref::<io::Error>() {
                    ioe.kind() == io::ErrorKind::WriteZero
                } else {
                    false
                };
            return if short_write {
                if buf_len >= serialized_len {
                    let mut b = Vec::new();
                    let need_len = o.serialize(&mut b).map(|_| b.len());
                    panic!("o.serialized_len() = {} underestimated required \
                            space, need {:?}", serialized_len, need_len);
                }
                Err(Error::InvalidArgument(
                    format!("Invalid buffer size, expected {}, got {}",
                            serialized_len, buf_len)).into())
            } else {
                Err(e)
            }
        }
    };
    Ok(cursor.position() as usize)
}

#[test]
fn test_generic_serialize_into() {
    let u = UserID::from("Mr. Pink");
    let mut b = vec![0; u.serialized_len()];
    u.serialize_into(&mut b[..]).unwrap();

    // Short buffer.
    let mut b = vec![0; u.serialized_len() - 1];
    let e = u.serialize_into(&mut b[..]).unwrap_err();
    assert_match!(Some(Error::InvalidArgument(_)) = e.downcast_ref());
}

#[test]
fn test_generic_export_into() {
    let u = UserID::from("Mr. Pink");
    let mut b = vec![0; u.serialized_len()];
    u.export_into(&mut b[..]).unwrap();

    // Short buffer.
    let mut b = vec![0; u.serialized_len() - 1];
    let e = u.export_into(&mut b[..]).unwrap_err();
    assert_match!(Some(Error::InvalidArgument(_)) = e.downcast_ref());
}

fn write_byte(o: &mut dyn std::io::Write, b: u8) -> io::Result<()> {
    o.write_all(&[b])
}

fn write_be_u16(o: &mut dyn std::io::Write, n: u16) -> io::Result<()> {
    o.write_all(&n.to_be_bytes())
}

fn write_be_u32(o: &mut dyn std::io::Write, n: u32) -> io::Result<()> {
    o.write_all(&n.to_be_bytes())
}

// Compute the log2 of an integer.  (This is simply the most
// significant bit.)  Note: log2(0) = -Inf, but this function returns
// log2(0) as 0 (which is the closest number that we can represent).
fn log2(x: u32) -> usize {
    if x == 0 {
        0
    } else {
        31 - x.leading_zeros() as usize
    }
}

#[test]
fn log2_test() {
    for i in 0..32 {
        // eprintln!("log2(1 << {} = {}) = {}", i, 1u32 << i, log2(1u32 << i));
        assert_eq!(log2(1u32 << i), i);
        if i > 0 {
            assert_eq!(log2((1u32 << i) - 1), i - 1);
            assert_eq!(log2((1u32 << i) + 1), i);
        }
    }
}

impl seal::Sealed for BodyLength {}
impl Marshal for BodyLength {
    /// Emits the length encoded for use with new-style CTBs.
    ///
    /// Note: the CTB itself is not emitted.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArgument`] if invoked on
    /// [`BodyLength::Indeterminate`].  If you want to serialize an
    /// old-style length, use [`serialize_old(..)`].
    ///
    /// [`Error::InvalidArgument`]: Error::InvalidArgument
    /// [`serialize_old(..)`]: BodyLength::serialize_old()
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        match self {
            BodyLength::Full(l) => {
                let l = *l;
                if l <= 191 {
                    write_byte(o, l as u8)?;
                } else if l <= 8383 {
                    let v = l - 192;
                    let v = v + (192 << 8);
                    write_be_u16(o, v as u16)?;
                } else {
                    write_byte(o, 0xff)?;
                    write_be_u32(o, l)?;
                }
            },
            BodyLength::Partial(l) => {
                let l = *l;
                if l > 1 << 30 {
                    return Err(Error::InvalidArgument(
                        format!("Partial length too large: {}", l)).into());
                }

                let chunk_size_log2 = log2(l);
                let chunk_size = 1 << chunk_size_log2;

                if l != chunk_size {
                    return Err(Error::InvalidArgument(
                        format!("Not a power of two: {}", l)).into());
                }

                let size_byte = 224 + chunk_size_log2;
                assert!(size_byte < 255);
                write_byte(o, size_byte as u8)?;
            },
            BodyLength::Indeterminate =>
                return Err(Error::InvalidArgument(
                    "Indeterminate lengths are not support for new format packets".
                        into()).into()),
        }

        Ok(())
    }
}

impl MarshalInto for BodyLength {
    fn serialized_len(&self) -> usize {
        match self {
            BodyLength::Full(l) => {
                let l = *l;
                if l <= 191 {
                    1
                } else if l <= 8383 {
                    2
                } else {
                    5
                }
            },
            BodyLength::Partial(_) => 1,
            BodyLength::Indeterminate => 0,
        }
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl BodyLength {
    /// Emits the length encoded for use with old-style CTBs.
    ///
    /// Note: the CTB itself is not emitted.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArgument`] if invoked on
    /// [`BodyLength::Partial`].  If you want to serialize a
    /// new-style length, use [`serialize(..)`].
    ///
    /// [`Error::InvalidArgument`]: Error::InvalidArgument
    /// [`serialize(..)`]: Serialize
    pub fn serialize_old<W: io::Write>(&self, o: &mut W) -> Result<()> {
        // Assume an optimal encoding is desired.
        let mut buffer = Vec::with_capacity(4);
        match self {
            BodyLength::Full(l) => {
                let l = *l;
                match l {
                    // One octet length.
                    // write_byte can't fail for a Vec.
                    0 ..= 0xFF =>
                        write_byte(&mut buffer, l as u8).unwrap(),
                    // Two octet length.
                    0x1_00 ..= 0xFF_FF =>
                        write_be_u16(&mut buffer, l as u16).unwrap(),
                    // Four octet length,
                    _ =>
                        write_be_u32(&mut buffer, l as u32).unwrap(),
                }
            },
            BodyLength::Indeterminate => {},
            BodyLength::Partial(_) =>
                return Err(Error::InvalidArgument(
                    "Partial body lengths are not support for old format packets".
                        into()).into()),
        }

        o.write_all(&buffer)?;
        Ok(())
    }
}

impl seal::Sealed for CTBNew {}
impl Marshal for CTBNew {
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        let tag: u8 = self.tag().into();
        o.write_all(&[0b1100_0000u8 | tag])?;
        Ok(())
    }
}

impl MarshalInto for CTBNew {
    fn serialized_len(&self) -> usize { 1 }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl seal::Sealed for CTBOld {}
impl Marshal for CTBOld {
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        let tag: u8 = self.tag().into();
        let length_type: u8 = self.length_type().into();
        o.write_all(&[0b1000_0000u8 | (tag << 2) | length_type])?;
        Ok(())
    }
}

impl MarshalInto for CTBOld {
    fn serialized_len(&self) -> usize { 1 }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl seal::Sealed for CTB {}
impl Marshal for CTB {
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        match self {
            CTB::New(ref c) => c.serialize(o),
            CTB::Old(ref c) => c.serialize(o),
        }?;
        Ok(())
    }
}

impl MarshalInto for CTB {
    fn serialized_len(&self) -> usize { 1 }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl seal::Sealed for Header {}
impl Marshal for Header {
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        self.ctb().serialize(o)?;
        self.length().serialize(o)?;
        Ok(())
    }
}

impl MarshalInto for Header {
    fn serialized_len(&self) -> usize {
        self.ctb().serialized_len() + self.length().serialized_len()
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl Serialize for KeyID {}
impl seal::Sealed for KeyID {}
impl Marshal for KeyID {
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        let raw = match self {
            KeyID::V4(ref fp) => &fp[..],
            KeyID::Invalid(ref fp) => &fp[..],
        };
        o.write_all(raw)?;
        Ok(())
    }
}

impl SerializeInto for KeyID {}
impl MarshalInto for KeyID {
    fn serialized_len(&self) -> usize {
        match self {
            KeyID::V4(_) => 8,
            KeyID::Invalid(ref fp) => fp.len(),
        }
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl Serialize for Fingerprint {}
impl seal::Sealed for Fingerprint {}
impl Marshal for Fingerprint {
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        o.write_all(self.as_bytes())?;
        Ok(())
    }
}

impl SerializeInto for Fingerprint {}
impl MarshalInto for Fingerprint {
    fn serialized_len(&self) -> usize {
        self.as_bytes().len()
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl seal::Sealed for crypto::mpi::MPI {}
impl Marshal for crypto::mpi::MPI {
    fn serialize(&self, w: &mut dyn std::io::Write) -> Result<()> {
        write_be_u16(w, self.bits() as u16)?;
        w.write_all(self.value())?;
        Ok(())
    }
}

impl MarshalInto for crypto::mpi::MPI {
    fn serialized_len(&self) -> usize {
        2 + self.value().len()
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl seal::Sealed for crypto::mpi::ProtectedMPI {}
impl Marshal for crypto::mpi::ProtectedMPI {
    fn serialize(&self, w: &mut dyn std::io::Write) -> Result<()> {
        write_be_u16(w, self.bits() as u16)?;
        w.write_all(self.value())?;
        Ok(())
    }
}

impl MarshalInto for crypto::mpi::ProtectedMPI {
    fn serialized_len(&self) -> usize {
        2 + self.value().len()
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

/// Writes `buf` into `w` prefixed by the length as u8, bailing out if
/// the length exceeds 256 bytes.
fn write_field_with_u8_size(w: &mut dyn Write, name: &str, buf: &[u8])
                            -> Result<()> {
    w.write_all(&[buf.len().try_into()
                  .map_err(|_| anyhow::Error::from(
                      Error::InvalidArgument(
                          format!("{} exceeds 255 bytes: {:?}",
                                  name, buf))))?])?;
    w.write_all(buf)?;
    Ok(())
}

impl seal::Sealed for crypto::mpi::PublicKey {}
impl Marshal for crypto::mpi::PublicKey {
    fn serialize(&self, w: &mut dyn std::io::Write) -> Result<()> {
        use crate::crypto::mpi::PublicKey::*;

        match self {
            RSA { ref e, ref n } => {
                n.serialize(w)?;
                e.serialize(w)?;
            }

            DSA { ref p, ref q, ref g, ref y } => {
                p.serialize(w)?;
                q.serialize(w)?;
                g.serialize(w)?;
                y.serialize(w)?;
            }

            ElGamal { ref p, ref g, ref y } => {
                p.serialize(w)?;
                g.serialize(w)?;
                y.serialize(w)?;
            }

            EdDSA { ref curve, ref q } => {
                write_field_with_u8_size(w, "Curve's OID", curve.oid())?;
                q.serialize(w)?;
            }

            ECDSA { ref curve, ref q } => {
                write_field_with_u8_size(w, "Curve's OID", curve.oid())?;
                q.serialize(w)?;
            }

            ECDH { ref curve, ref q, hash, sym } => {
                write_field_with_u8_size(w, "Curve's OID", curve.oid())?;
                q.serialize(w)?;
                w.write_all(&[3u8, 1u8, u8::from(*hash), u8::from(*sym)])?;
            }

            Unknown { ref mpis, ref rest } => {
                for mpi in mpis.iter() {
                    mpi.serialize(w)?;
                }
                w.write_all(rest)?;
            }
        }

        Ok(())
    }
}

impl MarshalInto for crypto::mpi::PublicKey {
    fn serialized_len(&self) -> usize {
        use crate::crypto::mpi::PublicKey::*;
        match self {
            RSA { ref e, ref n } => {
                n.serialized_len() + e.serialized_len()
            }

            DSA { ref p, ref q, ref g, ref y } => {
                p.serialized_len() + q.serialized_len() + g.serialized_len()
                    + y.serialized_len()
            }

            ElGamal { ref p, ref g, ref y } => {
                p.serialized_len() + g.serialized_len() + y.serialized_len()
            }

            EdDSA { ref curve, ref q } => {
                1 + curve.oid().len() + q.serialized_len()
            }

            ECDSA { ref curve, ref q } => {
                1 + curve.oid().len() + q.serialized_len()
            }

            ECDH { ref curve, ref q, hash: _, sym: _ } => {
                1 + curve.oid().len() + q.serialized_len() + 4
            }

            Unknown { ref mpis, ref rest } => {
                mpis.iter().map(|mpi| mpi.serialized_len()).sum::<usize>()
                    + rest.len()
            }
        }
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl seal::Sealed for crypto::mpi::SecretKeyMaterial {}
impl Marshal for crypto::mpi::SecretKeyMaterial {
    fn serialize(&self, w: &mut dyn std::io::Write) -> Result<()> {
        use crate::crypto::mpi::SecretKeyMaterial::*;

        match self {
            RSA{ ref d, ref p, ref q, ref u } => {
                d.serialize(w)?;
                p.serialize(w)?;
                q.serialize(w)?;
                u.serialize(w)?;
            }

            DSA{ ref x } => {
                x.serialize(w)?;
            }

            ElGamal{ ref x } => {
                x.serialize(w)?;
            }

            EdDSA{ ref scalar } => {
                scalar.serialize(w)?;
            }

            ECDSA{ ref scalar } => {
                scalar.serialize(w)?;
            }

            ECDH{ ref scalar } => {
                scalar.serialize(w)?;
            }

            Unknown { ref mpis, ref rest } => {
                for mpi in mpis.iter() {
                    mpi.serialize(w)?;
                }
                w.write_all(rest)?;
            }
        }

        Ok(())
    }
}

impl MarshalInto for crypto::mpi::SecretKeyMaterial {
    fn serialized_len(&self) -> usize {
        use crate::crypto::mpi::SecretKeyMaterial::*;
        match self {
            RSA{ ref d, ref p, ref q, ref u } => {
                d.serialized_len() + p.serialized_len() + q.serialized_len()
                    + u.serialized_len()
            }

            DSA{ ref x } => {
                x.serialized_len()
            }

            ElGamal{ ref x } => {
                x.serialized_len()
            }

            EdDSA{ ref scalar } => {
                scalar.serialized_len()
            }

            ECDSA{ ref scalar } => {
                scalar.serialized_len()
            }

            ECDH{ ref scalar } => {
                scalar.serialized_len()
            }

            Unknown { ref mpis, ref rest } => {
                mpis.iter().map(|mpi| mpi.serialized_len()).sum::<usize>()
                    + rest.len()
            }
        }
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl crypto::mpi::SecretKeyMaterial {
    /// Writes this secret key with a checksum to `w`.
    pub fn serialize_with_checksum(
        &self, w: &mut dyn io::Write,
        checksum: crypto::mpi::SecretKeyChecksum)
        -> Result<()>
    {
        // First, the MPIs.
        self.serialize(w)?;

        match checksum {
            crypto::mpi::SecretKeyChecksum::SHA1 => {
                // The checksum is SHA1 over the serialized MPIs.
                let mut hash = HashAlgorithm::SHA1.context().unwrap();
                self.serialize(&mut hash)?;
                let mut digest = [0u8; 20];
                let _ = hash.digest(&mut digest);
                w.write_all(&digest)?;
            },
            crypto::mpi::SecretKeyChecksum::Sum16 => {
                w.write_all(&self.to_vec()?.iter()
                            .fold(0u16, |acc, v| acc.wrapping_add(*v as u16))
                            .to_be_bytes())?;
            },
        }

        Ok(())
    }
}

impl seal::Sealed for crypto::mpi::Ciphertext {}
impl Marshal for crypto::mpi::Ciphertext {
    fn serialize(&self, w: &mut dyn std::io::Write) -> Result<()> {
        use crate::crypto::mpi::Ciphertext::*;

        match self {
            RSA{ ref c } => {
                c.serialize(w)?;
            }

            ElGamal{ ref e, ref c } => {
                e.serialize(w)?;
                c.serialize(w)?;
            }

            ECDH{ ref e, ref key } => {
                e.serialize(w)?;
                write_field_with_u8_size(w, "Key", key)?;
            }

            Unknown { ref mpis, ref rest } => {
                for mpi in mpis.iter() {
                    mpi.serialize(w)?;
                }
                w.write_all(rest)?;
            }
        }

        Ok(())
    }
}

impl MarshalInto for crypto::mpi::Ciphertext {
    fn serialized_len(&self) -> usize {
        use crate::crypto::mpi::Ciphertext::*;
        match self {
            RSA{ ref c } => {
                c.serialized_len()
            }

            ElGamal{ ref e, ref c } => {
                e.serialized_len() + c.serialized_len()
            }

            ECDH{ ref e, ref key } => {
                e.serialized_len() + 1 + key.len()
            }

            Unknown { ref mpis, ref rest } => {
                mpis.iter().map(|mpi| mpi.serialized_len()).sum::<usize>()
                    + rest.len()
            }
        }
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl seal::Sealed for crypto::mpi::Signature {}
impl Marshal for crypto::mpi::Signature {
    fn serialize(&self, w: &mut dyn std::io::Write) -> Result<()> {
        use crate::crypto::mpi::Signature::*;

        match self {
            RSA { ref s } => {
                s.serialize(w)?;
            }
            DSA { ref r, ref s } => {
                r.serialize(w)?;
                s.serialize(w)?;
            }
            ElGamal { ref r, ref s } => {
                r.serialize(w)?;
                s.serialize(w)?;
            }
            EdDSA { ref r, ref s } => {
                r.serialize(w)?;
                s.serialize(w)?;
            }
            ECDSA { ref r, ref s } => {
                r.serialize(w)?;
                s.serialize(w)?;
            }

            Unknown { ref mpis, ref rest } => {
                for mpi in mpis.iter() {
                    mpi.serialize(w)?;
                }
                w.write_all(rest)?;
            }
        }

        Ok(())
    }
}

impl MarshalInto for crypto::mpi::Signature {
    fn serialized_len(&self) -> usize {
        use crate::crypto::mpi::Signature::*;
        match self {
            RSA { ref s } => {
                s.serialized_len()
            }
            DSA { ref r, ref s } => {
                r.serialized_len() + s.serialized_len()
            }
            ElGamal { ref r, ref s } => {
                r.serialized_len() + s.serialized_len()
            }
            EdDSA { ref r, ref s } => {
                r.serialized_len() + s.serialized_len()
            }
            ECDSA { ref r, ref s } => {
                r.serialized_len() + s.serialized_len()
            }

            Unknown { ref mpis, ref rest } => {
                mpis.iter().map(|mpi| mpi.serialized_len()).sum::<usize>()
                    + rest.len()
            }
        }
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl seal::Sealed for S2K {}
impl Marshal for S2K {
    fn serialize(&self, w: &mut dyn std::io::Write) -> Result<()> {
        #[allow(deprecated)]
        match self {
            &S2K::Simple{ hash } => {
                w.write_all(&[0, hash.into()])?;
            }
            &S2K::Salted{ hash, salt } => {
                w.write_all(&[1, hash.into()])?;
                w.write_all(&salt[..])?;
            }
            &S2K::Iterated{ hash, salt, hash_bytes } => {
                w.write_all(&[3, hash.into()])?;
                w.write_all(&salt[..])?;
                w.write_all(&[S2K::encode_count(hash_bytes)?])?;
            }
            S2K::Private { tag, parameters }
            | S2K::Unknown { tag, parameters} => {
                w.write_all(&[*tag])?;
                if let Some(p) = parameters.as_ref() {
                    w.write_all(p)?;
                }
            }
        }

        Ok(())
    }
}

impl MarshalInto for S2K {
    fn serialized_len(&self) -> usize {
        #[allow(deprecated)]
        match self {
            &S2K::Simple{ .. } => 2,
            &S2K::Salted{ .. } => 2 + 8,
            &S2K::Iterated{ .. } => 2 + 8 + 1,
            S2K::Private { parameters, .. }
            | S2K::Unknown { parameters, .. } =>
                1 + parameters.as_ref().map(|p| p.len()).unwrap_or(0),
        }
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl seal::Sealed for Unknown {}
impl Marshal for Unknown {
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        o.write_all(self.body())?;
        Ok(())
    }
}

impl NetLength for Unknown {
    fn net_len(&self) -> usize {
        self.body().len()
    }
}

impl MarshalInto for Unknown {
    fn serialized_len(&self) -> usize {
        self.net_len()
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl seal::Sealed for SubpacketArea {}
impl Marshal for SubpacketArea {
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        for sb in self.iter() {
            sb.serialize(o)?;
        }
        Ok(())
    }
}

impl MarshalInto for SubpacketArea {
    fn serialized_len(&self) -> usize {
        self.iter().map(|sb| sb.serialized_len()).sum()
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        let mut written = 0;
        for sb in self.iter() {
            let n = sb.serialize_into(&mut buf[written..])?;
            written += cmp::min(buf.len() - written, n);
        }
        Ok(written)
    }
}

impl seal::Sealed for Subpacket {}
impl Marshal for Subpacket {
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        let tag = u8::from(self.tag())
            | if self.critical() { 1 << 7 } else { 0 };

        self.length.serialize(o)?;
        o.write_all(&[tag])?;
        self.value().serialize(o)
    }
}

impl MarshalInto for Subpacket {
    fn serialized_len(&self) -> usize {
        self.length.serialized_len() + 1 + self.value().serialized_len()
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl seal::Sealed for SubpacketValue {}
impl Marshal for SubpacketValue {
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        use self::SubpacketValue::*;
        match self {
            SignatureCreationTime(t) =>
                write_be_u32(o, (*t).into())?,
            SignatureExpirationTime(t) =>
                write_be_u32(o, (*t).into())?,
            ExportableCertification(e) =>
                o.write_all(&[if *e { 1 } else { 0 }])?,
            TrustSignature { ref level, ref trust } =>
                o.write_all(&[*level, *trust])?,
            RegularExpression(ref re) => {
                o.write_all(re)?;
                o.write_all(&[0])?;
            },
            Revocable(r) =>
                o.write_all(&[if *r { 1 } else { 0 }])?,
            KeyExpirationTime(t) =>
                write_be_u32(o, (*t).into())?,
            PreferredSymmetricAlgorithms(ref p) =>
                for a in p {
                    o.write_all(&[(*a).into()])?;
                },
            RevocationKey(rk) => rk.serialize(o)?,
            Issuer(ref id) =>
                o.write_all(id.as_bytes())?,
            NotationData(nd) => {
                o.write_all(nd.flags().as_slice())?;
                write_be_u16(o, nd.name().len() as u16)?;
                write_be_u16(o, nd.value().len() as u16)?;
                o.write_all(nd.name().as_bytes())?;
                o.write_all(nd.value())?;
            },
            PreferredHashAlgorithms(ref p) =>
                for a in p {
                    o.write_all(&[(*a).into()])?;
                },
            PreferredCompressionAlgorithms(ref p) =>
                for a in p {
                    o.write_all(&[(*a).into()])?;
                },
            KeyServerPreferences(ref p) =>
                o.write_all(p.as_slice())?,
            PreferredKeyServer(ref p) =>
                o.write_all(p)?,
            PrimaryUserID(p) =>
                o.write_all(&[if *p { 1 } else { 0 }])?,
            PolicyURI(ref p) =>
                o.write_all(p)?,
            KeyFlags(ref f) =>
                o.write_all(f.as_slice())?,
            SignersUserID(ref uid) =>
                o.write_all(uid)?,
            ReasonForRevocation { ref code, ref reason } => {
                o.write_all(&[(*code).into()])?;
                o.write_all(reason)?;
            },
            Features(ref f) =>
                o.write_all(f.as_slice())?,
            SignatureTarget { pk_algo, hash_algo, ref digest } => {
                o.write_all(&[(*pk_algo).into(), (*hash_algo).into()])?;
                o.write_all(digest)?;
            },
            EmbeddedSignature(sig) => sig.serialize(o)?,
            IssuerFingerprint(ref fp) => match fp {
                Fingerprint::V4(_) => {
                    o.write_all(&[4])?;
                    o.write_all(fp.as_bytes())?;
                },
                Fingerprint::V5(_) => {
                    o.write_all(&[5])?;
                    o.write_all(fp.as_bytes())?;
                },
                _ => return Err(Error::InvalidArgument(
                    "Unknown kind of fingerprint".into()).into()),
            }
            PreferredAEADAlgorithms(ref p) =>
                for a in p {
                    o.write_all(&[(*a).into()])?;
                },
            IntendedRecipient(ref fp) => match fp {
                Fingerprint::V4(_) => {
                    o.write_all(&[4])?;
                    o.write_all(fp.as_bytes())?;
                },
                Fingerprint::V5(_) => {
                    o.write_all(&[5])?;
                    o.write_all(fp.as_bytes())?;
                },
                _ => return Err(Error::InvalidArgument(
                    "Unknown kind of fingerprint".into()).into()),
            }
            AttestedCertifications(digests) => {
                for digest in digests {
                    o.write_all(digest)?;
                }
            },
            Unknown { body, .. } =>
                o.write_all(body)?,
        }
        Ok(())
    }
}

impl MarshalInto for SubpacketValue {
    fn serialized_len(&self) -> usize {
        use self::SubpacketValue::*;
        match self {
            SignatureCreationTime(_) => 4,
            SignatureExpirationTime(_) => 4,
            ExportableCertification(_) => 1,
            TrustSignature { .. } => 2,
            RegularExpression(ref re) => re.len() + 1,
            Revocable(_) => 1,
            KeyExpirationTime(_) => 4,
            PreferredSymmetricAlgorithms(ref p) => p.len(),
            RevocationKey(rk) => rk.serialized_len(),
            Issuer(ref id) => (id as &dyn MarshalInto).serialized_len(),
            NotationData(nd) => 4 + 2 + 2 + nd.name().len() + nd.value().len(),
            PreferredHashAlgorithms(ref p) => p.len(),
            PreferredCompressionAlgorithms(ref p) => p.len(),
            KeyServerPreferences(ref p) => p.as_slice().len(),
            PreferredKeyServer(ref p) => p.len(),
            PrimaryUserID(_) => 1,
            PolicyURI(ref p) => p.len(),
            KeyFlags(ref f) => f.as_slice().len(),
            SignersUserID(ref uid) => uid.len(),
            ReasonForRevocation { ref reason, .. } => 1 + reason.len(),
            Features(ref f) => f.as_slice().len(),
            SignatureTarget { ref digest, .. } => 2 + digest.len(),
            EmbeddedSignature(sig) => sig.serialized_len(),
            IssuerFingerprint(ref fp) =>
                1 + (fp as &dyn MarshalInto).serialized_len(),
            PreferredAEADAlgorithms(ref p) => p.len(),
            IntendedRecipient(ref fp) =>
                1 + (fp as &dyn MarshalInto).serialized_len(),
            AttestedCertifications(digests) =>
                digests.iter().map(|d| d.len()).sum(),
            Unknown { body, .. } => body.len(),
        }
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl seal::Sealed for SubpacketLength {}
impl Marshal for SubpacketLength {
    /// Writes the subpacket length to `sink`.
    fn serialize(&self, sink: &mut dyn std::io::Write)
                            -> Result<()> {
        match self.raw {
            Some(ref raw) => sink.write_all(raw)?,
            None => {
                BodyLength::serialize(&BodyLength::Full(self.len() as u32), sink)?
            }
        };

        Ok(())
    }
}

impl MarshalInto for SubpacketLength {
    /// Returns the length of the serialized subpacket length.
    fn serialized_len(&self) -> usize {
        if let Some(ref raw) = self.raw {
            raw.len()
        } else {
            Self::len_optimal_encoding(self.len() as u32)
        }
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}


impl seal::Sealed for RevocationKey {}
impl Marshal for RevocationKey {
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        let (pk_algo, fp) = self.revoker();
        o.write_all(&[self.class(), (pk_algo).into()])?;
        o.write_all(fp.as_bytes())?;
        Ok(())
    }
}

impl MarshalInto for RevocationKey {
    fn serialized_len(&self) -> usize {
        1 + 1 + self.revoker().1.as_bytes().len()
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl seal::Sealed for Signature {}
impl Marshal for Signature {
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        match self {
            Signature::V3(ref s) => s.serialize(o),
            Signature::V4(ref s) => s.serialize(o),
        }
    }

    fn export(&self, o: &mut dyn std::io::Write) -> Result<()> {
        match self {
            Signature::V3(ref s) => s.export(o),
            Signature::V4(ref s) => s.export(o),
        }
    }
}

impl MarshalInto for Signature {
    fn serialized_len(&self) -> usize {
        match self {
            Signature::V3(ref s) => s.serialized_len(),
            Signature::V4(ref s) => s.serialized_len(),
        }
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        match self {
            Signature::V3(ref s) => s.serialize_into(buf),
            Signature::V4(ref s) => s.serialize_into(buf),
        }
    }

    fn export_into(&self, buf: &mut [u8]) -> Result<usize> {
        match self {
            Signature::V3(ref s) => s.export_into(buf),
            Signature::V4(ref s) => s.export_into(buf),
        }
    }

    fn export_to_vec(&self) -> Result<Vec<u8>> {
        match self {
            Signature::V3(ref s) => s.export_to_vec(),
            Signature::V4(ref s) => s.export_to_vec(),
        }
    }
}

impl NetLength for Signature {
    fn net_len(&self) -> usize {
        match self {
            Signature::V3(sig) => sig.net_len(),
            Signature::V4(sig) => sig.net_len(),
        }
    }
}

impl seal::Sealed for Signature3 {}
impl Marshal for Signature3 {
    /// Writes a serialized version of the specified `Signature`
    /// packet to `o`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArgument`] if `self` does not contain
    /// a valid v3 signature.  Because v3 signature support was added
    /// late in the 1.x release cycle, `Signature3` is just a thin
    /// wrapper around a `Signature4`.  As such, it is possible to add
    /// v4 specific data to a `Signature3`.  In general, this isn't a
    /// significnat problem as generating v3 is deprecated.
    ///
    /// [`Error::InvalidArgument`]: Error::InvalidArgument
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        use crate::packet::signature::subpacket::SubpacketTag;

        assert_eq!(self.version(), 3);
        write_byte(o, self.version())?;
        // hashed length.
        write_byte(o, 5)?;
        write_byte(o, self.typ().into())?;
        if let Some(SubpacketValue::SignatureCreationTime(ct))
            = self.hashed_area().subpacket(
                SubpacketTag::SignatureCreationTime)
            .map(|sp| sp.value())
        {
            write_be_u32(o, u32::from(*ct))?;
        } else {
            return Err(Error::InvalidArgument(
                "Invalid v3 signature, missing creation time.".into()).into());
        }

        // Only one signature creation time subpacket is allowed in
        // the hashed area.
        let mut iter = self.hashed_area().iter();
        let _ = iter.next();
        if iter.next().is_some() {
            return Err(Error::InvalidArgument(
                format!("Invalid v3 signature: \
                         subpackets are not allowed: {}",
                        self.hashed_area().iter().map(|sp| {
                            format!("{}: {:?}", sp.tag(), sp.value())
                        }).collect::<Vec<String>>().join(", "))).into());
        }

        if let Some(SubpacketValue::Issuer(keyid))
            = self.unhashed_area().subpacket(SubpacketTag::Issuer)
            .map(|sp| sp.value())
        {
            match keyid {
                KeyID::V4(bytes) => {
                    assert_eq!(bytes.len(), 8);
                    o.write_all(&bytes[..])?;
                }
                KeyID::Invalid(_) => {
                    return Err(Error::InvalidArgument(
                        "Invalid v3 signature, invalid issuer.".into()).into());
                }
            }
        } else {
            return Err(Error::InvalidArgument(
                "Invalid v3 signature, missing issuer.".into()).into());
        }

        // Only one issuer subpacket is allowed in the unhashed area.
        let mut iter = self.unhashed_area().iter();
        let _ = iter.next();
        if iter.next().is_some() {
            return Err(Error::InvalidArgument(
                format!("Invalid v3 signature: \
                         subpackets are not allowed: {}",
                        self.unhashed_area().iter().map(|sp| {
                            format!("{}: {:?}", sp.tag(), sp.value())
                        }).collect::<Vec<String>>().join(", "))).into());
        }

        write_byte(o, self.pk_algo().into())?;
        write_byte(o, self.hash_algo().into())?;

        write_byte(o, self.digest_prefix()[0])?;
        write_byte(o, self.digest_prefix()[1])?;

        self.mpis().serialize(o)?;

        Ok(())
    }

    fn export(&self, o: &mut dyn std::io::Write) -> Result<()> {
        self.exportable()?;
        self.serialize(o)
    }
}

impl NetLength for Signature3 {
    fn net_len(&self) -> usize {
        assert_eq!(self.version(), 3);

        1 // Version.
            + 1 // Hashed length.
            + 1 // Signature type.
            + 4 // Creation time.
            + 8 // Issuer.
            + 1 // PK algorithm.
            + 1 // Hash algorithm.
            + 2 // Hash prefix.
            + self.mpis().serialized_len()
    }
}

impl MarshalInto for Signature3 {
    fn serialized_len(&self) -> usize {
        self.net_len()
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }

    fn export_into(&self, buf: &mut [u8]) -> Result<usize> {
        self.exportable()?;
        self.serialize_into(buf)
    }

    fn export_to_vec(&self) -> Result<Vec<u8>> {
        self.exportable()?;
        self.to_vec()
    }
}

impl seal::Sealed for Signature4 {}
impl Marshal for Signature4 {
    /// Writes a serialized version of the specified `Signature`
    /// packet to `o`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArgument`] if either the hashed-area
    /// or the unhashed-area exceeds the size limit of 2^16.
    ///
    /// [`Error::InvalidArgument`]: Error::InvalidArgument
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        assert_eq!(self.version(), 4);
        write_byte(o, self.version())?;
        write_byte(o, self.typ().into())?;
        write_byte(o, self.pk_algo().into())?;
        write_byte(o, self.hash_algo().into())?;

        let l = self.hashed_area().serialized_len();
        if l > std::u16::MAX as usize {
            return Err(Error::InvalidArgument(
                "Hashed area too large".into()).into());
        }
        write_be_u16(o, l as u16)?;
        self.hashed_area().serialize(o)?;

        let l = self.unhashed_area().serialized_len();
        if l > std::u16::MAX as usize {
            return Err(Error::InvalidArgument(
                "Unhashed area too large".into()).into());
        }
        write_be_u16(o, l as u16)?;
        self.unhashed_area().serialize(o)?;

        write_byte(o, self.digest_prefix()[0])?;
        write_byte(o, self.digest_prefix()[1])?;

        self.mpis().serialize(o)?;

        Ok(())
    }

    fn export(&self, o: &mut dyn std::io::Write) -> Result<()> {
        self.exportable()?;
        self.serialize(o)
    }
}

impl NetLength for Signature4 {
    fn net_len(&self) -> usize {
        assert_eq!(self.version(), 4);

        1 // Version.
            + 1 // Signature type.
            + 1 // PK algorithm.
            + 1 // Hash algorithm.
            + 2 // Hashed area size.
            + self.hashed_area().serialized_len()
            + 2 // Unhashed area size.
            + self.unhashed_area().serialized_len()
            + 2 // Hash prefix.
            + self.mpis().serialized_len()
    }
}

impl MarshalInto for Signature4 {
    fn serialized_len(&self) -> usize {
        self.net_len()
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }

    fn export_into(&self, buf: &mut [u8]) -> Result<usize> {
        self.exportable()?;
        self.serialize_into(buf)
    }

    fn export_to_vec(&self) -> Result<Vec<u8>> {
        self.exportable()?;
        self.to_vec()
    }
}

impl seal::Sealed for OnePassSig {}
impl Marshal for OnePassSig {
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        match self {
            OnePassSig::V3(ref s) => s.serialize(o),
        }
    }
}

impl MarshalInto for OnePassSig {
    fn serialized_len(&self) -> usize {
        match self {
            OnePassSig::V3(ref s) => s.serialized_len(),
        }
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        match self {
            OnePassSig::V3(ref s) => s.serialize_into(buf),
        }
    }
}

impl seal::Sealed for OnePassSig3 {}
impl Marshal for OnePassSig3 {
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        write_byte(o, 3)?; // Version.
        write_byte(o, self.typ().into())?;
        write_byte(o, self.hash_algo().into())?;
        write_byte(o, self.pk_algo().into())?;
        o.write_all(self.issuer().as_bytes())?;
        write_byte(o, self.last_raw())?;

        Ok(())
    }
}

impl NetLength for OnePassSig3 {
    fn net_len(&self) -> usize {
        1 // Version.
            + 1 // Signature type.
            + 1 // Hash algorithm
            + 1 // PK algorithm.
            + 8 // Issuer.
            + 1 // Last.
    }
}

impl MarshalInto for OnePassSig3 {
    fn serialized_len(&self) -> usize {
        self.net_len()
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl<P: key::KeyParts, R: key::KeyRole> seal::Sealed for Key<P, R> {}
impl<P: key::KeyParts, R: key::KeyRole> Marshal for Key<P, R> {
    fn serialize(&self, o: &mut dyn io::Write) -> Result<()> {
        match self {
            Key::V4(ref p) => p.serialize(o),
        }
    }
}

impl<P: key::KeyParts, R: key::KeyRole> MarshalInto for Key<P, R> {
    fn serialized_len(&self) -> usize {
        match self {
            Key::V4(ref p) => p.serialized_len(),
        }
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        match self {
            Key::V4(ref p) => p.serialize_into(buf),
        }
    }
}

impl<P, R> seal::Sealed for Key4<P, R>
    where P: key::KeyParts,
          R: key::KeyRole,
{}
impl<P, R> Marshal for Key4<P, R>
    where P: key::KeyParts,
          R: key::KeyRole,
{
    fn serialize(&self, o: &mut dyn io::Write) -> Result<()> {
        let have_secret_key = P::significant_secrets() && self.has_secret();

        write_byte(o, 4)?; // Version.
        write_be_u32(o, self.creation_time_raw().into())?;
        write_byte(o, self.pk_algo().into())?;
        self.mpis().serialize(o)?;

        if have_secret_key {
            use crypto::mpi::SecretKeyChecksum;
            match self.optional_secret().unwrap() {
                SecretKeyMaterial::Unencrypted(ref u) => u.map(|mpis| -> Result<()> {
                    write_byte(o, 0)?; // S2K usage.
                    mpis.serialize_with_checksum(o, SecretKeyChecksum::Sum16)
                })?,
                SecretKeyMaterial::Encrypted(ref e) => {
                    // S2K usage.
                    write_byte(o, match e.checksum() {
                        Some(SecretKeyChecksum::SHA1) => 254,
                        Some(SecretKeyChecksum::Sum16) => 255,
                        None => return Err(Error::InvalidOperation(
                            "In Key4 packets, encrypted secret keys must be \
                             checksummed".into()).into()),
                    })?;
                    write_byte(o, e.algo().into())?;
                    e.s2k().serialize(o)?;
                    o.write_all(e.raw_ciphertext())?;
                },
            }
        }

        Ok(())
    }
}

impl<P, R> NetLength for Key4<P, R>
    where P: key::KeyParts,
          R: key::KeyRole,
{
    fn net_len(&self) -> usize {
        let have_secret_key = P::significant_secrets() && self.has_secret();

        1 // Version.
            + 4 // Creation time.
            + 1 // PK algo.
            + self.mpis().serialized_len()
            + if have_secret_key {
                1 + match self.optional_secret().unwrap() {
                    SecretKeyMaterial::Unencrypted(ref u) =>
                        u.map(|mpis| mpis.serialized_len())
                        + 2, // Two octet checksum.
                    SecretKeyMaterial::Encrypted(ref e) =>
                        1 + e.s2k().serialized_len()
                        + e.raw_ciphertext().len(),
                }
            } else {
                0
            }
    }
}

impl<P, R> MarshalInto for Key4<P, R>
    where P: key::KeyParts,
          R: key::KeyRole,
{
    fn serialized_len(&self) -> usize {
        self.net_len()
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl seal::Sealed for Marker {}
impl Marshal for Marker {
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        o.write_all(Marker::BODY)?;
        Ok(())
    }
}

impl NetLength for Marker {
    fn net_len(&self) -> usize {
        Marker::BODY.len()
    }
}

impl MarshalInto for Marker {
    fn serialized_len(&self) -> usize {
        self.net_len()
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl seal::Sealed for Trust {}
impl Marshal for Trust {
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        o.write_all(self.value())?;
        Ok(())
    }
}

impl NetLength for Trust {
    fn net_len(&self) -> usize {
        self.value().len()
    }
}

impl MarshalInto for Trust {
    fn serialized_len(&self) -> usize {
        self.net_len()
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl seal::Sealed for UserID {}
impl Marshal for UserID {
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        o.write_all(self.value())?;
        Ok(())
    }
}

impl NetLength for UserID {
    fn net_len(&self) -> usize {
        self.value().len()
    }
}

impl MarshalInto for UserID {
    fn serialized_len(&self) -> usize {
        self.net_len()
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl seal::Sealed for UserAttribute {}
impl Marshal for UserAttribute {
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        o.write_all(self.value())?;
        Ok(())
    }
}

impl NetLength for UserAttribute {
    fn net_len(&self) -> usize {
        self.value().len()
    }
}

impl MarshalInto for UserAttribute {
    fn serialized_len(&self) -> usize {
        self.net_len()
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl seal::Sealed for user_attribute::Subpacket {}
impl Marshal for user_attribute::Subpacket {
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        let body_len = match self {
            user_attribute::Subpacket::Image(image) =>
                image.serialized_len(),
            user_attribute::Subpacket::Unknown(_tag, data) =>
                data.len(),
        };
        BodyLength::Full(1 + body_len as u32).serialize(o)?;
        match self {
            user_attribute::Subpacket::Image(image) => {
                write_byte(o, 1)?;
                image.serialize(o)?;
            },
            user_attribute::Subpacket::Unknown(tag, data) => {
                write_byte(o, *tag)?;
                o.write_all(&data[..])?;
            }
        }

        Ok(())
    }
}

impl MarshalInto for user_attribute::Subpacket {
    fn serialized_len(&self) -> usize {
        let body_len = match self {
            user_attribute::Subpacket::Image(image) =>
                image.serialized_len(),
            user_attribute::Subpacket::Unknown(_tag, data) =>
                data.len(),
        };
        let header_len =
            BodyLength::Full(1 + body_len as u32).serialized_len();
        header_len + 1 + body_len
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl seal::Sealed for user_attribute::Image {}
impl Marshal for user_attribute::Image {
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        const V1HEADER_TOP: [u8; 3] = [0x10, 0x00, 0x01];
        const V1HEADER_PAD: [u8; 12] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        match self {
            user_attribute::Image::JPEG(data) => {
                o.write_all(&V1HEADER_TOP[..])?;
                write_byte(o, 1)?;
                o.write_all(&V1HEADER_PAD[..])?;
                o.write_all(&data[..])?;
            }
            user_attribute::Image::Unknown(tag, data)
            | user_attribute::Image::Private(tag, data) => {
                o.write_all(&V1HEADER_TOP[..])?;
                write_byte(o, *tag)?;
                o.write_all(&V1HEADER_PAD[..])?;
                o.write_all(&data[..])?;
            }
        }

        Ok(())
    }
}

impl MarshalInto for user_attribute::Image {
    fn serialized_len(&self) -> usize {
        const V1HEADER_LEN: usize =
            2     /* Length */
            + 1   /* Version */
            + 1   /* Tag */
            + 12; /* Reserved padding */
        match self {
            user_attribute::Image::JPEG(data)
            | user_attribute::Image::Unknown(_, data)
            | user_attribute::Image::Private(_, data) =>
                V1HEADER_LEN + data.len(),
        }
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl Literal {
    /// Writes the headers of the `Literal` data packet to `o`.
    pub(crate) fn serialize_headers(&self, o: &mut dyn std::io::Write,
                                    write_tag: bool) -> Result<()>
    {
        let filename = if let Some(filename) = self.filename() {
            let len = cmp::min(filename.len(), 255) as u8;
            &filename[..len as usize]
        } else {
            &b""[..]
        };

        let date = if let Some(d) = self.date() {
            Timestamp::try_from(d)?.into()
        } else {
            0
        };

        if write_tag {
            let len = 1 + (1 + filename.len()) + 4
                + self.body().len();
            CTB::new(Tag::Literal).serialize(o)?;
            BodyLength::Full(len as u32).serialize(o)?;
        }
        write_byte(o, self.format().into())?;
        write_byte(o, filename.len() as u8)?;
        o.write_all(filename)?;
        write_be_u32(o, date)?;
        Ok(())
    }
}

impl seal::Sealed for Literal {}
impl Marshal for Literal {
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        let body = self.body();
        if TRACE {
            let prefix = &body[..cmp::min(body.len(), 20)];
            eprintln!("Literal::serialize({}{}, {} bytes)",
                      String::from_utf8_lossy(prefix),
                      if body.len() > 20 { "..." } else { "" },
                      body.len());
        }

        self.serialize_headers(o, false)?;
        o.write_all(body)?;

        Ok(())
    }
}

impl NetLength for Literal {
    fn net_len(&self) -> usize {
        1 + (1 + self.filename().map(|f| f.len()).unwrap_or(0)) + 4
            + self.body().len()
    }
}

impl MarshalInto for Literal {
    fn serialized_len(&self) -> usize {
        self.net_len()
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl seal::Sealed for CompressedData {}
impl Marshal for CompressedData {
    /// Writes a serialized version of the specified `CompressedData`
    /// packet to `o`.
    ///
    /// This function works recursively: if the `CompressedData` packet
    /// contains any packets, they are also serialized.
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        // The streaming serialization framework requires the sink to
        // be Send + Sync, but `o` is not.  Knowing that we create the
        // message here and don't keep the message object around, we
        // can cheat by creating a shim that is Send + Sync.
        struct Shim<'a>(&'a mut dyn std::io::Write);
        impl std::io::Write for Shim<'_> {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.write(buf)
            }
            fn flush(&mut self) -> std::io::Result<()> {
                self.0.flush()
            }
        }
        unsafe impl Send for Shim<'_> {}
        unsafe impl Sync for Shim<'_> {}

        match self.body() {
            Body::Unprocessed(bytes) => {
                if TRACE {
                    eprintln!("CompressedData::serialize(\
                               algo: {}, {} bytes of unprocessed body)",
                              self.algo(), bytes.len());
                }

                o.write_all(&[self.algo().into()])?;
                o.write_all(bytes)?;
            },

            Body::Processed(bytes) => {
                if TRACE {
                    eprintln!("CompressedData::serialize(\
                               algo: {}, {} bytes of processed body)",
                              self.algo(), bytes.len());
                }

                let o = stream::Message::new(Shim(o));
                let mut o = stream::Compressor::new_naked(
                    o, self.algo(), Default::default(), 0)?;
                o.write_all(bytes)?;
                o.finalize()?;
            },

            Body::Structured(children) => {
                if TRACE {
                    eprintln!("CompressedData::serialize(\
                               algo: {}, {:?} children)",
                              self.algo(), children.len());
                }

                let o = stream::Message::new(Shim(o));
                let mut o = stream::Compressor::new_naked(
                    o, self.algo(), Default::default(), 0)?;

                // Serialize the packets.
                for p in children {
                    (p as &dyn Marshal).serialize(&mut o)?;
                }

                o.finalize()?;
            },
        }
        Ok(())
    }
}

impl NetLength for CompressedData {
    fn net_len(&self) -> usize {
        // Worst case, the data gets larger.  Account for that.
        // Experiments suggest that the overhead of compressing random
        // data is worse for BZIP2, but it converges to 20% starting
        // at ~2k of random data.
        let compressed = |l| l + cmp::max(l / 5, 4096);

        match self.body() {
            Body::Unprocessed(bytes) => 1 /* Algo */ + bytes.len(),
            Body::Processed(bytes) => 1 /* Algo */ + compressed(bytes.len()),
            Body::Structured(packets) =>
                1 // Algo
                + compressed(packets.iter().map(|p| {
                    (p as &dyn MarshalInto).serialized_len()
                }).sum::<usize>()),
        }
    }
}

impl MarshalInto for CompressedData {
    /// Computes the maximal length of the serialized representation.
    ///
    /// The size of the serialized compressed data packet is tricky to
    /// predict.  First, it depends on the data being compressed.
    /// Second, we emit partial body encoded data.
    ///
    /// This function tries overestimates the length.  However, it may
    /// happen that `serialize_into()` fails.
    ///
    /// # Errors
    ///
    /// If serialization would fail, this function returns 0.
    fn serialized_len(&self) -> usize {
        self.net_len()
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl seal::Sealed for PKESK {}
impl Marshal for PKESK {
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        match self {
            PKESK::V3(ref p) => p.serialize(o),
        }
    }
}

impl MarshalInto for PKESK {
    fn serialized_len(&self) -> usize {
        match self {
            PKESK::V3(ref p) => p.serialized_len(),
        }
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        match self {
            PKESK::V3(p) =>
                generic_serialize_into(p, MarshalInto::serialized_len(p), buf),
        }
    }
}

impl seal::Sealed for PKESK3 {}
impl Marshal for PKESK3 {
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        write_byte(o, 3)?; // Version.
        (self.recipient() as &dyn Marshal).serialize(o)?;
        write_byte(o, self.pk_algo().into())?;
        self.esk().serialize(o)?;

        Ok(())
    }
}

impl NetLength for PKESK3 {
    fn net_len(&self) -> usize {
        1 // Version.
            + 8 // Recipient's key id.
            + 1 // Algo.
            + self.esk().serialized_len()
    }
}

impl MarshalInto for PKESK3 {
    fn serialized_len(&self) -> usize {
        self.net_len()
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl seal::Sealed for SKESK {}
impl Marshal for SKESK {
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        match self {
            SKESK::V4(ref s) => s.serialize(o),
            SKESK::V5(ref s) => s.serialize(o),
        }
    }
}

impl NetLength for SKESK {
    fn net_len(&self) -> usize {
        match self {
            SKESK::V4(ref s) => s.net_len(),
            SKESK::V5(ref s) => s.net_len(),
        }
    }
}

impl MarshalInto for SKESK {
    fn serialized_len(&self) -> usize {
        match self {
            SKESK::V4(ref s) => s.serialized_len(),
            SKESK::V5(ref s) => s.serialized_len(),
        }
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        match self {
            SKESK::V4(s) =>
                generic_serialize_into(s, MarshalInto::serialized_len(s), buf),
            SKESK::V5(s) =>
                generic_serialize_into(s, MarshalInto::serialized_len(s), buf),
        }
    }
}

impl seal::Sealed for SKESK4 {}
impl Marshal for SKESK4 {
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        write_byte(o, 4)?; // Version.
        write_byte(o, self.symmetric_algo().into())?;
        self.s2k().serialize(o)?;
        o.write_all(self.raw_esk())?;
        Ok(())
    }
}

impl NetLength for SKESK4 {
    fn net_len(&self) -> usize {
        1 // Version.
            + 1 // Algo.
            + self.s2k().serialized_len()
            + self.raw_esk().len()
    }
}

impl MarshalInto for SKESK4 {
    fn serialized_len(&self) -> usize {
        self.net_len()
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl seal::Sealed for SKESK5 {}
impl Marshal for SKESK5 {
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        write_byte(o, 5)?; // Version.
        write_byte(o, self.symmetric_algo().into())?;
        write_byte(o, self.aead_algo().into())?;
        self.s2k().serialize(o)?;
        if let Ok(iv) = self.aead_iv() {
            o.write_all(iv)?;
        }
        o.write_all(self.raw_esk())?;
        o.write_all(self.aead_digest())?;

        Ok(())
    }
}

impl NetLength for SKESK5 {
    fn net_len(&self) -> usize {
        1 // Version.
            + 1 // Cipher algo.
            + 1 // AEAD algo.
            + self.s2k().serialized_len()
            + self.aead_iv().map(|iv| iv.len()).unwrap_or(0)
            + self.raw_esk().len()
            + self.aead_digest().len()
    }
}

impl MarshalInto for SKESK5 {
    fn serialized_len(&self) -> usize {
        self.net_len()
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl seal::Sealed for SEIP {}
impl Marshal for SEIP {
    /// Writes a serialized version of the specified `SEIP`
    /// packet to `o`.
    ///
    /// # Errors
    ///
    /// Returns `Error::InvalidOperation` if this packet has children.
    /// To construct an encrypted message, use
    /// `serialize::stream::Encryptor`.
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        match self.body() {
            Body::Unprocessed(bytes) => {
                o.write_all(&[self.version()])?;
                o.write_all(bytes)?;
                Ok(())
            },
            _ => Err(Error::InvalidOperation(
                "Cannot encrypt, use serialize::stream::Encryptor".into())
                     .into()),
        }
    }
}

impl NetLength for SEIP {
    fn net_len(&self) -> usize {
        match self.body() {
            Body::Unprocessed(bytes) => 1 /* Version */ + bytes.len(),
            _ => 0,
        }
    }
}

impl MarshalInto for SEIP {
    fn serialized_len(&self) -> usize {
        self.net_len()
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl seal::Sealed for MDC {}
impl Marshal for MDC {
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        o.write_all(self.digest())?;
        Ok(())
    }
}

impl NetLength for MDC {
    fn net_len(&self) -> usize {
        20
    }
}

impl MarshalInto for MDC {
    fn serialized_len(&self) -> usize {
        self.net_len()
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl seal::Sealed for AED {}
impl Marshal for AED {
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        match self {
            AED::V1(ref p) => p.serialize(o),
        }
    }
}

impl MarshalInto for AED {
    fn serialized_len(&self) -> usize {
        match self {
            AED::V1(ref p) => p.serialized_len(),
        }
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        match self {
            AED::V1(ref p) => p.serialize_into(buf),
        }
    }
}

impl AED1 {
    /// Writes the headers of the `AED` data packet to `o`.
    fn serialize_headers(&self, o: &mut dyn std::io::Write) -> Result<()> {
        o.write_all(&[1, // Version.
                      self.symmetric_algo().into(),
                      self.aead().into(),
                      self.chunk_size().trailing_zeros() as u8 - 6])?;
        o.write_all(self.iv())?;
        Ok(())
    }
}

impl seal::Sealed for AED1 {}
impl Marshal for AED1 {
    /// Writes a serialized version of the specified `AED`
    /// packet to `o`.
    ///
    /// # Errors
    ///
    /// Returns `Error::InvalidOperation` if this packet has children.
    /// To construct an encrypted message, use
    /// `serialize::stream::Encryptor`.
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        match self.body() {
            Body::Unprocessed(bytes) => {
                self.serialize_headers(o)?;
                o.write_all(bytes)?;
                Ok(())
            },
            _ => Err(Error::InvalidOperation(
                "Cannot encrypt, use serialize::stream::Encryptor".into())
                     .into()),
        }
    }
}

impl NetLength for AED1 {
    fn net_len(&self) -> usize {
        match self.body() {
            Body::Unprocessed(bytes) =>
                4 // Headers.
                + self.iv().len()
                + bytes.len(),
            _ => 0,
        }
    }
}

impl MarshalInto for AED1 {
    fn serialized_len(&self) -> usize {
        self.net_len()
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl Serialize for Packet {}
impl seal::Sealed for Packet {}
impl Marshal for Packet {
    /// Writes a serialized version of the specified `Packet` to `o`.
    ///
    /// This function works recursively: if the packet contains any
    /// packets, they are also serialized.
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        CTB::new(self.tag()).serialize(o)?;

        // Special-case the compressed data packet, because we need
        // the accurate length, and CompressedData::net_len()
        // overestimates the size.
        if let Packet::CompressedData(ref p) = self {
            let mut body = Vec::new();
            p.serialize(&mut body)?;
            BodyLength::Full(body.len() as u32).serialize(o)?;
            o.write_all(&body)?;
            return Ok(());
        }

        BodyLength::Full(self.net_len() as u32).serialize(o)?;
        match self {
            Packet::Unknown(ref p) => p.serialize(o),
            Packet::Signature(ref p) => p.serialize(o),
            Packet::OnePassSig(ref p) => p.serialize(o),
            Packet::PublicKey(ref p) => p.serialize(o),
            Packet::PublicSubkey(ref p) => p.serialize(o),
            Packet::SecretKey(ref p) => p.serialize(o),
            Packet::SecretSubkey(ref p) => p.serialize(o),
            Packet::Marker(ref p) => p.serialize(o),
            Packet::Trust(ref p) => p.serialize(o),
            Packet::UserID(ref p) => p.serialize(o),
            Packet::UserAttribute(ref p) => p.serialize(o),
            Packet::Literal(ref p) => p.serialize(o),
            Packet::CompressedData(_) => unreachable!("handled above"),
            Packet::PKESK(ref p) => p.serialize(o),
            Packet::SKESK(ref p) => p.serialize(o),
            Packet::SEIP(ref p) => p.serialize(o),
            Packet::MDC(ref p) => p.serialize(o),
            Packet::AED(ref p) => p.serialize(o),
        }
    }

    /// Exports a serialized version of the specified `Packet` to `o`.
    ///
    /// This function works recursively: if the packet contains any
    /// packets, they are also serialized.
    fn export(&self, o: &mut dyn std::io::Write) -> Result<()> {
        CTB::new(self.tag()).serialize(o)?;

        // Special-case the compressed data packet, because we need
        // the accurate length, and CompressedData::net_len()
        // overestimates the size.
        if let Packet::CompressedData(ref p) = self {
            let mut body = Vec::new();
            p.export(&mut body)?;
            BodyLength::Full(body.len() as u32).export(o)?;
            o.write_all(&body)?;
            return Ok(());
        }

        BodyLength::Full(self.net_len() as u32).export(o)?;
        match self {
            Packet::Unknown(ref p) => p.export(o),
            Packet::Signature(ref p) => p.export(o),
            Packet::OnePassSig(ref p) => p.export(o),
            Packet::PublicKey(ref p) => p.export(o),
            Packet::PublicSubkey(ref p) => p.export(o),
            Packet::SecretKey(ref p) => p.export(o),
            Packet::SecretSubkey(ref p) => p.export(o),
            Packet::Marker(ref p) => p.export(o),
            Packet::Trust(ref p) => p.export(o),
            Packet::UserID(ref p) => p.export(o),
            Packet::UserAttribute(ref p) => p.export(o),
            Packet::Literal(ref p) => p.export(o),
            Packet::CompressedData(_) => unreachable!("handled above"),
            Packet::PKESK(ref p) => p.export(o),
            Packet::SKESK(ref p) => p.export(o),
            Packet::SEIP(ref p) => p.export(o),
            Packet::MDC(ref p) => p.export(o),
            Packet::AED(ref p) => p.export(o),
        }
    }
}

impl NetLength for Packet {
    fn net_len(&self) -> usize {
        match self {
            Packet::Unknown(ref p) => p.net_len(),
            Packet::Signature(ref p) => p.net_len(),
            Packet::OnePassSig(ref p) => p.net_len(),
            Packet::PublicKey(ref p) => p.net_len(),
            Packet::PublicSubkey(ref p) => p.net_len(),
            Packet::SecretKey(ref p) => p.net_len(),
            Packet::SecretSubkey(ref p) => p.net_len(),
            Packet::Marker(ref p) => p.net_len(),
            Packet::Trust(ref p) => p.net_len(),
            Packet::UserID(ref p) => p.net_len(),
            Packet::UserAttribute(ref p) => p.net_len(),
            Packet::Literal(ref p) => p.net_len(),
            Packet::CompressedData(ref p) => p.net_len(),
            Packet::PKESK(ref p) => p.net_len(),
            Packet::SKESK(ref p) => p.net_len(),
            Packet::SEIP(ref p) => p.net_len(),
            Packet::MDC(ref p) => p.net_len(),
            Packet::AED(ref p) => p.net_len(),
        }
    }
}

impl SerializeInto for Packet {}
impl MarshalInto for Packet {
    fn serialized_len(&self) -> usize {
        self.gross_len()
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }

    fn export_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_export_into(self, MarshalInto::serialized_len(self), buf)
    }
}

/// References packet bodies.
///
/// Like [`openpgp::Packet`], but instead of owning the packet's bodies,
/// they are referenced.  `PacketRef` is only used to serialize packet
/// bodies (like [`packet::Signature`]) encapsulating them in OpenPGP
/// frames.
///
/// [`openpgp::Packet`]: super::Packet
/// [`packet::Signature`]: crate::packet::Signature
#[allow(dead_code)]
#[allow(clippy::upper_case_acronyms)]
enum PacketRef<'a> {
    /// Unknown packet.
    Unknown(&'a packet::Unknown),
    /// Signature packet.
    Signature(&'a packet::Signature),
    /// One pass signature packet.
    OnePassSig(&'a packet::OnePassSig),
    /// Public key packet.
    PublicKey(&'a packet::key::PublicKey),
    /// Public subkey packet.
    PublicSubkey(&'a packet::key::PublicSubkey),
    /// Public/Secret key pair.
    SecretKey(&'a packet::key::SecretKey),
    /// Public/Secret subkey pair.
    SecretSubkey(&'a packet::key::SecretSubkey),
    /// Marker packet.
    Marker(&'a packet::Marker),
    /// Trust packet.
    Trust(&'a packet::Trust),
    /// User ID packet.
    UserID(&'a packet::UserID),
    /// User attribute packet.
    UserAttribute(&'a packet::UserAttribute),
    /// Literal data packet.
    Literal(&'a packet::Literal),
    /// Compressed literal data packet.
    CompressedData(&'a packet::CompressedData),
    /// Public key encrypted data packet.
    PKESK(&'a packet::PKESK),
    /// Symmetric key encrypted data packet.
    SKESK(&'a packet::SKESK),
    /// Symmetric key encrypted, integrity protected data packet.
    SEIP(&'a packet::SEIP),
    /// Modification detection code packet.
    MDC(&'a packet::MDC),
    /// AEAD Encrypted Data Packet.
    AED(&'a packet::AED),
}

impl<'a> PacketRef<'a> {
    /// Returns the `PacketRef's` corresponding OpenPGP tag.
    ///
    /// Tags are explained in [Section 4.3 of RFC 4880].
    ///
    ///   [Section 4.3 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-4.3
    fn tag(&self) -> packet::Tag {
        match self {
            PacketRef::Unknown(packet) => packet.tag(),
            PacketRef::Signature(_) => Tag::Signature,
            PacketRef::OnePassSig(_) => Tag::OnePassSig,
            PacketRef::PublicKey(_) => Tag::PublicKey,
            PacketRef::PublicSubkey(_) => Tag::PublicSubkey,
            PacketRef::SecretKey(_) => Tag::SecretKey,
            PacketRef::SecretSubkey(_) => Tag::SecretSubkey,
            PacketRef::Marker(_) => Tag::Marker,
            PacketRef::Trust(_) => Tag::Trust,
            PacketRef::UserID(_) => Tag::UserID,
            PacketRef::UserAttribute(_) => Tag::UserAttribute,
            PacketRef::Literal(_) => Tag::Literal,
            PacketRef::CompressedData(_) => Tag::CompressedData,
            PacketRef::PKESK(_) => Tag::PKESK,
            PacketRef::SKESK(_) => Tag::SKESK,
            PacketRef::SEIP(_) => Tag::SEIP,
            PacketRef::MDC(_) => Tag::MDC,
            PacketRef::AED(_) => Tag::AED,
        }
    }
}

impl<'a> Serialize for PacketRef<'a> {}
impl<'a> seal::Sealed for PacketRef<'a> {}
impl<'a> Marshal for PacketRef<'a> {
    /// Writes a serialized version of the specified `Packet` to `o`.
    ///
    /// This function works recursively: if the packet contains any
    /// packets, they are also serialized.
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        CTB::new(self.tag()).serialize(o)?;

        // Special-case the compressed data packet, because we need
        // the accurate length, and CompressedData::net_len()
        // overestimates the size.
        if let PacketRef::CompressedData(p) = self {
            let mut body = Vec::new();
            p.serialize(&mut body)?;
            BodyLength::Full(body.len() as u32).serialize(o)?;
            o.write_all(&body)?;
            return Ok(());
        }

        BodyLength::Full(self.net_len() as u32).serialize(o)?;
        match self {
            PacketRef::Unknown(p) => p.serialize(o),
            PacketRef::Signature(p) => p.serialize(o),
            PacketRef::OnePassSig(p) => p.serialize(o),
            PacketRef::PublicKey(p) => p.serialize(o),
            PacketRef::PublicSubkey(p) => p.serialize(o),
            PacketRef::SecretKey(p) => p.serialize(o),
            PacketRef::SecretSubkey(p) => p.serialize(o),
            PacketRef::Marker(p) => p.serialize(o),
            PacketRef::Trust(p) => p.serialize(o),
            PacketRef::UserID(p) => p.serialize(o),
            PacketRef::UserAttribute(p) => p.serialize(o),
            PacketRef::Literal(p) => p.serialize(o),
            PacketRef::CompressedData(_) => unreachable!("handled above"),
            PacketRef::PKESK(p) => p.serialize(o),
            PacketRef::SKESK(p) => p.serialize(o),
            PacketRef::SEIP(p) => p.serialize(o),
            PacketRef::MDC(p) => p.serialize(o),
            PacketRef::AED(p) => p.serialize(o),
        }
    }

    /// Exports a serialized version of the specified `Packet` to `o`.
    ///
    /// This function works recursively: if the packet contains any
    /// packets, they are also serialized.
    fn export(&self, o: &mut dyn std::io::Write) -> Result<()> {
        CTB::new(self.tag()).serialize(o)?;

        // Special-case the compressed data packet, because we need
        // the accurate length, and CompressedData::net_len()
        // overestimates the size.
        if let PacketRef::CompressedData(p) = self {
            let mut body = Vec::new();
            p.export(&mut body)?;
            BodyLength::Full(body.len() as u32).export(o)?;
            o.write_all(&body)?;
            return Ok(());
        }

        BodyLength::Full(self.net_len() as u32).export(o)?;
        match self {
            PacketRef::Unknown(p) => p.export(o),
            PacketRef::Signature(p) => p.export(o),
            PacketRef::OnePassSig(p) => p.export(o),
            PacketRef::PublicKey(p) => p.export(o),
            PacketRef::PublicSubkey(p) => p.export(o),
            PacketRef::SecretKey(p) => p.export(o),
            PacketRef::SecretSubkey(p) => p.export(o),
            PacketRef::Marker(p) => p.export(o),
            PacketRef::Trust(p) => p.export(o),
            PacketRef::UserID(p) => p.export(o),
            PacketRef::UserAttribute(p) => p.export(o),
            PacketRef::Literal(p) => p.export(o),
            PacketRef::CompressedData(_) => unreachable!("handled above"),
            PacketRef::PKESK(p) => p.export(o),
            PacketRef::SKESK(p) => p.export(o),
            PacketRef::SEIP(p) => p.export(o),
            PacketRef::MDC(p) => p.export(o),
            PacketRef::AED(p) => p.export(o),
        }
    }
}

impl<'a> NetLength for PacketRef<'a> {
    fn net_len(&self) -> usize {
        match self {
            PacketRef::Unknown(p) => p.net_len(),
            PacketRef::Signature(p) => p.net_len(),
            PacketRef::OnePassSig(p) => p.net_len(),
            PacketRef::PublicKey(p) => p.net_len(),
            PacketRef::PublicSubkey(p) => p.net_len(),
            PacketRef::SecretKey(p) => p.net_len(),
            PacketRef::SecretSubkey(p) => p.net_len(),
            PacketRef::Marker(p) => p.net_len(),
            PacketRef::Trust(p) => p.net_len(),
            PacketRef::UserID(p) => p.net_len(),
            PacketRef::UserAttribute(p) => p.net_len(),
            PacketRef::Literal(p) => p.net_len(),
            PacketRef::CompressedData(p) => p.net_len(),
            PacketRef::PKESK(p) => p.net_len(),
            PacketRef::SKESK(p) => p.net_len(),
            PacketRef::SEIP(p) => p.net_len(),
            PacketRef::MDC(p) => p.net_len(),
            PacketRef::AED(p) => p.net_len(),
        }
    }
}

impl<'a> SerializeInto for PacketRef<'a> {}
impl<'a> MarshalInto for PacketRef<'a> {
    fn serialized_len(&self) -> usize {
        self.gross_len()
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }

    fn export_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_export_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl Serialize for PacketPile {}
impl seal::Sealed for PacketPile {}
impl Marshal for PacketPile {
    /// Writes a serialized version of the specified `PacketPile` to `o`.
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        for p in self.children() {
            (p as &dyn Marshal).serialize(o)?;
        }

        Ok(())
    }

    /// Exports a serialized version of the specified `PacketPile` to `o`.
    fn export(&self, o: &mut dyn std::io::Write) -> Result<()> {
        for p in self.children() {
            (p as &dyn Marshal).export(o)?;
        }

        Ok(())
    }
}

impl SerializeInto for PacketPile {}
impl MarshalInto for PacketPile {
    fn serialized_len(&self) -> usize {
        self.children().map(|p| {
            (p as &dyn MarshalInto).serialized_len()
        }).sum()
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_serialize_into(self, MarshalInto::serialized_len(self), buf)
    }

    fn export_into(&self, buf: &mut [u8]) -> Result<usize> {
        generic_export_into(self, MarshalInto::serialized_len(self), buf)
    }
}

impl Serialize for Message {}
impl seal::Sealed for Message {}
impl Marshal for Message {
    /// Writes a serialized version of the specified `Message` to `o`.
    fn serialize(&self, o: &mut dyn std::io::Write) -> Result<()> {
        (self.deref() as &dyn Marshal).serialize(o)
    }
}

impl SerializeInto for Message {}
impl MarshalInto for Message {
    fn serialized_len(&self) -> usize {
        (self.deref() as &dyn MarshalInto).serialized_len()
    }

    fn serialize_into(&self, buf: &mut [u8]) -> Result<usize> {
        (self.deref() as &dyn MarshalInto).serialize_into(buf)
    }

    fn export_into(&self, buf: &mut [u8]) -> Result<usize> {
        (self.deref() as &dyn MarshalInto).export_into(buf)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::types::CompressionAlgorithm;
    use crate::parse::to_unknown_packet;
    use crate::parse::PacketParserBuilder;
    use crate::parse::Parse;

    // A convenient function to dump binary data to stdout.
    fn binary_pp(data: &[u8]) -> String {
        let mut output = Vec::new();
        crate::fmt::hex::Dumper::new(&mut output, "")
            .write_ascii(data).unwrap();
        // We know the content is valid UTF-8.
        String::from_utf8(output).unwrap()
    }

    // Does a bit-wise comparison of two packets ignoring the CTB
    // format, the body length encoding, and whether partial body
    // length encoding was used.
    fn packets_bitwise_compare(filename: &str, packet: &Packet,
                               expected: &[u8], got: &[u8]) {
        let expected = to_unknown_packet(expected).unwrap();
        let got = to_unknown_packet(got).unwrap();

        let expected_body = expected.body();
        let got_body = got.body();

        let mut fail = false;
        if expected.tag() != got.tag() {
            eprintln!("Expected a {:?}, got a {:?}", expected.tag(), got.tag());
            fail = true;
        }
        if expected_body != got_body {
            eprintln!("Packet contents don't match (for {}):",
                      filename);
            eprintln!("Expected ({} bytes):\n{}",
                      expected_body.len(), binary_pp(expected_body));
            eprintln!("Got ({} bytes):\n{}",
                      got_body.len(), binary_pp(got_body));
            eprintln!("Packet: {:#?}", packet);
            fail = true;
        }
        if fail {
            panic!("Packets don't match (for {}).", filename);
        }
    }

    #[test]
    fn serialize_test_1() {
        // Given a packet in serialized form:
        //
        // - Parse and reserialize it;
        //
        // - Do a bitwise comparison (modulo the body length encoding)
        //   of the original and reserialized data.
        //
        // Note: This test only works on messages with a single packet.
        //
        // Note: This test does not work with non-deterministic
        // packets, like compressed data packets, since the serialized
        // forms may be different.

        let filenames = [
            "literal-mode-b.gpg",
            "literal-mode-t-partial-body.gpg",

            "sig.gpg",

            "public-key-bare.gpg",
            "public-subkey-bare.gpg",
            "userid-bare.gpg",

            "s2k/mode-0-password-1234.gpg",
            "s2k/mode-0-password-1234.gpg",
            "s2k/mode-1-password-123456-1.gpg",
            "s2k/mode-1-password-foobar-2.gpg",
            "s2k/mode-3-aes128-password-13-times-0123456789.gpg",
            "s2k/mode-3-aes192-password-123.gpg",
            "s2k/mode-3-encrypted-key-password-bgtyhn.gpg",
            "s2k/mode-3-password-9876-2.gpg",
            "s2k/mode-3-password-qwerty-1.gpg",
            "s2k/mode-3-twofish-password-13-times-0123456789.gpg",
        ];

        for filename in filenames.iter() {
            // 1. Read the message byte stream into a local buffer.
            let data = crate::tests::message(filename);

            // 2. Parse the message.
            let pile = PacketPile::from_bytes(data).unwrap();

            // The following test only works if the message has a
            // single top-level packet.
            assert_eq!(pile.children().len(), 1);

            // 3. Serialize the packet it into a local buffer.
            let p = pile.descendants().next().unwrap();
            let mut buffer = Vec::new();
            match p {
                Packet::Literal(_) | Packet::Signature(_)
                    | Packet::PublicKey(_) | Packet::PublicSubkey(_)
                    | Packet::UserID(_) | Packet::SKESK(_) => (),
                p => {
                    panic!("Didn't expect a {:?} packet.", p.tag());
                },
            }
            (p as &dyn Marshal).serialize(&mut buffer).unwrap();

            // 4. Modulo the body length encoding, check that the
            // reserialized content is identical to the original data.
            packets_bitwise_compare(filename, p, data, &buffer[..]);
        }
    }

    #[test]
    fn serialize_test_1_unknown() {
        // This is an variant of serialize_test_1 that tests the
        // unknown packet serializer.
        let filenames = [
            "compressed-data-algo-1.gpg",
            "compressed-data-algo-2.gpg",
            "compressed-data-algo-3.gpg",
            "recursive-2.gpg",
            "recursive-3.gpg",
        ];

        for filename in filenames.iter() {
            // 1. Read the message byte stream into a local buffer.
            let data = crate::tests::message(filename);

            // 2. Parse the message.
            let u = Packet::Unknown(to_unknown_packet(data).unwrap());

            // 3. Serialize the packet it into a local buffer.
            let data2 = (&u as &dyn MarshalInto).to_vec().unwrap();

            // 4. Modulo the body length encoding, check that the
            // reserialized content is identical to the original data.
            packets_bitwise_compare(filename, &u, data, &data2[..]);
        }

    }

    #[test]
    fn serialize_test_2() {
        // Given a packet in serialized form:
        //
        // - Parse, reserialize, and reparse it;
        //
        // - Compare the messages.
        //
        // Note: This test only works on messages with a single packet
        // top-level packet.
        //
        // Note: serialize_test_1 is a better test, because it
        // compares the serialized data, but serialize_test_1 doesn't
        // work if the content is non-deterministic.
        let filenames = [
            "compressed-data-algo-1.gpg",
            "compressed-data-algo-2.gpg",
            "compressed-data-algo-3.gpg",
            "recursive-2.gpg",
            "recursive-3.gpg",
        ];

        for filename in filenames.iter() {
            eprintln!("{}...", filename);

            // 1. Read the message into a local buffer.
            let data = crate::tests::message(filename);

            // 2. Do a shallow parse of the message.  In other words,
            // never recurse so that the resulting message only
            // contains the top-level packets.  Any containers will
            // have their raw content stored in packet.content.
            let pile = PacketParserBuilder::from_bytes(data).unwrap()
                .max_recursion_depth(0)
                .buffer_unread_content()
                //.trace()
                .into_packet_pile().unwrap();

            // 3. Get the first packet.
            let po = pile.descendants().next();
            if let Some(&Packet::CompressedData(ref cd)) = po {
                if ! cd.algo().is_supported() {
                    eprintln!("Skipping {} because {} is not supported.",
                              filename, cd.algo());
                    continue;
                }

                // 4. Serialize the container.
                let buffer =
                    (&Packet::CompressedData(cd.clone()) as &dyn MarshalInto)
                        .to_vec().unwrap();

                // 5. Reparse it.
                let pile2 = PacketParserBuilder::from_bytes(&buffer[..]).unwrap()
                    .max_recursion_depth(0)
                    .buffer_unread_content()
                    //.trace()
                    .into_packet_pile().unwrap();

                // 6. Make sure the original message matches the
                // serialized and reparsed message.
                if pile != pile2 {
                    eprintln!("Orig:");
                    let p = pile.children().next().unwrap();
                    eprintln!("{:?}", p);
                    let body = p.processed_body().unwrap();
                    eprintln!("Body: {}", body.len());
                    eprintln!("{}", binary_pp(body));

                    eprintln!("Reparsed:");
                    let p = pile2.children().next().unwrap();
                    eprintln!("{:?}", p);
                    let body = p.processed_body().unwrap();
                    eprintln!("Body: {}", body.len());
                    eprintln!("{}", binary_pp(body));

                    assert_eq!(pile, pile2);
                }
            } else {
                panic!("Expected a compressed data data packet.");
            }
        }
    }

    // Create some crazy nesting structures, serialize the messages,
    // reparse them, and make sure we get the same result.
    #[test]
    fn serialize_test_3() {
        use crate::types::DataFormat::Text as T;

        // serialize_test_1 and serialize_test_2 parse a byte stream.
        // This tests creates the message, and then serializes and
        // reparses it.

        let mut messages = Vec::new();

        // 1: CompressedData(CompressedData { algo: 0 })
        //  1: Literal(Literal { body: "one (3 bytes)" })
        //  2: Literal(Literal { body: "two (3 bytes)" })
        // 2: Literal(Literal { body: "three (5 bytes)" })
        let mut one = Literal::new(T);
        one.set_body(b"one".to_vec());
        let mut two = Literal::new(T);
        two.set_body(b"two".to_vec());
        let mut three = Literal::new(T);
        three.set_body(b"three".to_vec());
        let mut four = Literal::new(T);
        four.set_body(b"four".to_vec());
        let mut five = Literal::new(T);
        five.set_body(b"five".to_vec());
        let mut six = Literal::new(T);
        six.set_body(b"six".to_vec());

        let mut top_level = Vec::new();
        top_level.push(
            CompressedData::new(CompressionAlgorithm::Uncompressed)
                .push(one.clone().into())
                .push(two.clone().into())
                .into());
        top_level.push(three.clone().into());
        messages.push(top_level);

        // 1: CompressedData(CompressedData { algo: 0 })
        //  1: CompressedData(CompressedData { algo: 0 })
        //   1: Literal(Literal { body: "one (3 bytes)" })
        //   2: Literal(Literal { body: "two (3 bytes)" })
        //  2: CompressedData(CompressedData { algo: 0 })
        //   1: Literal(Literal { body: "three (5 bytes)" })
        //   2: Literal(Literal { body: "four (4 bytes)" })
        let mut top_level = Vec::new();
        top_level.push(
            CompressedData::new(CompressionAlgorithm::Uncompressed)
                .push(CompressedData::new(CompressionAlgorithm::Uncompressed)
                      .push(one.clone().into())
                      .push(two.clone().into())
                      .into())
                .push(CompressedData::new(CompressionAlgorithm::Uncompressed)
                      .push(three.clone().into())
                      .push(four.clone().into())
                      .into())
                .into());
        messages.push(top_level);

        // 1: CompressedData(CompressedData { algo: 0 })
        //  1: CompressedData(CompressedData { algo: 0 })
        //   1: CompressedData(CompressedData { algo: 0 })
        //    1: CompressedData(CompressedData { algo: 0 })
        //     1: Literal(Literal { body: "one (3 bytes)" })
        //     2: Literal(Literal { body: "two (3 bytes)" })
        //  2: CompressedData(CompressedData { algo: 0 })
        //   1: CompressedData(CompressedData { algo: 0 })
        //    1: Literal(Literal { body: "three (5 bytes)" })
        //   2: Literal(Literal { body: "four (4 bytes)" })
        let mut top_level = Vec::new();
        top_level.push(
            CompressedData::new(CompressionAlgorithm::Uncompressed)
                .push(CompressedData::new(CompressionAlgorithm::Uncompressed)
                    .push(CompressedData::new(CompressionAlgorithm::Uncompressed)
                        .push(CompressedData::new(CompressionAlgorithm::Uncompressed)
                            .push(one.clone().into())
                            .push(two.clone().into())
                            .into())
                        .into())
                    .into())
                .push(CompressedData::new(CompressionAlgorithm::Uncompressed)
                    .push(CompressedData::new(CompressionAlgorithm::Uncompressed)
                        .push(three.clone().into())
                        .into())
                    .push(four.clone().into())
                    .into())
                .into());
        messages.push(top_level);

        // 1: CompressedData(CompressedData { algo: 0 })
        //  1: Literal(Literal { body: "one (3 bytes)" })
        //  2: Literal(Literal { body: "two (3 bytes)" })
        // 2: Literal(Literal { body: "three (5 bytes)" })
        // 3: Literal(Literal { body: "four (4 bytes)" })
        // 4: CompressedData(CompressedData { algo: 0 })
        //  1: Literal(Literal { body: "five (4 bytes)" })
        //  2: Literal(Literal { body: "six (3 bytes)" })
        let mut top_level = Vec::new();
        top_level.push(
            CompressedData::new(CompressionAlgorithm::Uncompressed)
                .push(one.clone().into())
                .push(two.clone().into())
                .into());
        top_level.push(
            three.clone().into());
        top_level.push(
            four.clone().into());
        top_level.push(
            CompressedData::new(CompressionAlgorithm::Uncompressed)
                .push(five.into())
                .push(six.into())
                .into());
        messages.push(top_level);

        // 1: UserID(UserID { value: "Foo" })
        let mut top_level = Vec::new();
        let uid = UserID::from("Foo");
        top_level.push(uid.into());
        messages.push(top_level);

        for m in messages.into_iter() {
            // 1. The message.
            let pile = PacketPile::from(m);

            pile.pretty_print();

            // 2. Serialize the message into a buffer.
            let mut buffer = Vec::new();
            (&pile as &dyn Marshal).serialize(&mut buffer).unwrap();

            // 3. Reparse it.
            let pile2 = PacketParserBuilder::from_bytes(&buffer[..]).unwrap()
                //.trace()
                .buffer_unread_content()
                .into_packet_pile().unwrap();

            // 4. Compare the messages.
            if pile != pile2 {
                eprintln!("ORIG...");
                pile.pretty_print();
                eprintln!("REPARSED...");
                pile2.pretty_print();
                panic!("Reparsed packet does not match original packet!");
            }
        }
    }

    #[test]
    fn body_length_edge_cases() {
        {
            let mut buf = vec![];
            BodyLength::Full(0).serialize(&mut buf).unwrap();
            assert_eq!(&buf[..], &b"\x00"[..]);
        }

        {
            let mut buf = vec![];
            BodyLength::Full(1).serialize(&mut buf).unwrap();
            assert_eq!(&buf[..], &b"\x01"[..]);
        }
        {
            let mut buf = vec![];
            BodyLength::Full(191).serialize(&mut buf).unwrap();
            assert_eq!(&buf[..], &b"\xbf"[..]);
        }
        {
            let mut buf = vec![];
            BodyLength::Full(192).serialize(&mut buf).unwrap();
            assert_eq!(&buf[..], &b"\xc0\x00"[..]);
        }
        {
            let mut buf = vec![];
            BodyLength::Full(193).serialize(&mut buf).unwrap();
            assert_eq!(&buf[..], &b"\xc0\x01"[..]);
        }
        {
            let mut buf = vec![];
            BodyLength::Full(8383).serialize(&mut buf).unwrap();
            assert_eq!(&buf[..], &b"\xdf\xff"[..]);
        }
        {
            let mut buf = vec![];
            BodyLength::Full(8384).serialize(&mut buf).unwrap();
            assert_eq!(&buf[..], &b"\xff\x00\x00\x20\xc0"[..]);
        }
        {
            let mut buf = vec![];
            BodyLength::Full(0xffffffff).serialize(&mut buf).unwrap();
            assert_eq!(&buf[..], &b"\xff\xff\xff\xff\xff"[..]);
        }
    }

    #[test]
    fn export_signature() {
        use crate::cert::prelude::*;

        let (cert, _) = CertBuilder::new().generate().unwrap();
        let mut keypair = cert.primary_key().key().clone().parts_into_secret()
            .unwrap().into_keypair().unwrap();
        let uid = UserID::from("foo");

        // Make a signature w/o an exportable certification subpacket.
        let sig = uid.bind(
            &mut keypair, &cert,
            signature::SignatureBuilder::new(SignatureType::GenericCertification))
            .unwrap();

        // The signature is exportable.  Try to export it in
        // various ways.
        sig.export(&mut Vec::new()).unwrap();
        sig.export_into(&mut vec![0; sig.serialized_len()]).unwrap();
        sig.export_to_vec().unwrap();
        (&PacketRef::Signature(&sig) as &dyn Marshal)
            .export(&mut Vec::new()).unwrap();
        (&PacketRef::Signature(&sig) as &dyn MarshalInto).export_into(
            &mut vec![0; (&PacketRef::Signature(&sig) as &dyn MarshalInto)
                             .serialized_len()]).unwrap();
        (&PacketRef::Signature(&sig) as &dyn MarshalInto)
            .export_to_vec().unwrap();
        let p = Packet::Signature(sig);
        (&p as &dyn Marshal).export(&mut Vec::new()).unwrap();
        (&p as &dyn MarshalInto)
            .export_into(
                &mut vec![0; (&p as &dyn MarshalInto).serialized_len()])
            .unwrap();
        (&p as &dyn MarshalInto).export_to_vec().unwrap();
        let pp = PacketPile::from(vec![p]);
        (&pp as &dyn Marshal).export(&mut Vec::new()).unwrap();
        (&pp as &dyn MarshalInto)
            .export_into(
                &mut vec![0; (&pp as &dyn MarshalInto).serialized_len()])
            .unwrap();
        (&pp as &dyn MarshalInto).export_to_vec().unwrap();

        // Make a signature that is explicitly marked as exportable.
        let sig = uid.bind(
            &mut keypair, &cert,
            signature::SignatureBuilder::new(SignatureType::GenericCertification)
                .set_exportable_certification(true).unwrap()).unwrap();

        // The signature is exportable.  Try to export it in
        // various ways.
        sig.export(&mut Vec::new()).unwrap();
        sig.export_into(&mut vec![0; sig.serialized_len()]).unwrap();
        sig.export_to_vec().unwrap();
        (&PacketRef::Signature(&sig) as &dyn Marshal)
            .export(&mut Vec::new()).unwrap();
        (&PacketRef::Signature(&sig) as &dyn MarshalInto)
            .export_into(
                &mut vec![0; (&PacketRef::Signature(&sig)
                              as &dyn MarshalInto).serialized_len()])
            .unwrap();
        (&PacketRef::Signature(&sig) as &dyn MarshalInto)
            .export_to_vec().unwrap();
        let p = Packet::Signature(sig);
        (&p as &dyn Marshal).export(&mut Vec::new()).unwrap();
        (&p as &dyn MarshalInto)
            .export_into(
                &mut vec![0; (&p as &dyn MarshalInto).serialized_len()])
            .unwrap();
        (&p as &dyn MarshalInto).export_to_vec().unwrap();
        let pp = PacketPile::from(vec![p]);
        (&pp as &dyn Marshal).export(&mut Vec::new()).unwrap();
        (&pp as &dyn MarshalInto)
            .export_into(
                &mut vec![0; (&pp as &dyn MarshalInto).serialized_len()])
            .unwrap();
        (&pp as &dyn MarshalInto).export_to_vec().unwrap();

        // Make a non-exportable signature.
        let sig = uid.bind(
            &mut keypair, &cert,
            signature::SignatureBuilder::new(SignatureType::GenericCertification)
                .set_exportable_certification(false).unwrap()).unwrap();

        // The signature is not exportable.  Try to export it in
        // various ways.
        sig.export(&mut Vec::new()).unwrap_err();
        sig.export_into(&mut vec![0; sig.serialized_len()]).unwrap_err();
        sig.export_to_vec().unwrap_err();
        (&PacketRef::Signature(&sig) as &dyn Marshal)
            .export(&mut Vec::new()).unwrap_err();
        (&PacketRef::Signature(&sig) as &dyn MarshalInto)
            .export_into(
                &mut vec![0; (&PacketRef::Signature(&sig)
                              as &dyn MarshalInto).serialized_len()])
            .unwrap_err();
        (&PacketRef::Signature(&sig) as &dyn MarshalInto)
            .export_to_vec().unwrap_err();
        let p = Packet::Signature(sig);
        (&p as &dyn Marshal).export(&mut Vec::new()).unwrap_err();
        (&p as &dyn MarshalInto)
            .export_into(&mut vec![0; (&p as &dyn MarshalInto).serialized_len()])
            .unwrap_err();
        (&p as &dyn MarshalInto).export_to_vec().unwrap_err();
        let pp = PacketPile::from(vec![p]);
        (&pp as &dyn Marshal).export(&mut Vec::new()).unwrap_err();
        (&pp as &dyn MarshalInto)
            .export_into(
                &mut vec![0; (&pp as &dyn MarshalInto).serialized_len()])
            .unwrap_err();
        (&pp as &dyn MarshalInto).export_to_vec().unwrap_err();
    }

    quickcheck! {
        /// Checks that SerializeInto::serialized_len computes the
        /// exact size (except for CompressedData packets where we may
        /// overestimate the size).
        fn packet_serialized_len(p: Packet) -> bool {
            let p_as_vec = SerializeInto::to_vec(&p).unwrap();
            if let Packet::CompressedData(_) = p {
                // serialized length may be an over-estimate
                assert!(SerializeInto::serialized_len(&p) >= p_as_vec.len());
            } else {
                // serialized length should be exact
                assert_eq!(SerializeInto::serialized_len(&p), p_as_vec.len());
            }
            true
        }
    }
}
