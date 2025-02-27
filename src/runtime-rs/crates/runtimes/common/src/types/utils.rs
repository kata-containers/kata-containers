// Copyright 2024 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use std::convert::TryInto;
use std::time;

fn system_time_into(time: time::SystemTime) -> ::protobuf::well_known_types::timestamp::Timestamp {
    let mut proto_time = ::protobuf::well_known_types::timestamp::Timestamp::new();
    proto_time.seconds = time
        .duration_since(time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .try_into()
        .unwrap_or_default();

    proto_time
}

pub fn option_system_time_into(
    time: Option<time::SystemTime>,
) -> protobuf::MessageField<protobuf::well_known_types::timestamp::Timestamp> {
    match time {
        Some(v) => ::protobuf::MessageField::some(system_time_into(v)),
        None => ::protobuf::MessageField::none(),
    }
}
