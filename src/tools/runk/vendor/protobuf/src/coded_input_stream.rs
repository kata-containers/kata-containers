#![doc(hidden)]

//! `CodedInputStream` and `CodedOutputStream` implementations

use std::io;
use std::io::BufRead;
use std::io::Read;
use std::mem;
use std::mem::MaybeUninit;
use std::slice;

#[cfg(feature = "bytes")]
use bytes::Bytes;

use crate::buf_read_iter::BufReadIter;
#[cfg(feature = "bytes")]
use crate::chars::Chars;
use crate::enums::ProtobufEnum;
use crate::error::ProtobufError;
use crate::error::ProtobufResult;
use crate::error::WireError;
use crate::message::Message;
use crate::misc::maybe_ununit_array_assume_init;
use crate::unknown::UnknownValue;
use crate::wire_format;
use crate::zigzag::decode_zig_zag_32;
use crate::zigzag::decode_zig_zag_64;

/// Default recursion level limit. 100 is the default value of C++'s implementation.
const DEFAULT_RECURSION_LIMIT: u32 = 100;

/// Max allocated vec when reading length-delimited from unknown input stream
pub(crate) const READ_RAW_BYTES_MAX_ALLOC: usize = 10_000_000;

/// Buffered read with handy utilities.
pub struct CodedInputStream<'a> {
    source: BufReadIter<'a>,
    recursion_level: u32,
    recursion_limit: u32,
}

