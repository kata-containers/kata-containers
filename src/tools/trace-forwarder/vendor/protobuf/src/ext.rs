//! Utilities to support "extension" fields.
//!
//! Extensions are [described in the official protobuf documentation][exts].
//!
//! [exts]: https://developers.google.com/protocol-buffers/docs/proto#extensions

use std::marker::PhantomData;

use crate::message::Message;
use crate::types::ProtobufType;

/// Optional ext field
pub struct ExtFieldOptional<M: Message, T: ProtobufType> {
    /// Extension field number
    pub field_number: u32,
    /// Marker
    // TODO: hide
    pub phantom: PhantomData<(M, T)>,
}

/// Repeated ext field
pub struct ExtFieldRepeated<M: Message, T: ProtobufType> {
    /// Extension field number
    pub field_number: u32,
    /// Extension field number
    // TODO: hide
    pub phantom: PhantomData<(M, T)>,
}

impl<M: Message, T: ProtobufType> ExtFieldOptional<M, T> {
    /// Get a copy of value from a message.
    ///
    /// Extension data is stored in [`UnknownFields`](crate::UnknownFields).
    pub fn get(&self, m: &M) -> Option<T::Value> {
        m.get_unknown_fields()
            .get(self.field_number)
            .and_then(T::get_from_unknown)
    }
}

impl<M: Message, T: ProtobufType> ExtFieldRepeated<M, T> {
    /// Get a copy of value from a message (**not implemented**).
    pub fn get(&self, _m: &M) -> Vec<T::Value> {
        // TODO
        unimplemented!()
    }
}
