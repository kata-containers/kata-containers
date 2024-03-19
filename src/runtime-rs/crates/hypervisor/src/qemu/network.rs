// Copyright (c) 2024 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use std::convert::TryFrom;
use std::fs::{File, OpenOptions};
use std::os::fd::RawFd;

use crate::utils::{clear_cloexec, open_named_tuntap};
use crate::{Address, NetworkConfig};
use anyhow::{anyhow, Context, Result};

// VirtioTransport is the transport in use for a virtio device.
#[derive(Debug, Default, PartialEq)]
enum VirtioTransport {
    #[default]
    Pci,
}

impl ToString for VirtioTransport {
    fn to_string(&self) -> String {
        match self {
            VirtioTransport::Pci => "pci".to_owned(),
        }
    }
}

impl TryFrom<&str> for VirtioTransport {
    type Error = anyhow::Error;

    fn try_from(_transport: &str) -> Result<Self> {
        Ok(VirtioTransport::Pci)
    }
}

// DeviceDriver is set in "-device driver=<DeviceDriver>"
#[derive(Debug, Default, PartialEq)]
enum DeviceDriver {
    // VirtioNetPci("virtio-net-pci") is a virtio-net device using PCI transport.
    #[default]
    VirtioNetPci,

    // VfioPci("vfio-pci") is an attached host device using PCI transport.
    VfioPci,
}

impl ToString for DeviceDriver {
    fn to_string(&self) -> String {
        match self {
            DeviceDriver::VirtioNetPci => "virtio-net-pci".to_owned(),
            DeviceDriver::VfioPci => "vfio-pci".to_owned(),
        }
    }
}

impl TryFrom<&str> for DeviceDriver {
    type Error = anyhow::Error;

    fn try_from(device_driver: &str) -> Result<Self> {
        Ok(match device_driver {
            "virtio-net-pci" => DeviceDriver::VirtioNetPci,
            "vfio-pci" => DeviceDriver::VfioPci,
            _ => return Err(anyhow!("unsupported transport")),
        })
    }
}

#[derive(Debug, Default, PartialEq)]
enum NetDev {
    /// Tap("tap") is a tap networking device type.
    #[default]
    Tap,

    /// MacTap("macvtap") is a macvtap networking device type.
    #[allow(dead_code)]
    MacvTap,
}

impl ToString for NetDev {
    fn to_string(&self) -> String {
        match self {
            NetDev::Tap | NetDev::MacvTap => "tap".to_owned(),
            // VhostUser is to be added in future.
            // NetDev::VhostUser => "vhost-user".to_owned(),
        }
    }
}

// NetDevice represents a guest networking device
// -netdev tap,id=hostnet0,vhost=on,vhostfds=x:y:z,fds=a:b:c
// -device virtio-net-pci,netdev=hostnet0,id=net0,mac=24:42:54:20:50:46,bus=pci.0,addr=0x7
#[derive(Debug, Default)]
pub struct NetDevice {
    // device_type is the netdev type (e.g. tap).
    device_type: NetDev,

    // driver is the qemu device driver
    device_driver: DeviceDriver,

    // id is the net device identifier.
    id: String,

    // if_name is the interface name,
    if_name: String,

    // bus is the bus path name of a PCI device.
    bus: String,

    // pci_addr is the address offset of a PCI device.
    pci_addr: String,

    // fds represents the list of already existing file descriptors to be used.
    // This is mostly useful for mq support.
    // {
    //      fds:      Vec<File>,
    //      vhost_fds: Vec<File>,
    // }
    fds: HashMap<String, Vec<RawFd>>,

    // disable_vhost_net disables virtio device emulation from the host kernel instead of from qemu.
    disable_vhost_net: bool,

    // mac_address is the networking device interface MAC address.
    mac_address: Address,

    // disable_modern prevents qemu from relying on fast MMIO.
    disable_modern: bool,

    // transport is the virtio transport for this device.
    transport: VirtioTransport,
}

impl NetDevice {
    #[allow(dead_code)]
    pub fn new(
        config: &NetworkConfig,
        disable_vhost_net: bool,
        tun_fds: Vec<i32>,
        vhost_fds: Vec<i32>,
    ) -> Self {
        // we have only two <Key, Value>s:
        // {
        //      "fds": vec![fd1, fd2,...],
        //      "vhostfds": vec![fd3, fd4,...],
        // }
        let mut fds: HashMap<String, Vec<RawFd>> = HashMap::with_capacity(2);
        fds.insert("fds".to_owned(), tun_fds);
        fds.insert("vhostfds".to_owned(), vhost_fds);

        // FIXME(Hard Code): It's safe to unwrap here because of the valid input.
        // Ideally device_driver should be derived from transport to minimize code duplication.
        // While currently we focus on PCI for the initial implementation.
        // And we'll support other transports, e.g. s390x's CCW.
        let device_driver = DeviceDriver::try_from("virtio-net-pci").unwrap();
        let transport = VirtioTransport::try_from("pci").unwrap();

        NetDevice {
            device_type: NetDev::Tap,
            device_driver,
            id: format!("network-{}", &config.index),
            if_name: config.virt_iface_name.clone(),
            mac_address: config.guest_mac.clone().unwrap(),
            disable_vhost_net,
            disable_modern: false,
            fds,
            transport,
            ..Default::default()
        }
    }