impl<'a> CodedInputStream<'a> {
    /// Wrap a `Read`.
    ///
    /// Note resulting `CodedInputStream` is buffered even if `Read` is not.
    pub fn new(read: &'a mut dyn Read) -> CodedInputStream<'a> {
        CodedInputStream::from_buf_read_iter(BufReadIter::from_read(read))
    }

    /// Create from `BufRead`.
    ///
    /// `CodedInputStream` will utilize `BufRead` buffer.
    pub fn from_buffered_reader(buf_read: &'a mut dyn BufRead) -> CodedInputStream<'a> {
        CodedInputStream::from_buf_read_iter(BufReadIter::from_buf_read(buf_read))
    }

    /// Read from byte slice
    pub fn from_bytes(bytes: &'a [u8]) -> CodedInputStream<'a> {
        CodedInputStream::from_buf_read_iter(BufReadIter::from_byte_slice(bytes))
    }

    /// Read from `Bytes`.
    ///
    /// `CodedInputStream` operations like
    /// [`read_carllerche_bytes`](crate::CodedInputStream::read_carllerche_bytes)
    /// will return a shared copy of this bytes object.
    #[cfg(feature = "bytes")]
    pub fn from_carllerche_bytes(bytes: &'a Bytes) -> CodedInputStream<'a> {
        CodedInputStream::from_buf_read_iter(BufReadIter::from_bytes(bytes))
    }

    fn from_buf_read_iter(source: BufReadIter<'a>) -> CodedInputStream<'a> {
        CodedInputStream {
            source: source,
            recursion_level: 0,
            recursion_limit: DEFAULT_RECURSION_LIMIT,
        }
    }

    /// Set the recursion limit.
    pub fn set_recursion_limit(&mut self, limit: u32) {
        self.recursion_limit = limit;
    }

    #[inline]
    pub(crate) fn incr_recursion(&mut self) -> ProtobufResult<()> {
        if self.recursion_level >= self.recursion_limit {
            return Err(ProtobufError::WireError(WireError::OverRecursionLimit));
        }
        self.recursion_level += 1;
        Ok(())
    }

    #[inline]
    pub(crate) fn decr_recursion(&mut self) {
        self.recursion_level -= 1;
    }

    /// How many bytes processed
    pub fn pos(&self) -> u64 {
        self.source.pos()
    }

    /// How many bytes until current limit
    pub fn bytes_until_limit(&self) -> u64 {
        self.source.bytes_until_limit()
    }

    /// Read bytes into given `buf`.
    #[inline]
    fn read_exact_uninit(&mut self, buf: &mut [MaybeUninit<u8>]) -> ProtobufResult<()> {
        self.source.read_exact(buf)
    }

    /// Read bytes into given `buf`.
    ///
    /// Return `0` on EOF.
    // TODO: overload with `Read::read`
    pub fn read(&mut self, buf: &mut [u8]) -> ProtobufResult<()> {
        // SAFETY: same layout
        let buf = unsafe {
            slice::from_raw_parts_mut(buf.as_mut_ptr() as *mut MaybeUninit<u8>, buf.len())
        };
        self.read_exact_uninit(buf)
    }

    /// Read exact number of bytes as `Bytes` object.
    ///
    /// This operation returns a shared view if `CodedInputStream` is
    /// constructed with `Bytes` parameter.
    #[cfg(feature = "bytes")]
    fn read_raw_callerche_bytes(&mut self, count: usize) -> ProtobufResult<Bytes> {
        self.source.read_exact_bytes(count)
    }

    /// Read one byte
    #[inline(always)]
    pub fn read_raw_byte(&mut self) -> ProtobufResult<u8> {
        self.source.read_byte()
    }

    /// Push new limit, return previous limit.
    pub fn push_limit(&mut self, limit: u64) -> ProtobufResult<u64> {
        self.source.push_limit(limit)
    }

    /// Restore previous limit.
    pub fn pop_limit(&mut self, old_limit: u64) {
        self.source.pop_limit(old_limit);
    }

    /// Are we at EOF?
    #[inline(always)]
    pub fn eof(&mut self) -> ProtobufResult<bool> {
        self.source.eof()
    }

    /// Check we are at EOF.
    ///
    /// Return error if we are not at EOF.
    pub fn check_eof(&mut self) -> ProtobufResult<()> {
        let eof = self.eof()?;
        if !eof {
            return Err(ProtobufError::WireError(WireError::UnexpectedEof));
        }
        Ok(())
    }

    fn read_raw_varint64_slow(&mut self) -> ProtobufResult<u64> {
        let mut r: u64 = 0;
        let mut i = 0;
        loop {
            if i == 10 {
                return Err(ProtobufError::WireError(WireError::IncorrectVarint));
            }
            let b = self.read_raw_byte()?;
            // TODO: may overflow if i == 9
            r = r | (((b & 0x7f) as u64) << (i * 7));
            i += 1;
            if b < 0x80 {
                return Ok(r);
            }
        }
    }

    /// Read varint
    #[inline(always)]
    pub fn read_raw_varint64(&mut self) -> ProtobufResult<u64> {
        'slow: loop {
            let ret;
            let consume;

            loop {
                let rem = self.source.remaining_in_buf();

                if rem.len() >= 1 {
                    // most varints are in practice fit in 1 byte
                    if rem[0] < 0x80 {
                        ret = rem[0] as u64;
                        consume = 1;
                    } else {
                        // handle case of two bytes too
                        if rem.len() >= 2 && rem[1] < 0x80 {
                            ret = (rem[0] & 0x7f) as u64 | (rem[1] as u64) << 7;
                            consume = 2;
                        } else if rem.len() >= 10 {
                            // Read from array when buf at at least 10 bytes,
                            // max len for varint.
                            let mut r: u64 = 0;
                            let mut i: usize = 0;
                            {
                                let rem = rem;
                                loop {
                                    if i == 10 {
                                        return Err(ProtobufError::WireError(
                                            WireError::IncorrectVarint,
                                        ));
                                    }

                                    let b = if true {
                                        // skip range check
                                        unsafe { *rem.get_unchecked(i) }
                                    } else {
                                        rem[i]
                                    };

                                    // TODO: may overflow if i == 9
                                    r = r | (((b & 0x7f) as u64) << (i * 7));
                                    i += 1;
                                    if b < 0x80 {
                                        break;
                                    }
                                }
                            }
                            consume = i;
                            ret = r;
                        } else {
                            break 'slow;
                        }
                    }
                } else {
                    break 'slow;
                }
                break;
            }

            self.source.consume(consume);
            return Ok(ret);
        }

        self.read_raw_varint64_slow()
    }

    /// Read varint
    #[inline(always)]
    pub fn read_raw_varint32(&mut self) -> ProtobufResult<u32> {
        self.read_raw_varint64().map(|v| v as u32)
    }

    /// Read little-endian 32-bit integer
    pub fn read_raw_little_endian32(&mut self) -> ProtobufResult<u32> {
        let mut bytes = [MaybeUninit::uninit(); 4];
        self.read_exact_uninit(&mut bytes)?;
        // SAFETY: `read_exact` guarantees that the buffer is filled.
        let bytes = unsafe { maybe_ununit_array_assume_init(bytes) };
        Ok(u32::from_le_bytes(bytes))
    }

    /// Read little-endian 64-bit integer
    pub fn read_raw_little_endian64(&mut self) -> ProtobufResult<u64> {
        let mut bytes = [MaybeUninit::uninit(); 8];
        self.read_exact_uninit(&mut bytes)?;
        // SAFETY: `read_exact` guarantees that the buffer is filled.
        let bytes = unsafe { maybe_ununit_array_assume_init(bytes) };
        Ok(u64::from_le_bytes(bytes))
    }

    /// Read tag
    #[inline]
    pub fn read_tag(&mut self) -> ProtobufResult<wire_format::Tag> {
        let v = self.read_raw_varint32()?;
        match wire_format::Tag::new(v) {
            Some(tag) => Ok(tag),
            None => Err(ProtobufError::WireError(WireError::IncorrectTag(v))),
        }
    }

    /// Read tag, return it is pair (field number, wire type)
    #[inline]
    pub fn read_tag_unpack(&mut self) -> ProtobufResult<(u32, wire_format::WireType)> {
        self.read_tag().map(|t| t.unpack())
    }

    /// Read `double`
    pub fn read_double(&mut self) -> ProtobufResult<f64> {
        let bits = self.read_raw_little_endian64()?;
        unsafe { Ok(mem::transmute::<u64, f64>(bits)) }
    }

    /// Read `float`
    pub fn read_float(&mut self) -> ProtobufResult<f32> {
        let bits = self.read_raw_little_endian32()?;
        unsafe { Ok(mem::transmute::<u32, f32>(bits)) }
    }

    /// Read `int64`
    pub fn read_int64(&mut self) -> ProtobufResult<i64> {
        self.read_raw_varint64().map(|v| v as i64)
    }

    /// Read `int32`
    pub fn read_int32(&mut self) -> ProtobufResult<i32> {
        self.read_raw_varint32().map(|v| v as i32)
    }

    /// Read `uint64`
    pub fn read_uint64(&mut self) -> ProtobufResult<u64> {
        self.read_raw_varint64()
    }

    /// Read `uint32`
    pub fn read_uint32(&mut self) -> ProtobufResult<u32> {
        self.read_raw_varint32()
    }

    /// Read `sint64`
    pub fn read_sint64(&mut self) -> ProtobufResult<i64> {
        self.read_uint64().map(decode_zig_zag_64)
    }

    /// Read `sint32`
    pub fn read_sint32(&mut self) -> ProtobufResult<i32> {
        self.read_uint32().map(decode_zig_zag_32)
    }

    /// Read `fixed64`
    pub fn read_fixed64(&mut self) -> ProtobufResult<u64> {
        self.read_raw_little_endian64()
    }

    /// Read `fixed32`
    pub fn read_fixed32(&mut self) -> ProtobufResult<u32> {
        self.read_raw_little_endian32()
    }

    /// Read `sfixed64`
    pub fn read_sfixed64(&mut self) -> ProtobufResult<i64> {
        self.read_raw_little_endian64().map(|v| v as i64)
    }

    /// Read `sfixed32`
    pub fn read_sfixed32(&mut self) -> ProtobufResult<i32> {
        self.read_raw_little_endian32().map(|v| v as i32)
    }

    /// Read `bool`
    pub fn read_bool(&mut self) -> ProtobufResult<bool> {
        self.read_raw_varint32().map(|v| v != 0)
    }

    /// Read `enum` as `ProtobufEnum`
    pub fn read_enum<E: ProtobufEnum>(&mut self) -> ProtobufResult<E> {
        let i = self.read_int32()?;
        match ProtobufEnum::from_i32(i) {
            Some(e) => Ok(e),
            None => Err(ProtobufError::WireError(WireError::InvalidEnumValue(i))),
        }
    }

    /// Read `repeated` packed `double`
    pub fn read_repeated_packed_double_into(
        &mut self,
        target: &mut Vec<f64>,
    ) -> ProtobufResult<()> {
        let len = self.read_raw_varint64()?;

        target.reserve((len / 4) as usize);

        let old_limit = self.push_limit(len)?;
        while !self.eof()? {
            target.push(self.read_double()?);
        }
        self.pop_limit(old_limit);
        Ok(())
    }

    /// Read `repeated` packed `float`
    pub fn read_repeated_packed_float_into(&mut self, target: &mut Vec<f32>) -> ProtobufResult<()> {
        let len = self.read_raw_varint64()?;

        target.reserve((len / 4) as usize);

        let old_limit = self.push_limit(len)?;
        while !self.eof()? {
            target.push(self.read_float()?);
        }
        self.pop_limit(old_limit);
        Ok(())
    }

    /// Read `repeated` packed `int64`
    pub fn read_repeated_packed_int64_into(&mut self, target: &mut Vec<i64>) -> ProtobufResult<()> {
        let len = self.read_raw_varint64()?;
        let old_limit = self.push_limit(len as u64)?;
        while !self.eof()? {
            target.push(self.read_int64()?);
        }
        self.pop_limit(old_limit);
        Ok(())
    }

    /// Read repeated packed `int32`
    pub fn read_repeated_packed_int32_into(&mut self, target: &mut Vec<i32>) -> ProtobufResult<()> {
        let len = self.read_raw_varint64()?;
        let old_limit = self.push_limit(len)?;
        while !self.eof()? {
            target.push(self.read_int32()?);
        }
        self.pop_limit(old_limit);
        Ok(())
    }

    /// Read repeated packed `uint64`
    pub fn read_repeated_packed_uint64_into(
        &mut self,
        target: &mut Vec<u64>,
    ) -> ProtobufResult<()> {
        let len = self.read_raw_varint64()?;
        let old_limit = self.push_limit(len)?;
        while !self.eof()? {
            target.push(self.read_uint64()?);
        }
        self.pop_limit(old_limit);
        Ok(())
    }

    /// Read repeated packed `uint32`
    pub fn read_repeated_packed_uint32_into(
        &mut self,
        target: &mut Vec<u32>,
    ) -> ProtobufResult<()> {
        let len = self.read_raw_varint64()?;
        let old_limit = self.push_limit(len)?;
        while !self.eof()? {
            target.push(self.read_uint32()?);
        }
        self.pop_limit(old_limit);
        Ok(())
    }

    /// Read repeated packed `sint64`
    pub fn read_repeated_packed_sint64_into(
        &mut self,
        target: &mut Vec<i64>,
    ) -> ProtobufResult<()> {
        let len = self.read_raw_varint64()?;
        let old_limit = self.push_limit(len)?;
        while !self.eof()? {
            target.push(self.read_sint64()?);
        }
        self.pop_limit(old_limit);
        Ok(())
    }

    /// Read repeated packed `sint32`
    pub fn read_repeated_packed_sint32_into(
        &mut self,
        target: &mut Vec<i32>,
    ) -> ProtobufResult<()> {
        let len = self.read_raw_varint64()?;
        let old_limit = self.push_limit(len)?;
        while !self.eof()? {
            target.push(self.read_sint32()?);
        }
        self.pop_limit(old_limit);
        Ok(())
    }

    /// Read repeated packed `fixed64`
    pub fn read_repeated_packed_fixed64_into(
        &mut self,
        target: &mut Vec<u64>,
    ) -> ProtobufResult<()> {
        let len = self.read_raw_varint64()?;

        target.reserve((len / 8) as usize);

        let old_limit = self.push_limit(len)?;
        while !self.eof()? {
            target.push(self.read_fixed64()?);
        }
        self.pop_limit(old_limit);
        Ok(())
    }

    /// Read repeated packed `fixed32`
    pub fn read_repeated_packed_fixed32_into(
        &mut self,
        target: &mut Vec<u32>,
    ) -> ProtobufResult<()> {
        let len = self.read_raw_varint64()?;

        target.reserve((len / 4) as usize);

        let old_limit = self.push_limit(len)?;
        while !self.eof()? {
            target.push(self.read_fixed32()?);
        }
        self.pop_limit(old_limit);
        Ok(())
    }

    /// Read repeated packed `sfixed64`
    pub fn read_repeated_packed_sfixed64_into(
        &mut self,
        target: &mut Vec<i64>,
    ) -> ProtobufResult<()> {
        let len = self.read_raw_varint64()?;

        target.reserve((len / 8) as usize);

        let old_limit = self.push_limit(len)?;
        while !self.eof()? {
            target.push(self.read_sfixed64()?);
        }
        self.pop_limit(old_limit);
        Ok(())
    }

    /// Read repeated packed `sfixed32`
    pub fn read_repeated_packed_sfixed32_into(
        &mut self,
        target: &mut Vec<i32>,
    ) -> ProtobufResult<()> {
        let len = self.read_raw_varint64()?;

        target.reserve((len / 4) as usize);

        let old_limit = self.push_limit(len)?;
        while !self.eof()? {
            target.push(self.read_sfixed32()?);
        }
        self.pop_limit(old_limit);
        Ok(())
    }

    /// Read repeated packed `bool`
    pub fn read_repeated_packed_bool_into(&mut self, target: &mut Vec<bool>) -> ProtobufResult<()> {
        let len = self.read_raw_varint64()?;

        // regular bool value is 1-byte size
        target.reserve(len as usize);

        let old_limit = self.push_limit(len)?;
        while !self.eof()? {
            target.push(self.read_bool()?);
        }
        self.pop_limit(old_limit);
        Ok(())
    }

    /// Read repeated packed `enum` into `ProtobufEnum`
    pub fn read_repeated_packed_enum_into<E: ProtobufEnum>(
        &mut self,
        target: &mut Vec<E>,
    ) -> ProtobufResult<()> {
        let len = self.read_raw_varint64()?;
        let old_limit = self.push_limit(len)?;
        while !self.eof()? {
            target.push(self.read_enum()?);
        }
        self.pop_limit(old_limit);
        Ok(())
    }

    /// Read `UnknownValue`
    pub fn read_unknown(
        &mut self,
        wire_type: wire_format::WireType,
    ) -> ProtobufResult<UnknownValue> {
        match wire_type {
            wire_format::WireTypeVarint => {
                self.read_raw_varint64().map(|v| UnknownValue::Varint(v))
            }
            wire_format::WireTypeFixed64 => self.read_fixed64().map(|v| UnknownValue::Fixed64(v)),
            wire_format::WireTypeFixed32 => self.read_fixed32().map(|v| UnknownValue::Fixed32(v)),
            wire_format::WireTypeLengthDelimited => {
                let len = self.read_raw_varint32()?;
                self.read_raw_bytes(len)
                    .map(|v| UnknownValue::LengthDelimited(v))
            }
            _ => Err(ProtobufError::WireError(WireError::UnexpectedWireType(
                wire_type,
            ))),
        }
    }

    /// Skip field
    pub fn skip_field(&mut self, wire_type: wire_format::WireType) -> ProtobufResult<()> {
        self.read_unknown(wire_type).map(|_| ())
    }

    /// Read raw bytes into the supplied vector.  The vector will be resized as needed and
    /// overwritten.
    pub fn read_raw_bytes_into(&mut self, count: u32, target: &mut Vec<u8>) -> ProtobufResult<()> {
        self.source.read_exact_to_vec(count as usize, target)
    }

    /// Read exact number of bytes
    pub fn read_raw_bytes(&mut self, count: u32) -> ProtobufResult<Vec<u8>> {
        let mut r = Vec::new();
        self.read_raw_bytes_into(count, &mut r)?;
        Ok(r)
    }

    /// Skip exact number of bytes
    pub fn skip_raw_bytes(&mut self, count: u32) -> ProtobufResult<()> {
        // TODO: make it more efficient
        self.read_raw_bytes(count).map(|_| ())
    }

    /// Read `bytes` field, length delimited
    pub fn read_bytes(&mut self) -> ProtobufResult<Vec<u8>> {
        let mut r = Vec::new();
        self.read_bytes_into(&mut r)?;
        Ok(r)
    }

    /// Read `bytes` field, length delimited
    #[cfg(feature = "bytes")]
    pub fn read_carllerche_bytes(&mut self) -> ProtobufResult<Bytes> {
        let len = self.read_raw_varint32()?;
        self.read_raw_callerche_bytes(len as usize)
    }

    /// Read `string` field, length delimited
    #[cfg(feature = "bytes")]
    pub fn read_carllerche_chars(&mut self) -> ProtobufResult<Chars> {
        let bytes = self.read_carllerche_bytes()?;
        Ok(Chars::from_bytes(bytes)?)
    }

    /// Read `bytes` field, length delimited
    pub fn read_bytes_into(&mut self, target: &mut Vec<u8>) -> ProtobufResult<()> {
        let len = self.read_raw_varint32()?;
        self.read_raw_bytes_into(len, target)?;
        Ok(())
    }

    /// Read `string` field, length delimited
    pub fn read_string(&mut self) -> ProtobufResult<String> {
        let mut r = String::new();
        self.read_string_into(&mut r)?;
        Ok(r)
    }

    /// Read `string` field, length delimited
    pub fn read_string_into(&mut self, target: &mut String) -> ProtobufResult<()> {
        target.clear();
        // take target's buffer
        let mut vec = mem::replace(target, String::new()).into_bytes();
        self.read_bytes_into(&mut vec)?;

        let s = match String::from_utf8(vec) {
            Ok(t) => t,
            Err(_) => return Err(ProtobufError::WireError(WireError::Utf8Error)),
        };
        *target = s;
        Ok(())
    }

    /// Read message, do not check if message is initialized
    pub fn merge_message<M: Message>(&mut self, message: &mut M) -> ProtobufResult<()> {
        let len = self.read_raw_varint64()?;
        let old_limit = self.push_limit(len)?;
        message.merge_from(self)?;
        self.pop_limit(old_limit);
        Ok(())
    }

    /// Read message
    pub fn read_message<M: Message>(&mut self) -> ProtobufResult<M> {
        let mut r: M = Message::new();
        self.merge_message(&mut r)?;
        r.check_initialized()?;
        Ok(r)
    }
}

impl<'a> Read for CodedInputStream<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.source.read(buf).map_err(Into::into)
    }
}

impl<'a> BufRead for CodedInputStream<'a> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.source.fill_buf().map_err(Into::into)
    }

