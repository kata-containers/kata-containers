// SPDX-License-Identifier: MIT

use crate::{Handle, NeighbourAddRequest, NeighbourDelRequest, NeighbourGetRequest};
use netlink_packet_route::NeighbourMessage;
use std::net::IpAddr;

pub struct NeighbourHandle(Handle);

impl NeighbourHandle {
    pub fn new(handle: Handle) -> Self {
        NeighbourHandle(handle)
    }

    /// List neighbour entries (equivalent to `ip neighbour show`)
    pub fn get(&self) -> NeighbourGetRequest {
        NeighbourGetRequest::new(self.0.clone())
    }

    /// Add a new neighbour entry (equivalent to `ip neighbour add`)
    pub fn add(&self, index: u32, destination: IpAddr) -> NeighbourAddRequest {
        NeighbourAddRequest::new(self.0.clone(), index, destination)
    }

    /// Add a new fdb entry (equivalent to `bridge fdb add`)
    pub fn add_bridge(&self, index: u32, lla: &[u8]) -> NeighbourAddRequest {
        NeighbourAddRequest::new_bridge(self.0.clone(), index, lla)
    }

    /// Delete a neighbour entry (equivalent to `ip neighbour delete`)
    pub fn del(&self, message: NeighbourMessage) -> NeighbourDelRequest {
        NeighbourDelRequest::new(self.0.clone(), message)
    }
}
