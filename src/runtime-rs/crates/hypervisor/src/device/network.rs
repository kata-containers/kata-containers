// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fmt;

pub struct Address(pub [u8; 6]);

impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let b = self.0;
        write!(
            f,
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            b[0], b[1], b[2], b[3], b[4], b[5]
        )
    }
}

#[derive(Debug)]
pub struct NetworkConfig {
    /// Unique identifier of the device
    pub id: String,

    /// Host level path for the guest network interface.
    pub host_dev_name: String,

    /// Guest MAC address.
    pub guest_mac: Option<Address>,
}
