// Copyright (C) 2026 Ant Group. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Snapshot state of the virtio-vsock device.
//!
//! A snapshot preserves guest RAM, so it preserves the guest kernel's
//! AF_VSOCK sockets. It cannot preserve what those sockets were talking to:
//! the host half of a connection is a file descriptor, an epoll registration
//! and a peer process, none of which can be handed to a new VMM process. A
//! restore therefore has to tell the guest that every connection which was
//! live when the snapshot was taken is gone, by sending it a `VSOCK_OP_RST`
//! per connection.
//!
//! All that requires is each connection's port tuple, which is what
//! [`VsockState::reset_connections`] carries.

use serde::{Deserialize, Serialize};

use crate::persist::VirtioDeviceInfoState;

/// Identity of one vsock muxer connection, as recorded in a snapshot.
///
/// A connection is keyed by its host (`local_port`) and guest (`peer_port`)
/// ports. The guest CID belongs to the device, so it is not repeated per
/// connection, and the device id in the enclosing state says which device
/// owns a tuple when a VM has several.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct VsockConnectionId {
    /// Host-side port.
    pub local_port: u32,
    /// Guest-side port.
    pub peer_port: u32,
}

/// Serializable state of a virtio-vsock device.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct VsockState {
    /// Guest-negotiated device state.
    #[serde(flatten)]
    pub device_info: VirtioDeviceInfoState,
    /// Connections that were live when the snapshot was taken. Restore sends
    /// the guest one `VSOCK_OP_RST` per tuple, ahead of any other RX traffic.
    /// Sorted and deduplicated, which keeps the serialized form
    /// deterministic.
    #[serde(default)]
    pub reset_connections: Vec<VsockConnectionId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(local_port: u32, peer_port: u32) -> VsockConnectionId {
        VsockConnectionId {
            local_port,
            peer_port,
        }
    }

    #[test]
    fn test_vsock_state_json_roundtrip() {
        let state = VsockState {
            device_info: VirtioDeviceInfoState {
                avail_features: 0x1_0000_0001,
                acked_features: 1,
                config_space: vec![3, 0, 0, 0, 0, 0, 0, 0],
            },
            reset_connections: vec![id(1023, 9), id(1024, 7)],
        };

        let json = serde_json::to_string(&state).unwrap();
        // `device_info` is flattened, so a snapshot produced before reset
        // support stays readable field-for-field.
        assert!(json.contains("\"acked_features\":1"));
        assert_eq!(serde_json::from_str::<VsockState>(&json).unwrap(), state);
    }

    #[test]
    fn test_vsock_state_without_reset_list_deserializes() {
        let json = serde_json::json!({
            "avail_features": 0u64,
            "acked_features": 0u64,
            "config_space": Vec::<u8>::new(),
        })
        .to_string();

        let state: VsockState = serde_json::from_str(&json).unwrap();
        assert!(state.reset_connections.is_empty());
    }
}