    fn consume(&mut self, amt: usize) {
        self.source.consume(amt)
    }
}

/// Helper internal utility, should not be used directly
#[doc(hidden)]
pub trait WithCodedInputStream {
    fn with_coded_input_stream<T, F>(self, cb: F) -> ProtobufResult<T>
    where
        F: FnOnce(&mut CodedInputStream) -> ProtobufResult<T>;
}

impl<'a> WithCodedInputStream for &'a mut (dyn Read + 'a) {
    fn with_coded_input_stream<T, F>(self, cb: F) -> ProtobufResult<T>
    where
        F: FnOnce(&mut CodedInputStream) -> ProtobufResult<T>,
    {
        let mut is = CodedInputStream::new(self);
        let r = cb(&mut is)?;
        is.check_eof()?;
        Ok(r)
    }
}

impl<'a> WithCodedInputStream for &'a mut (dyn BufRead + 'a) {
    fn with_coded_input_stream<T, F>(self, cb: F) -> ProtobufResult<T>
    where
        F: FnOnce(&mut CodedInputStream) -> ProtobufResult<T>,
    {
        let mut is = CodedInputStream::from_buffered_reader(self);
        let r = cb(&mut is)?;
        is.check_eof()?;
        Ok(r)
    }
}

impl<'a> WithCodedInputStream for &'a [u8] {
    fn with_coded_input_stream<T, F>(self, cb: F) -> ProtobufResult<T>
    where
        F: FnOnce(&mut CodedInputStream) -> ProtobufResult<T>,
    {
        let mut is = CodedInputStream::from_bytes(self);
        let r = cb(&mut is)?;
        is.check_eof()?;
        Ok(r)
    }
}

