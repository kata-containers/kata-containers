// SPDX-License-Identifier: MIT

use std::net::IpAddr;

use super::{AddressAddRequest, AddressDelRequest, AddressGetRequest};
use crate::Handle;

use netlink_packet_route::AddressMessage;

pub struct AddressHandle(Handle);

impl AddressHandle {
    pub fn new(handle: Handle) -> Self {
        AddressHandle(handle)
    }

    /// Retrieve the list of ip addresses (equivalent to `ip addr show`)
    pub fn get(&self) -> AddressGetRequest {
        AddressGetRequest::new(self.0.clone())
    }

    /// Add an ip address on an interface (equivalent to `ip addr add`)
    pub fn add(&self, index: u32, address: IpAddr, prefix_len: u8) -> AddressAddRequest {
        AddressAddRequest::new(self.0.clone(), index, address, prefix_len)
    }

    /// Delete the given address
    pub fn del(&self, address: AddressMessage) -> AddressDelRequest {
        AddressDelRequest::new(self.0.clone(), address)
    }
}
