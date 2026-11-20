// Copyright 2026 Kata Contributors
// SPDX-License-Identifier: Apache-2.0

use anyhow::{ensure, Context, Result};

use super::DragonballInner;
use crate::device::driver::Address;
use crate::{DeviceType, NetworkConfig};

const TEMPLATE_INTERFACE: &str = "eth0";

impl DragonballInner {
    pub(super) fn prepare_template_network(&mut self) -> Result<()> {
        let template = &self.config.vm_template;
        if !self.config.factory.preplug_network
            || !(template.boot_to_be_template || template.boot_from_template)
        {
            return Ok(());
        }
        ensure!(
            self.config.network_info.disable_vhost_net,
            "template network preplug requires in-VMM virtio-net (disable_vhost_net = true)"
        );

        let mut nics = self.pending_devices.iter().filter(|dev| {
            matches!(
                dev,
                DeviceType::Network(_) | DeviceType::VhostUserNetwork(_)
            )
        });
        if template.boot_from_template {
            ensure!(
                matches!(nics.next(), Some(DeviceType::Network(dev))
                    if dev.config.virt_iface_name == TEMPLATE_INTERFACE
                    && !dev.config.host_dev_name.is_empty()
                    && dev.config.guest_mac.is_some())
                    && nics.next().is_none(),
                "restoring a template with a preplugged NIC requires one eth0 with a TAP and guest MAC"
            );
            // The normal cold-plug path opens this sandbox's TAP before the
            // VMM replays the virtqueues for the matching interface ID.
            return Ok(());
        }
        ensure!(
            nics.next().is_none(),
            "template already has a network device"
        );
        self.add_net_device(&NetworkConfig {
            // An empty host name asks the kernel for a fresh nonpersistent
            // TAP. Closing the factory VM releases it.
            virt_iface_name: TEMPLATE_INTERFACE.to_owned(),
            guest_mac: Some(Address([0x02, 0, 0, 0, 0, 1])),
            ..Default::default()
        })
        .context("preplug template network interface")
    }
}