#[cfg(feature = "bytes")]
impl<'a> WithCodedInputStream for &'a Bytes {
    fn with_coded_input_stream<T, F>(self, cb: F) -> ProtobufResult<T>
    where
        F: FnOnce(&mut CodedInputStream) -> ProtobufResult<T>,
    {
        let mut is = CodedInputStream::from_carllerche_bytes(self);
        let r = cb(&mut is)?;
        is.check_eof()?;
        Ok(r)
    }
}

#[cfg(test)]
mod test {

    use std::fmt::Debug;
    use std::io;
    use std::io::BufRead;
    use std::io::Read;

    use super::CodedInputStream;
    use super::READ_RAW_BYTES_MAX_ALLOC;
    use crate::error::ProtobufError;
    use crate::error::ProtobufResult;
    use crate::hex::decode_hex;

    fn test_read_partial<F>(hex: &str, mut callback: F)
    where
        F: FnMut(&mut CodedInputStream),
    {
        let d = decode_hex(hex);
        let mut reader = io::Cursor::new(d);
        let mut is = CodedInputStream::from_buffered_reader(&mut reader as &mut dyn BufRead);
        assert_eq!(0, is.pos());
        callback(&mut is);
    }

    fn test_read<F>(hex: &str, mut callback: F)
    where
        F: FnMut(&mut CodedInputStream),
    {
        let len = decode_hex(hex).len();
        test_read_partial(hex, |reader| {
            callback(reader);
            assert!(reader.eof().expect("eof"));
            assert_eq!(len as u64, reader.pos());
        });
    }

