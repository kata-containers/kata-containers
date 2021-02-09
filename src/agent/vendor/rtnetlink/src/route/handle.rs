//use std::net::IpAddr;

use super::{RouteAddIpv4Request, RouteAddIpv6Request};
use crate::{Handle, IpVersion, RouteDelRequest, RouteGetRequest};
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
    pub fn add_v4(&self) -> RouteAddIpv4Request {
        RouteAddIpv4Request::new(self.0.clone())
    }

    /// Add an routing table entry (equivalent to `ip route add`)
    pub fn add_v6(&self) -> RouteAddIpv6Request {
        RouteAddIpv6Request::new(self.0.clone())
    }

    /// Delete the given routing table entry (equivalent to `ip route del`)
    pub fn del(&self, route: RouteMessage) -> RouteDelRequest {
        RouteDelRequest::new(self.0.clone(), route)
    }
}
