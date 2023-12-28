// Copyright (C) 2019-2023 Ant Group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use dbs_utils::net::{MacAddr, MAC_ADDR_LEN};
use log::debug;
use virtio_bindings::bindings::virtio_net::{
    VIRTIO_NET_F_MAC, VIRTIO_NET_F_MQ, VIRTIO_NET_F_STATUS, VIRTIO_NET_S_LINK_UP,
};

use crate::{Error, Result};

// Config space of network config:
// https://docs.oasis-open.org/virtio/virtio/v1.1/csprd01/virtio-v1.1-csprd01.html#x1-2000004
// MAC
pub const CONFIG_SPACE_MAC: usize = 0;
// Status
pub const CONFIG_SPACE_STATUS: usize = CONFIG_SPACE_MAC + MAC_ADDR_LEN;
pub const CONFIG_SPACE_STATUS_SIZE: usize = 2;
// Max virtqueue pairs
pub const CONFIG_SPACE_MAX_VIRTQUEUE_PAIRS: usize = CONFIG_SPACE_STATUS + CONFIG_SPACE_STATUS_SIZE;
pub const CONFIG_SPACE_MAX_VIRTQUEUE_PAIRS_SIZE: usize = 2;
// MTU
pub const CONFIG_SPACE_MTU: usize =
    CONFIG_SPACE_MAX_VIRTQUEUE_PAIRS + CONFIG_SPACE_MAX_VIRTQUEUE_PAIRS_SIZE;
pub const CONFIG_SPACE_MTU_SIZE: usize = 2;
// Size of config space
pub const CONFIG_SPACE_SIZE: usize = MAC_ADDR_LEN
    + CONFIG_SPACE_STATUS_SIZE
    + CONFIG_SPACE_MAX_VIRTQUEUE_PAIRS_SIZE
    + CONFIG_SPACE_MTU_SIZE;

// Default MTU for network device
pub const DEFAULT_MTU: u16 = 1500;

/// Setup config space for network device.
pub fn setup_config_space(
    device_name: &str,
    guest_mac: &Option<&MacAddr>,
    avail_features: &mut u64,
    vq_pairs: u16,
    mtu: u16,
) -> Result<Vec<u8>> {
    let mut config_space = vec![0u8; CONFIG_SPACE_SIZE];
    if let Some(mac) = guest_mac.as_ref() {
        config_space[CONFIG_SPACE_MAC..CONFIG_SPACE_MAC + MAC_ADDR_LEN]
            .copy_from_slice(mac.get_bytes());
        // When this feature isn't available, the driver generates a random MAC address.
        // Otherwise, it should attempt to read the device MAC address from the config space.
        *avail_features |= 1u64 << VIRTIO_NET_F_MAC;
    }

    // Mark link as up: status only exists if VIRTIO_NET_F_STATUS is set.
    if *avail_features & (1 << VIRTIO_NET_F_STATUS) != 0 {
        config_space[CONFIG_SPACE_STATUS..CONFIG_SPACE_STATUS + CONFIG_SPACE_STATUS_SIZE]
            .copy_from_slice(&(VIRTIO_NET_S_LINK_UP as u16).to_le_bytes());
    }

    // Set max virtqueue pairs, which only exists if VIRTIO_NET_F_MQ is set.
    if *avail_features & (1 << VIRTIO_NET_F_MQ) != 0 {
        if vq_pairs <= 1 {
            return Err(Error::InvalidInput);
        }
        config_space[CONFIG_SPACE_MAX_VIRTQUEUE_PAIRS
            ..CONFIG_SPACE_MAX_VIRTQUEUE_PAIRS + CONFIG_SPACE_MAX_VIRTQUEUE_PAIRS_SIZE]
            .copy_from_slice(&vq_pairs.to_le_bytes());
    }

    config_space[CONFIG_SPACE_MTU..CONFIG_SPACE_MTU + CONFIG_SPACE_MTU_SIZE]
        .copy_from_slice(&mtu.to_le_bytes());

    debug!(
        "{}: config space is set to {:X?}, guest_mac: {:?}, avail_feature: 0x{:X}, vq_pairs: {}, mtu: {}",
        device_name, config_space, guest_mac, avail_features, vq_pairs, mtu
    );

    Ok(config_space)
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    use super::*;

    #[test]
    fn test_set_config_space() {
        let mac = MacAddr::parse_str("bf:b7:72:50:82:00").unwrap();
        let mut afeatures: u64;
        let mut vq_pairs: u16;
        // Avail features: VIRTIO_NET_F_STATUS + VIRTIO_NET_F_MQ
        {
            afeatures = 0;
            vq_pairs = 2;
            afeatures |= 1 << VIRTIO_NET_F_STATUS | 1 << VIRTIO_NET_F_MQ;

            let cs = setup_config_space(
                "virtio-net",
                &Some(&mac),
                &mut afeatures,
                vq_pairs,
                DEFAULT_MTU,
            )
            .unwrap();

            // Mac
            assert_eq!(
                mac.get_bytes(),
                &cs[CONFIG_SPACE_MAC..CONFIG_SPACE_MAC + MAC_ADDR_LEN]
            );
            // Status
            assert_eq!(
                VIRTIO_NET_S_LINK_UP as u16,
                u16::from_le_bytes(
                    cs[CONFIG_SPACE_STATUS..CONFIG_SPACE_STATUS + CONFIG_SPACE_STATUS_SIZE]
                        .try_into()
                        .unwrap()
                )
            );
            // Max virtqueue pairs
            assert_eq!(
                vq_pairs,
                u16::from_le_bytes(
                    cs[CONFIG_SPACE_MAX_VIRTQUEUE_PAIRS
                        ..CONFIG_SPACE_MAX_VIRTQUEUE_PAIRS + CONFIG_SPACE_MAX_VIRTQUEUE_PAIRS_SIZE]
                        .try_into()
                        .unwrap()
                )
            );
            // MTU
            assert_eq!(
                DEFAULT_MTU,
                u16::from_le_bytes(
                    cs[CONFIG_SPACE_MTU..CONFIG_SPACE_MTU + CONFIG_SPACE_MTU_SIZE]
                        .try_into()
                        .unwrap()
                )
            );
        }
        // No avail features
        {
            afeatures = 0;
            vq_pairs = 1;

            let cs = setup_config_space(
                "virtio-net",
                &Some(&mac),
                &mut afeatures,
                vq_pairs,
                DEFAULT_MTU,
            )
            .unwrap();

            // Status
            assert_eq!(
                0,
                u16::from_le_bytes(
                    cs[CONFIG_SPACE_STATUS..CONFIG_SPACE_STATUS + CONFIG_SPACE_STATUS_SIZE]
                        .try_into()
                        .unwrap()
                )
            );
            // Max virtqueue pairs
            assert_eq!(
                0,
                u16::from_le_bytes(
                    cs[CONFIG_SPACE_MAX_VIRTQUEUE_PAIRS
                        ..CONFIG_SPACE_MAX_VIRTQUEUE_PAIRS + CONFIG_SPACE_MAX_VIRTQUEUE_PAIRS_SIZE]
                        .try_into()
                        .unwrap()
                )
            );
        }
        // Avail features: VIRTIO_NET_F_MQ and invalid value of vq_pairs
        {
            afeatures = 0;
            vq_pairs = 1;
            afeatures |= 1 << VIRTIO_NET_F_MQ;

            let cs = setup_config_space(
                "virtio-net",
                &Some(&mac),
                &mut afeatures,
                vq_pairs,
                DEFAULT_MTU,
            );
            assert!(cs.is_err());
        }
    }
}