    fn test_read_v<F, V>(hex: &str, v: V, mut callback: F)
    where
        F: FnMut(&mut CodedInputStream) -> ProtobufResult<V>,
        V: PartialEq + Debug,
    {
        test_read(hex, |reader| {
            assert_eq!(v, callback(reader).unwrap());
        });
    }

    #[test]
    fn test_input_stream_read_raw_byte() {
        test_read("17", |is| {
            assert_eq!(23, is.read_raw_byte().unwrap());
        });
    }

    #[test]
    fn test_input_stream_read_raw_varint() {
        test_read_v("07", 7, |reader| reader.read_raw_varint32());
        test_read_v("07", 7, |reader| reader.read_raw_varint64());

        test_read_v("96 01", 150, |reader| reader.read_raw_varint32());
        test_read_v("96 01", 150, |reader| reader.read_raw_varint64());

        test_read_v(
            "ff ff ff ff ff ff ff ff ff 01",
            0xffffffffffffffff,
            |reader| reader.read_raw_varint64(),
        );

        test_read_v("ff ff ff ff 0f", 0xffffffff, |reader| {
            reader.read_raw_varint32()
        });
        test_read_v("ff ff ff ff 0f", 0xffffffff, |reader| {
            reader.read_raw_varint64()
        });
    }

