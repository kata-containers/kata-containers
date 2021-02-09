//! Protocol Buffers well-known wrapper types.
//!
//! This module provides implementations of `Message` for Rust standard library types which
//! correspond to a Protobuf well-known wrapper type. The remaining well-known types are defined in
//! the `prost-types` crate in order to avoid a cyclic dependency between `prost` and
//! `prost-build`.

use ::bytes::{Buf, BufMut};

use crate::encoding::*;
use crate::DecodeError;
use crate::Message;

/// `google.protobuf.BoolValue`
impl Message for bool {
    fn encode_raw<B>(&self, buf: &mut B)
    where
        B: BufMut,
    {
        if *self {
            bool::encode(1, self, buf)
        }
    }
    fn merge_field<B>(&mut self, buf: &mut B) -> Result<(), DecodeError>
    where
        B: Buf,
    {
        let (tag, wire_type) = decode_key(buf)?;
        if tag == 1 {
            bool::merge(wire_type, self, buf)
        } else {
            skip_field(wire_type, buf)
        }
    }
    fn encoded_len(&self) -> usize {
        if *self {
            2
        } else {
            0
        }
    }
    fn clear(&mut self) {
        *self = false;
    }
}

/// `google.protobuf.UInt32Value`
impl Message for u32 {
    fn encode_raw<B>(&self, buf: &mut B)
    where
        B: BufMut,
    {
        if *self != 0 {
            uint32::encode(1, self, buf)
        }
    }
    fn merge_field<B>(&mut self, buf: &mut B) -> Result<(), DecodeError>
    where
        B: Buf,
    {
        let (tag, wire_type) = decode_key(buf)?;
        if tag == 1 {
            uint32::merge(wire_type, self, buf)
        } else {
            skip_field(wire_type, buf)
        }
    }
    fn encoded_len(&self) -> usize {
        if *self != 0 {
            uint32::encoded_len(1, self)
        } else {
            0
        }
    }
    fn clear(&mut self) {
        *self = 0;
    }
}

/// `google.protobuf.UInt64Value`
impl Message for u64 {
    fn encode_raw<B>(&self, buf: &mut B)
    where
        B: BufMut,
    {
        if *self != 0 {
            uint64::encode(1, self, buf)
        }
    }
    fn merge_field<B>(&mut self, buf: &mut B) -> Result<(), DecodeError>
    where
        B: Buf,
    {
        let (tag, wire_type) = decode_key(buf)?;
        if tag == 1 {
            uint64::merge(wire_type, self, buf)
        } else {
            skip_field(wire_type, buf)
        }
    }
    fn encoded_len(&self) -> usize {
        if *self != 0 {
            uint64::encoded_len(1, self)
        } else {
            0
        }
    }
    fn clear(&mut self) {
        *self = 0;
    }
}

/// `google.protobuf.Int32Value`
impl Message for i32 {
    fn encode_raw<B>(&self, buf: &mut B)
    where
        B: BufMut,
    {
        if *self != 0 {
            int32::encode(1, self, buf)
        }
    }
    fn merge_field<B>(&mut self, buf: &mut B) -> Result<(), DecodeError>
    where
        B: Buf,
    {
        let (tag, wire_type) = decode_key(buf)?;
        if tag == 1 {
            int32::merge(wire_type, self, buf)
        } else {
            skip_field(wire_type, buf)
        }
    }
    fn encoded_len(&self) -> usize {
        if *self != 0 {
            int32::encoded_len(1, self)
        } else {
            0
        }
    }
    fn clear(&mut self) {
        *self = 0;
    }
}

/// `google.protobuf.Int64Value`
impl Message for i64 {
    fn encode_raw<B>(&self, buf: &mut B)
    where
        B: BufMut,
    {
        if *self != 0 {
            int64::encode(1, self, buf)
        }
    }
    fn merge_field<B>(&mut self, buf: &mut B) -> Result<(), DecodeError>
    where
        B: Buf,
    {
        let (tag, wire_type) = decode_key(buf)?;
        if tag == 1 {
            int64::merge(wire_type, self, buf)
        } else {
            skip_field(wire_type, buf)
        }
    }
    fn encoded_len(&self) -> usize {
        if *self != 0 {
            int64::encoded_len(1, self)
        } else {
            0
        }
    }
    fn clear(&mut self) {
        *self = 0;
    }
}

