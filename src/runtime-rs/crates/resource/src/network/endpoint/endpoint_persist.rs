// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct PhysicalEndpointState {
    pub bdf: String,
    pub driver: String,
    pub vendor_id: String,
    pub device_id: String,
    pub hard_addr: String,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct MacvlanEndpointState {
    pub if_name: String,
    pub network_qos: bool,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct VlanEndpointState {
    pub if_name: String,
    pub network_qos: bool,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct VethEndpointState {
    pub if_name: String,
    pub network_qos: bool,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct IpVlanEndpointState {
    pub if_name: String,
    pub network_qos: bool,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct TapEndpointState {
    pub if_name: String,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct VhostUserEndpointState {
    pub if_name: String,
    pub socket_path: String,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct EndpointState {
    pub physical_endpoint: Option<PhysicalEndpointState>,
    pub veth_endpoint: Option<VethEndpointState>,
    pub ipvlan_endpoint: Option<IpVlanEndpointState>,
    pub macvlan_endpoint: Option<MacvlanEndpointState>,
    pub vlan_endpoint: Option<VlanEndpointState>,
    pub tap_endpoint: Option<TapEndpointState>,
    pub vhost_user_endpoint: Option<VhostUserEndpointState>,
    // TODO : other endpoint
}
