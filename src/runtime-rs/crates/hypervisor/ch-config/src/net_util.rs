// Copyright (c) 2022-2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize, Serializer};
use std::fmt;

pub const MAC_ADDR_LEN: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Default)]
pub struct MacAddr {
    pub bytes: [u8; MAC_ADDR_LEN],
}

// Note: Implements ToString automatically.
impl fmt::Display for MacAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let b = &self.bytes;
        write!(
            f,
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            b[0], b[1], b[2], b[3], b[4], b[5]
        )
    }
}

// Requried to remove the `bytes` member from the serialized JSON!
impl Serialize for MacAddr {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.to_string().serialize(serializer)
    }
}