    #[test]
    fn test_input_stream_read_raw_vaint_malformed() {
        // varint cannot have length > 10
        test_read_partial("ff ff ff ff ff ff ff ff ff ff 01", |reader| {
            let result = reader.read_raw_varint64();
            match result {
                // TODO: make an enum variant
                Err(ProtobufError::WireError(..)) => (),
                _ => panic!(),
            }
        });
        test_read_partial("ff ff ff ff ff ff ff ff ff ff 01", |reader| {
            let result = reader.read_raw_varint32();
            match result {
                // TODO: make an enum variant
                Err(ProtobufError::WireError(..)) => (),
                _ => panic!(),
            }
        });
    }

    #[test]
    fn test_input_stream_read_raw_varint_unexpected_eof() {
        test_read_partial("96 97", |reader| {
            let result = reader.read_raw_varint32();
            match result {
                Err(ProtobufError::WireError(..)) => (),
                _ => panic!(),
            }
        });
    }

    #[test]
    fn test_input_stream_read_raw_varint_pos() {
        test_read_partial("95 01 98", |reader| {
            assert_eq!(149, reader.read_raw_varint32().unwrap());
            assert_eq!(2, reader.pos());
        });
    }

    #[test]
    fn test_input_stream_read_int32() {
        test_read_v("02", 2, |reader| reader.read_int32());
    }

