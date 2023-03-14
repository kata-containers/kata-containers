// SPDX-License-Identifier: MIT

use crate::{Handle, IpVersion, RouteAddRequest, RouteDelRequest, RouteGetRequest};
use netlink_packet_route::RouteMessage;

pub struct RouteHandle(Handle);

impl RouteHandle {
    pub fn new(handle: Handle) -> Self {
        RouteHandle(handle)
    }

    /// Retrieve the list of routing table entries (equivalent to `ip route show`)
    pub fn get(&self, ip_version: IpVersion) -> RouteGetRequest {
        RouteGetRequest::new(self.0.clone(), ip_version)
    }

    /// Add an routing table entry (equivalent to `ip route add`)
    pub fn add(&self) -> RouteAddRequest {
        RouteAddRequest::new(self.0.clone())
    }

    /// Delete the given routing table entry (equivalent to `ip route del`)
    pub fn del(&self, route: RouteMessage) -> RouteDelRequest {
        RouteDelRequest::new(self.0.clone(), route)
    }
}
