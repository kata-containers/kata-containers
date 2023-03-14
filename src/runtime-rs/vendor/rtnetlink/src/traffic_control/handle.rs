// SPDX-License-Identifier: MIT

use super::{
    QDiscDelRequest,
    QDiscGetRequest,
    QDiscNewRequest,
    TrafficChainGetRequest,
    TrafficClassGetRequest,
    TrafficFilterGetRequest,
    TrafficFilterNewRequest,
};

use crate::{
    packet::{TcMessage, NLM_F_CREATE, NLM_F_EXCL, NLM_F_REPLACE},
    Handle,
};

pub struct QDiscHandle(Handle);

impl QDiscHandle {
    pub fn new(handle: Handle) -> Self {
        QDiscHandle(handle)
    }

    /// Retrieve the list of qdisc (equivalent to `tc qdisc show`)
    pub fn get(&mut self) -> QDiscGetRequest {
        QDiscGetRequest::new(self.0.clone())
    }

    /// Create a new qdisc, don't replace if the object already exists.
    /// ( equivalent to `tc qdisc add dev STRING`)
    pub fn add(&mut self, index: i32) -> QDiscNewRequest {
        let msg = TcMessage::with_index(index);
        QDiscNewRequest::new(self.0.clone(), msg, NLM_F_EXCL | NLM_F_CREATE)
    }

    /// Change the qdisc, the handle cannot be changed and neither can the parent.
    /// In other words, change cannot move a node.
    /// ( equivalent to `tc qdisc change dev STRING`)
    pub fn change(&mut self, index: i32) -> QDiscNewRequest {
        let msg = TcMessage::with_index(index);
        QDiscNewRequest::new(self.0.clone(), msg, 0)
    }

    /// Replace existing matching qdisc, create qdisc if it doesn't already exist.
    /// ( equivalent to `tc qdisc replace dev STRING`)
    pub fn replace(&mut self, index: i32) -> QDiscNewRequest {
        let msg = TcMessage::with_index(index);
        QDiscNewRequest::new(self.0.clone(), msg, NLM_F_CREATE | NLM_F_REPLACE)
    }

    /// Performs a replace where the node must exist already.
    /// ( equivalent to `tc qdisc link dev STRING`)
    pub fn link(&mut self, index: i32) -> QDiscNewRequest {
        let msg = TcMessage::with_index(index);
        QDiscNewRequest::new(self.0.clone(), msg, NLM_F_REPLACE)
    }

    /// Delete the qdisc ( equivalent to `tc qdisc del dev STRING`)
    pub fn del(&mut self, index: i32) -> QDiscDelRequest {
        let msg = TcMessage::with_index(index);
        QDiscDelRequest::new(self.0.clone(), msg)
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

    /// Add a filter to a node, don't replace if the object already exists.
    /// ( equivalent to `tc filter add dev STRING`)
    pub fn add(&mut self) -> TrafficFilterNewRequest {
        TrafficFilterNewRequest::new(self.handle.clone(), self.ifindex, NLM_F_EXCL | NLM_F_CREATE)
    }

    /// Change the filter, the handle cannot be changed and neither can the parent.
    /// In other words, change cannot move a node.
    /// ( equivalent to `tc filter change dev STRING`)
    pub fn change(&mut self) -> TrafficFilterNewRequest {
        TrafficFilterNewRequest::new(self.handle.clone(), self.ifindex, 0)
    }

    /// Replace existing matching filter, create filter if it doesn't already exist.
    /// ( equivalent to `tc filter replace dev STRING`)
    pub fn replace(&mut self) -> TrafficFilterNewRequest {
        TrafficFilterNewRequest::new(self.handle.clone(), self.ifindex, NLM_F_CREATE)
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
