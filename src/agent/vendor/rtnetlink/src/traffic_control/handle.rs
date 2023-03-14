use super::{
    QDiscGetRequest,
    TrafficChainGetRequest,
    TrafficClassGetRequest,
    TrafficFilterGetRequest,
};
use crate::Handle;

pub struct QDiscHandle(Handle);

impl QDiscHandle {
    pub fn new(handle: Handle) -> Self {
        QDiscHandle(handle)
    }

    /// Retrieve the list of qdisc (equivalent to `tc qdisc show`)
    pub fn get(&mut self) -> QDiscGetRequest {
        QDiscGetRequest::new(self.0.clone())
    }
}

pub struct TrafficClassHandle {
    handle: Handle,
    ifindex: i32,
}

impl TrafficClassHandle {
    pub fn new(handle: Handle, ifindex: i32) -> Self {
        TrafficClassHandle { handle, ifindex }
    }

    /// Retrieve the list of traffic class (equivalent to
    /// `tc class show dev <interface_name>`)
    pub fn get(&mut self) -> TrafficClassGetRequest {
        TrafficClassGetRequest::new(self.handle.clone(), self.ifindex)
    }
}

pub struct TrafficFilterHandle {
    handle: Handle,
    ifindex: i32,
}

impl TrafficFilterHandle {
    pub fn new(handle: Handle, ifindex: i32) -> Self {
        TrafficFilterHandle { handle, ifindex }
    }

    /// Retrieve the list of filter (equivalent to
    /// `tc filter show dev <iface_name>`)
    pub fn get(&mut self) -> TrafficFilterGetRequest {
        TrafficFilterGetRequest::new(self.handle.clone(), self.ifindex)
    }
}

pub struct TrafficChainHandle {
    handle: Handle,
    ifindex: i32,
}

impl TrafficChainHandle {
    pub fn new(handle: Handle, ifindex: i32) -> Self {
        TrafficChainHandle { handle, ifindex }
    }

    /// Retrieve the list of chain (equivalent to
    /// `tc chain show dev <iface_name>`)
    pub fn get(&mut self) -> TrafficChainGetRequest {
        TrafficChainGetRequest::new(self.handle.clone(), self.ifindex)
    }
}
