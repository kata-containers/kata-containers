// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

//! Backend-independent OVS port configuration and operations.

use std::collections::BTreeMap;
use std::net::IpAddr;

use dan_core::HostPortConfig;

use crate::OvsError;

/// Configuration for one interface and its matching single-interface OVS port.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OvsPortSpec {
    pub bridge_name: String,
    pub port_name: String,
    pub interface_type: String,
    /// Option keys managed by the caller; other Interface options are preserved.
    pub options: BTreeMap<String, String>,
    pub mtu_request: Option<u32>,
    pub iface_id: String,
    pub encapsulation_ip: Option<IpAddr>,
    pub owner_id: String,
}

/// A consistent snapshot of the interface, port, and owning bridge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OvsPortInfo {
    pub port_uuid: String,
    pub interface_uuid: String,
    pub bridge_uuid: String,
    pub bridge_name: String,
    pub port_name: String,
    pub port_owner: Option<String>,
    pub interface_owner: Option<String>,
    pub iface_id: Option<String>,
    pub encapsulation_ip: Option<IpAddr>,
    pub ovn_installed: bool,
    pub interface_type: String,
    pub options: BTreeMap<String, String>,
    pub mtu_request: Option<u32>,
    pub ofport: Option<i64>,
    pub interface_error: Option<String>,
}

/// OVS operations shared by endpoint backends.
pub trait OvsControl: Send + Sync {
    fn inspect(&self, port_name: &str) -> Result<Option<OvsPortInfo>, OvsError>;

    /// Read only the interface's readiness while waiting for OVS/OVN.
    fn interface_ready(&self, port_name: &str) -> Result<bool, OvsError> {
        self.inspect(port_name)?
            .ok_or_else(|| {
                OvsError::OperationFailed(format!("OVS interface `{port_name}` does not exist"))
            })?
            .is_ready()
    }

    /// Inspect resource identity and ownership without requiring valid configuration.
    fn inspect_owned(&self, port_name: &str) -> Result<Option<OvsPortInfo>, OvsError> {
        self.inspect(port_name)
    }

    fn interfaces_with_iface_id(&self, iface_id: &str) -> Result<Vec<String>, OvsError>;

    /// Atomically create an exclusive binding owned by this attachment.
    /// A failed snapshot guard returns TransactionConflict without committing.
    fn add_port(&self, spec: &OvsPortSpec) -> Result<(), OvsError>;

    /// Create a port with its initial traffic policy in the same transaction.
    fn add_port_with_config(
        &self,
        spec: &OvsPortSpec,
        config: &HostPortConfig,
    ) -> Result<(), OvsError> {
        if config.is_empty() {
            self.add_port(spec)
        } else {
            Err(OvsError::InvalidInput(
                "OVS control does not support initial host port policies".into(),
            ))
        }
    }

    /// Delete the inspected resource only while its exact identity and owner match.
    fn delete_owned_port(&self, info: &OvsPortInfo, owner_id: &str) -> Result<(), OvsError>;

    /// Return the datapath type, or `None` when the bridge does not exist.
    fn bridge_datapath_type(&self, bridge_name: &str) -> Result<Option<String>, OvsError> {
        Err(OvsError::OperationFailed(format!(
            "OVS control does not support querying datapath type for `{bridge_name}`"
        )))
    }
}
