// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use protobuf::{EnumOrUnknown, MessageField};
use serde::{Deserialize, Serialize};

#[cfg(feature = "with-serde")]
pub fn serialize_enum_or_unknown<E: protobuf::EnumFull, S: serde::Serializer>(
    e: &protobuf::EnumOrUnknown<E>,
    s: S,
) -> Result<S::Ok, S::Error> {
    e.value().serialize(s)
}

pub fn serialize_message_field<E: Serialize, S: serde::Serializer>(
    e: &protobuf::MessageField<E>,
    s: S,
) -> Result<S::Ok, S::Error> {
    if e.is_some() {
        e.as_ref().unwrap().serialize(s)
    } else {
        s.serialize_unit()
    }
}

pub fn deserialize_enum_or_unknown<'de, E: Deserialize<'de>, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<protobuf::EnumOrUnknown<E>, D::Error> {
    i32::deserialize(d).map(EnumOrUnknown::from_i32)
}

pub fn deserialize_message_field<'de, E: Deserialize<'de>, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<protobuf::MessageField<E>, D::Error> {
    Option::deserialize(d).map(MessageField::from_option)
}