    #[test]
    fn test_input_stream_read_float() {
        test_read_v("95 73 13 61", 17e19, |is| is.read_float());
    }

    #[test]
    fn test_input_stream_read_double() {
        test_read_v("40 d5 ab 68 b3 07 3d 46", 23e29, |is| is.read_double());
    }

    #[test]
    fn test_input_stream_skip_raw_bytes() {
        test_read("", |reader| {
            reader.skip_raw_bytes(0).unwrap();
        });
        test_read("aa bb", |reader| {
            reader.skip_raw_bytes(2).unwrap();
        });
        test_read("aa bb cc dd ee ff", |reader| {
            reader.skip_raw_bytes(6).unwrap();
        });
    }

    #[test]
    fn test_input_stream_read_raw_bytes() {
        test_read("", |reader| {
            assert_eq!(
                Vec::from(&b""[..]),
                reader.read_raw_bytes(0).expect("read_raw_bytes")
            );
        })
    }

    #[test]
    fn test_input_stream_limits() {
        test_read("aa bb cc", |is| {
            let old_limit = is.push_limit(1).unwrap();
            assert_eq!(1, is.bytes_until_limit());
            let r1 = is.read_raw_bytes(1).unwrap();
            assert_eq!(&[0xaa as u8], &r1[..]);
            is.pop_limit(old_limit);
            let r2 = is.read_raw_bytes(2).unwrap();
            assert_eq!(&[0xbb as u8, 0xcc], &r2[..]);
        });
    }

