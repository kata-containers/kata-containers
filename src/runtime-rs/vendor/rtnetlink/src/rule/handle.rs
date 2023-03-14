// SPDX-License-Identifier: MIT

use crate::{Handle, IpVersion, RuleAddRequest, RuleDelRequest, RuleGetRequest};
use netlink_packet_route::RuleMessage;

pub struct RuleHandle(Handle);

impl RuleHandle {
    pub fn new(handle: Handle) -> Self {
        RuleHandle(handle)
    }

    /// Retrieve the list of route rule entries (equivalent to `ip rule show`)
    pub fn get(&self, ip_version: IpVersion) -> RuleGetRequest {
        RuleGetRequest::new(self.0.clone(), ip_version)
    }

    /// Add a route rule entry (equivalent to `ip rule add`)
    pub fn add(&self) -> RuleAddRequest {
        RuleAddRequest::new(self.0.clone())
    }

    /// Delete the given route rule entry (equivalent to `ip rule del`)
    pub fn del(&self, rule: RuleMessage) -> RuleDelRequest {
        RuleDelRequest::new(self.0.clone(), rule)
    }
}