    fn mq_param(&self) -> String {
        let mut params = vec!["mq=on".to_owned()];
        if self.transport == VirtioTransport::Pci {
            // https://www.linux-kvm.org/page/Multiqueue
            // -netdev tap,vhost=on,queues=N
            // enable mq and specify msix vectors in qemu cmdline
            // (2N+2 vectors, N for tx queues, N for rx queues, 1 for config, and one for possible control vq)
            // -device virtio-net-pci,mq=on,vectors=2N+2...
            // enable mq in guest by 'ethtool -L eth0 combined $queue_num'
            // Clearlinux automatically sets up the queues properly
            // The agent implementation should do this to ensure that it is
            // always set

            // vectors = len(netdev.FDs) * 2 + 2
            if let Some(fds) = self.fds.get("fds") {
                params.push(format!("vectors={}", 2 * fds.len() + 2));
            }
        }

        params.join(",")
    }

    pub fn qemu_device_params(&self) -> Result<Vec<String>> {
        let mut device_params: Vec<String> = Vec::new();

        device_params.push(format!("driver={}", &self.device_driver.to_string()));
        device_params.push(format!("netdev={}", &self.id));

        let mac = self.mac_address.to_string();
        device_params.push(format!("mac={}", &mac));

        if !self.bus.is_empty() {
            device_params.push(format!("bus={}", &self.bus));
        }

        if !self.pci_addr.is_empty() {
            // FIXME: pci_addr: PciPath
            device_params.push(format!("addr={}", &self.pci_addr));
        }

        device_params.push(format!(
            "disable-modern={}",
            if self.disable_modern { "true" } else { "false" }
        ));

        if !self.fds.is_empty() {
            device_params.push(self.mq_param());
        }

        Ok(device_params)
    }

    pub fn qemu_netdev_params(&self) -> Result<Vec<String>> {
        let mut netdev_params: Vec<String> = Vec::new();
        let netdev_type = self.device_type.to_string();
        netdev_params.push(netdev_type);
        netdev_params.push(format!("id={}", self.id));

        if !self.disable_vhost_net {
            netdev_params.push("vhost=on".to_owned());
            if let Some(vhost_fds) = self.fds.get("vhostfds") {
                for fd in vhost_fds.iter() {
                    clear_cloexec(*fd).context("clearing O_CLOEXEC failed on vhost fd")?;
                }
                let s = vhost_fds
                    .iter()
                    .map(|&n| n.to_string())
                    .collect::<Vec<String>>()
                    .join(":");

                netdev_params.push(format!("vhostfds={}", s));
            }
        }

        if let Some(tuntap_fds) = self.fds.get("fds") {
            for fd in tuntap_fds.iter() {
                clear_cloexec(*fd).context("clearing O_CLOEXEC failed on tuntap fd")?;
            }
            let s = tuntap_fds
                .iter()
                .map(|&n| n.to_string())
                .collect::<Vec<String>>()
                .join(":");
            netdev_params.push(format!("fds={}", s));
        } else {
            netdev_params.push(format!("ifname={}", self.if_name));
            netdev_params.push("script=no".to_owned());
            netdev_params.push("downscript=no".to_owned());
        }

        Ok(netdev_params)
    }
}

impl ToString for Address {
    fn to_string(&self) -> String {
        let b: [u8; 6] = self.0;

        format!(
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            b[0], b[1], b[2], b[3], b[4], b[5]
        )
    }
}

// /dev/tap$(cat /sys/class/net/macvtap1/ifindex)
// for example: /dev/tap2381
#[allow(dead_code)]
pub fn create_macvtap_fds(ifindex: u32, queues: u32) -> Result<Vec<File>> {
    let macvtap = format!("/dev/tap{}", ifindex);
    create_fds(macvtap.as_str(), queues as usize)
}

pub fn create_vhost_net_fds(queues: u32) -> Result<Vec<File>> {
    let vhost_dev = "/dev/vhost-net";
    let num_fds = if queues > 1 { queues as usize } else { 1_usize };

    create_fds(vhost_dev, num_fds)
}

// For example: if num_fds = 3; fds = {0xc000012028, 0xc000012030, 0xc000012038}
fn create_fds(device: &str, num_fds: usize) -> Result<Vec<File>> {
    let mut fds: Vec<File> = Vec::with_capacity(num_fds);

    for i in 0..num_fds {
        match OpenOptions::new().read(true).write(true).open(device) {
            Ok(f) => {
                fds.push(f);
            }
            Err(e) => {
                fds.clear();
                return Err(anyhow!(
                    "It failed with error {:?} when opened the {:?} device.",
                    e,
                    i
                ));
            }
        };
    }

    Ok(fds)
}

pub fn generate_netdev_fds(
    network_config: &NetworkConfig,
    queues: u32,
) -> Result<(Vec<File>, Vec<File>)> {
    let if_name = network_config.host_dev_name.as_str();
    let tun_taps = open_named_tuntap(if_name, queues)?;
    let vhost_fds = create_vhost_net_fds(queues)?;

    Ok((tun_taps, vhost_fds))
}

#[cfg(test)]
mod tests {
    use super::create_fds;

    #[test]
    fn test_ctreate_fds() {
        let device = "/dev/null";
        let num_fds = 3_usize;
        let fds = create_fds(device, num_fds);
        assert!(fds.is_ok());
        assert_eq!(fds.unwrap().len(), num_fds);
    }
}