    #[test]
    fn test_input_stream_io_read() {
        test_read("aa bb cc", |is| {
            let mut buf = [0; 3];
            assert_eq!(Read::read(is, &mut buf).expect("io::Read"), 3);
            assert_eq!(buf, [0xaa, 0xbb, 0xcc]);
        });
    }

    #[test]
    fn test_input_stream_io_bufread() {
        test_read("aa bb cc", |is| {
            assert_eq!(
                BufRead::fill_buf(is).expect("io::BufRead::fill_buf"),
                &[0xaa, 0xbb, 0xcc]
            );
            BufRead::consume(is, 3);
        });
    }

    #[test]
    fn test_input_stream_read_raw_bytes_into_huge() {
        let mut v = Vec::new();
        for i in 0..READ_RAW_BYTES_MAX_ALLOC + 1000 {
            v.push((i % 10) as u8);
        }

        let mut slice: &[u8] = v.as_slice();

        let mut is = CodedInputStream::new(&mut slice);

        let mut buf = Vec::new();

        is.read_raw_bytes_into(READ_RAW_BYTES_MAX_ALLOC as u32 + 10, &mut buf)
            .expect("read");

        assert_eq!(READ_RAW_BYTES_MAX_ALLOC + 10, buf.len());

        buf.clear();

        is.read_raw_bytes_into(1000 - 10, &mut buf).expect("read");

        assert_eq!(1000 - 10, buf.len());

        assert!(is.eof().expect("eof"));
    }
}
