// Copyright (C) 2019-2023 Alibaba Cloud. All rights reserved.
// Copyright (C) 2019-2023 Ant Group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use dbs_utils::net::MacAddr;
use serde::{Deserialize, Serialize};

#[cfg(feature = "virtio-net")]
use super::{VirtioNetDeviceConfigInfo, VirtioNetDeviceConfigUpdateInfo};
use crate::config_manager::RateLimiterConfigInfo;
#[cfg(feature = "vhost-net")]
use crate::device_manager::vhost_net_dev_mgr::{self, VhostNetDeviceConfigInfo};
#[cfg(feature = "virtio-net")]
use crate::device_manager::virtio_net_dev_mgr;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "type", deny_unknown_fields)]
/// An enum to specify a backend of Virtio network
pub enum Backend {
    #[serde(rename = "virtio")]
    #[cfg(feature = "virtio-net")]
    /// Virtio-net
    Virtio(VirtioConfig),
    #[serde(rename = "vhost")]
    #[cfg(feature = "vhost-net")]
    /// Vhost-net
    Vhost(VirtioConfig),
}

impl Default for Backend {
    fn default() -> Self {
        Self::Virtio(VirtioConfig::default())
    }
}

/// Virtio network config, working for virtio-net and vhost-net.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct VirtioConfig {
    /// ID of the guest network interface.
    pub iface_id: String,
    /// Host level path for the guest network interface.
    pub host_dev_name: String,
    /// Rate Limiter for received packages.
    pub rx_rate_limiter: Option<RateLimiterConfigInfo>,
    /// Rate Limiter for transmitted packages.
    pub tx_rate_limiter: Option<RateLimiterConfigInfo>,
    /// Allow duplicate mac
    pub allow_duplicate_mac: bool,
}

/// This struct represents the strongly typed equivalent of the json body from
/// net iface related requests.
/// This struct works with virtio-net devices and vhost-net devices.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub struct NetworkInterfaceConfig {
    /// Number of virtqueue pairs to use. (https://www.linux-kvm.org/page/Multiqueue)
    pub num_queues: Option<usize>,
    /// Size of each virtqueue.
    pub queue_size: Option<u16>,
    /// Net backend driver.
    #[serde(default = "Backend::default")]
    pub backend: Backend,
    /// mac of the interface.
    pub guest_mac: Option<MacAddr>,
    /// Use shared irq
    pub use_shared_irq: Option<bool>,
    /// Use generic irq
    pub use_generic_irq: Option<bool>,
}

#[cfg(feature = "virtio-net")]
impl From<NetworkInterfaceConfig> for VirtioNetDeviceConfigInfo {
    fn from(value: NetworkInterfaceConfig) -> Self {
        let self_ref = &value;
        self_ref.into()
    }
}

#[cfg(feature = "virtio-net")]
impl From<&NetworkInterfaceConfig> for VirtioNetDeviceConfigInfo {
    fn from(value: &NetworkInterfaceConfig) -> Self {
        let queue_size = value
            .queue_size
            .unwrap_or(virtio_net_dev_mgr::DEFAULT_QUEUE_SIZE);

        // It is safe because we tested the type of config before.
        let config = match &value.backend {
            Backend::Virtio(config) => config,
            _ => panic!("The virtio backend config is invalid: {:?}", value),
        };

        Self {
            iface_id: config.iface_id.clone(),
            host_dev_name: config.host_dev_name.clone(),
            num_queues: virtio_net_dev_mgr::DEFAULT_NUM_QUEUES,
            queue_size,
            guest_mac: value.guest_mac,
            rx_rate_limiter: config.rx_rate_limiter.clone(),
            tx_rate_limiter: config.tx_rate_limiter.clone(),
            allow_duplicate_mac: config.allow_duplicate_mac,
            use_shared_irq: value.use_shared_irq,
            use_generic_irq: value.use_generic_irq,
        }
    }
}

