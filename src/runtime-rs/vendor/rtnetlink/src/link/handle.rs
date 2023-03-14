// SPDX-License-Identifier: MIT

use super::{
    LinkAddRequest,
    LinkDelPropRequest,
    LinkDelRequest,
    LinkGetRequest,
    LinkNewPropRequest,
    LinkSetRequest,
};
use crate::Handle;

pub struct LinkHandle(Handle);

impl LinkHandle {
    pub fn new(handle: Handle) -> Self {
        LinkHandle(handle)
    }

    pub fn set(&self, index: u32) -> LinkSetRequest {
        LinkSetRequest::new(self.0.clone(), index)
    }

    pub fn add(&self) -> LinkAddRequest {
        LinkAddRequest::new(self.0.clone())
    }

    pub fn property_add(&self, index: u32) -> LinkNewPropRequest {
        LinkNewPropRequest::new(self.0.clone(), index)
    }

    pub fn property_del(&self, index: u32) -> LinkDelPropRequest {
        LinkDelPropRequest::new(self.0.clone(), index)
    }

    pub fn del(&mut self, index: u32) -> LinkDelRequest {
        LinkDelRequest::new(self.0.clone(), index)
    }

    /// Retrieve the list of links (equivalent to `ip link show`)
    pub fn get(&mut self) -> LinkGetRequest {
        LinkGetRequest::new(self.0.clone())
    }
}
