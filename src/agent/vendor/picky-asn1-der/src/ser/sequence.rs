use crate::misc::WriteExt;
use crate::ser::{to_writer, Serializer};
use crate::{Asn1DerError, Result};
use picky_asn1::tag::Tag;
use serde::Serialize;
use std::io::Cursor;

/// A serializer for sequences
pub struct Sequence<'a, 'se> {
    ser: &'a mut Serializer<'se>,
    buf: Cursor<Vec<u8>>,
    tag: Tag,
}

impl<'a, 'se> Sequence<'a, 'se> {
    /// Creates a lazy serializer that will serialize the sequence's sub-elements to `writer`
    pub fn serialize_lazy(ser: &'a mut Serializer<'se>, tag: Tag) -> Self {
        Self {
            ser,
            buf: Cursor::new(Vec::new()),
            tag,
        }
    }

    /// Writes the next `value` to the internal buffer
    fn write_object<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
        to_writer(value, &mut self.buf)?;
        Ok(())
    }

    /// Finalizes the sequence
    fn finalize(self) -> Result<usize> {
        // Reclaim buffer
        let buf = self.buf.into_inner();

        let mut written = self.ser.h_write_header(self.tag, buf.len())?;
        written += self.ser.writer.write_exact(&buf)?;

        Ok(written)
    }
}

impl<'a, 'se> serde::ser::SerializeSeq for Sequence<'a, 'se> {
    type Ok = usize;
    type Error = Asn1DerError;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
        self.write_object(value)
    }
    fn end(self) -> Result<Self::Ok> {
        self.finalize()
    }
}

impl<'a, 'se> serde::ser::SerializeTuple for Sequence<'a, 'se> {
    type Ok = usize;
    type Error = Asn1DerError;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
        self.write_object(value)
    }
    fn end(self) -> Result<Self::Ok> {
        self.finalize()
    }
}

impl<'a, 'se> serde::ser::SerializeStruct for Sequence<'a, 'se> {
    type Ok = usize;
    type Error = Asn1DerError;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, _key: &'static str, value: &T) -> Result<()> {
        self.write_object(value)
    }
    fn end(self) -> Result<Self::Ok> {
        self.finalize()
    }
}

impl<'a, 'se> serde::ser::SerializeTupleStruct for Sequence<'a, 'se> {
    type Ok = usize;
    type Error = Asn1DerError;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
        self.write_object(value)
    }
    fn end(self) -> Result<Self::Ok> {
        self.finalize()
    }
}