#[cfg(feature = "vhost-net")]
impl From<NetworkInterfaceConfig> for VhostNetDeviceConfigInfo {
    fn from(value: NetworkInterfaceConfig) -> Self {
        let self_ref = &value;
        self_ref.into()
    }
}

#[cfg(feature = "vhost-net")]
impl From<&NetworkInterfaceConfig> for VhostNetDeviceConfigInfo {
    fn from(value: &NetworkInterfaceConfig) -> Self {
        let num_queues = value
            .num_queues
            .unwrap_or(vhost_net_dev_mgr::DEFAULT_NUM_QUEUES);
        let queue_size = value
            .queue_size
            .unwrap_or(vhost_net_dev_mgr::DEFAULT_QUEUE_SIZE);

        // It is safe because we tested the type of config before.
        let config = match &value.backend {
            Backend::Vhost(config) => config,
            _ => panic!("The virtio backend config is invalid: {:?}", value),
        };

        Self {
            iface_id: config.iface_id.clone(),
            host_dev_name: config.host_dev_name.clone(),
            num_queues,
            vq_pairs: num_queues / 2,
            queue_size,
            guest_mac: value.guest_mac,
            allow_duplicate_mac: config.allow_duplicate_mac,
            use_shared_irq: value.use_shared_irq,
            use_generic_irq: value.use_generic_irq,
        }
    }
}

/// The data fed into a network iface update request. Currently, only the RX and
/// TX rate limiters can be updated.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub struct NetworkInterfaceUpdateConfig {
    /// ID of the guest network interface.
    pub iface_id: String,
    /// New RX rate limiter config. Only provided data will be updated. I.e. if any optional data
    /// is missing, it will not be nullified, but left unchanged.
    pub rx_rate_limiter: Option<RateLimiterConfigInfo>,
    /// New TX rate limiter config. Only provided data will be updated. I.e. if any optional data
    /// is missing, it will not be nullified, but left unchanged.
    pub tx_rate_limiter: Option<RateLimiterConfigInfo>,
}

#[cfg(feature = "virtio-net")]
impl From<NetworkInterfaceUpdateConfig> for VirtioNetDeviceConfigUpdateInfo {
    fn from(value: NetworkInterfaceUpdateConfig) -> Self {
        let self_ref = &value;
        self_ref.into()
    }
}

#[cfg(feature = "virtio-net")]
impl From<&NetworkInterfaceUpdateConfig> for VirtioNetDeviceConfigUpdateInfo {
    fn from(value: &NetworkInterfaceUpdateConfig) -> Self {
        Self {
            iface_id: value.iface_id.clone(),
            rx_rate_limiter: value.rx_rate_limiter.clone(),
            tx_rate_limiter: value.tx_rate_limiter.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use dbs_utils::net::MacAddr;

    use crate::api::v1::Backend;

    use super::NetworkInterfaceConfig;

    #[test]
    fn test_network_interface_config() {
        let json_str = r#"{
            "num_queues": 4,
            "queue_size": 512,
            "backend": {
                "type": "virtio",
                "iface_id": "eth0",
                "host_dev_name": "tap0",
                "allow_duplicate_mac": true
            },
            "guest_mac": "81:87:1D:00:08:A9"
        }"#;
        let net_config: NetworkInterfaceConfig = serde_json::from_str(json_str).unwrap();
        assert_eq!(net_config.num_queues, Some(4));
        assert_eq!(net_config.queue_size, Some(512));
        assert_eq!(
            net_config.guest_mac,
            Some(MacAddr::from_bytes(&[129, 135, 29, 0, 8, 169]).unwrap())
        );
        if let Backend::Virtio(config) = net_config.backend {
            assert_eq!(config.iface_id, "eth0");
            assert_eq!(config.host_dev_name, "tap0");
            assert!(config.allow_duplicate_mac);
        } else {
            panic!("Unexpected backend type");
        }
    }
}