/// `google.protobuf.FloatValue`
impl Message for f32 {
    fn encode_raw<B>(&self, buf: &mut B)
    where
        B: BufMut,
    {
        if *self != 0.0 {
            float::encode(1, self, buf)
        }
    }
    fn merge_field<B>(&mut self, buf: &mut B) -> Result<(), DecodeError>
    where
        B: Buf,
    {
        let (tag, wire_type) = decode_key(buf)?;
        if tag == 1 {
            float::merge(wire_type, self, buf)
        } else {
            skip_field(wire_type, buf)
        }
    }
    fn encoded_len(&self) -> usize {
        if *self != 0.0 {
            float::encoded_len(1, self)
        } else {
            0
        }
    }
    fn clear(&mut self) {
        *self = 0.0;
    }
}

/// `google.protobuf.DoubleValue`
impl Message for f64 {
    fn encode_raw<B>(&self, buf: &mut B)
    where
        B: BufMut,
    {
        if *self != 0.0 {
            double::encode(1, self, buf)
        }
    }
    fn merge_field<B>(&mut self, buf: &mut B) -> Result<(), DecodeError>
    where
        B: Buf,
    {
        let (tag, wire_type) = decode_key(buf)?;
        if tag == 1 {
            double::merge(wire_type, self, buf)
        } else {
            skip_field(wire_type, buf)
        }
    }
    fn encoded_len(&self) -> usize {
        if *self != 0.0 {
            double::encoded_len(1, self)
        } else {
            0
        }
    }
    fn clear(&mut self) {
        *self = 0.0;
    }
}

/// `google.protobuf.StringValue`
impl Message for String {
    fn encode_raw<B>(&self, buf: &mut B)
    where
        B: BufMut,
    {
        if !self.is_empty() {
            string::encode(1, self, buf)
        }
    }
    fn merge_field<B>(&mut self, buf: &mut B) -> Result<(), DecodeError>
    where
        B: Buf,
    {
        let (tag, wire_type) = decode_key(buf)?;
        if tag == 1 {
            string::merge(wire_type, self, buf)
        } else {
            skip_field(wire_type, buf)
        }
    }
    fn encoded_len(&self) -> usize {
        if !self.is_empty() {
            string::encoded_len(1, self)
        } else {
            0
        }
    }
    fn clear(&mut self) {
        self.clear();
    }
}

/// `google.protobuf.BytesValue`
impl Message for Vec<u8> {
    fn encode_raw<B>(&self, buf: &mut B)
    where
        B: BufMut,
    {
        if !self.is_empty() {
            bytes::encode(1, self, buf)
        }
    }
    fn merge_field<B>(&mut self, buf: &mut B) -> Result<(), DecodeError>
    where
        B: Buf,
    {
        let (tag, wire_type) = decode_key(buf)?;
        if tag == 1 {
            bytes::merge(wire_type, self, buf)
        } else {
            skip_field(wire_type, buf)
        }
    }
    fn encoded_len(&self) -> usize {
        if !self.is_empty() {
            bytes::encoded_len(1, self)
        } else {
            0
        }
    }
    fn clear(&mut self) {
        self.clear();
    }
}

/// `google.protobuf.Empty`
impl Message for () {
    fn encode_raw<B>(&self, _buf: &mut B)
    where
        B: BufMut,
    {
    }
    fn merge_field<B>(&mut self, buf: &mut B) -> Result<(), DecodeError>
    where
        B: Buf,
    {
        let (_, wire_type) = decode_key(buf)?;
        skip_field(wire_type, buf)
    }
    fn encoded_len(&self) -> usize {
        0
    }
    fn clear(&mut self) {}
}
