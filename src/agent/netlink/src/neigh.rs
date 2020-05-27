// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::{
    __s32, __u16, __u8, addattr_var, ifinfomsg, nlmsghdr, parse_ipaddr, IFA_F_PERMANENT,
    NLMSG_ALIGNTO, NLM_F_CREATE, NLM_F_EXCL, NLM_F_REQUEST, RTM_NEWNEIGH,
};
use crate::{NLMSG_ALIGN, NLMSG_DATA, NLMSG_HDRLEN, NLMSG_LENGTH};
use protocols::types::ARPNeighbor;
use rustjail::errors::*;
use std::mem;

#[repr(C)]
#[derive(Copy)]
pub struct ndmsg {
    ndm_family: __u8,
    ndm_pad1: __u8,
    ndm_pad: __u16,
    ndm_ifindex: __s32,
    ndm_state: __u16,
    ndm_flags: __u8,
    ndm_type: __u8,
}

pub const NDA_UNSPEC: __u16 = 0;
pub const NDA_DST: __u16 = 1;
pub const NDA_LLADDR: __u16 = 2;
pub const NDA_CACHEINFO: __u16 = 3;
pub const NDA_PROBES: __u16 = 4;
pub const NDA_VLAN: __u16 = 5;
pub const NDA_PORT: __u16 = 6;
pub const NDA_VNI: __u16 = 7;
pub const NDA_IFINDEX: __u16 = 8;
pub const NDA_MASTER: __u16 = 9;
pub const NDA_LINK_NETNSID: __u16 = 10;
pub const NDA_SRC_VNI: __u16 = 11;
pub const __NDA_MAX: __u16 = 12;

impl Clone for ndmsg {
    fn clone(&self) -> Self {
        Self { ..*self }
    }
}

impl Default for ndmsg {
    fn default() -> Self {
        unsafe { mem::zeroed::<Self>() }
    }
}

impl crate::RtnlHandle {
    pub fn add_arp_neighbors(&mut self, neighs: &Vec<ARPNeighbor>) -> Result<()> {
        for neigh in neighs {
            self.add_one_arp_neighbor(&neigh)?;
        }

        Ok(())
    }
    pub fn add_one_arp_neighbor(&mut self, neigh: &ARPNeighbor) -> Result<()> {
        let dev: ifinfomsg;

        match self.find_link_by_name(&neigh.device) {
            Ok(d) => dev = d,
            Err(e) => {
                return Err(ErrorKind::ErrorCode(format!(
                    "Could not find link from device {}: {}",
                    neigh.device, e
                ))
                .into());
            }
        }

        if neigh.toIPAddress.is_none() {
            return Err(ErrorKind::ErrorCode("toIPAddress is required".to_string()).into());
        }

        let to_ip = &neigh.toIPAddress.as_ref().unwrap().address;
        if to_ip.is_empty() {
            return Err(ErrorKind::ErrorCode("toIPAddress.address is required".to_string()).into());
        }

        let mut v: Vec<u8> = vec![0; 2048];
        unsafe {
            // init
            let mut nlh: *mut nlmsghdr = v.as_mut_ptr() as *mut nlmsghdr;
            let mut ndm: *mut ndmsg = NLMSG_DATA!(nlh) as *mut ndmsg;

            (*nlh).nlmsg_len = NLMSG_LENGTH!(mem::size_of::<ndmsg>()) as u32;
            (*nlh).nlmsg_type = RTM_NEWNEIGH;
            (*nlh).nlmsg_flags = NLM_F_REQUEST | NLM_F_CREATE | NLM_F_EXCL;

            self.seq += 1;
            self.dump = self.seq;
            (*nlh).nlmsg_seq = self.seq;

            (*ndm).ndm_family = libc::AF_UNSPEC as __u8;
            (*ndm).ndm_state = IFA_F_PERMANENT as __u16;

            // process lladdr
            if neigh.lladdr != "" {
                let llabuf = parse_mac(&neigh.lladdr)?;

                addattr_var(nlh, NDA_LLADDR, llabuf.as_ptr() as *const u8, llabuf.len());
            }

            // process destination
            let (family, ip_data) = parse_addr(&to_ip)?;
            (*ndm).ndm_family = family;
            addattr_var(nlh, NDA_DST, ip_data.as_ptr() as *const u8, ip_data.len());

            // process state
            if neigh.state != 0 {
                (*ndm).ndm_state = neigh.state as __u16;
            }

            // process flags
            (*ndm).ndm_flags = (*ndm).ndm_flags | neigh.flags as __u8;

            // process dev
            (*ndm).ndm_ifindex = dev.ifi_index;

            // send
            self.rtnl_talk(v.as_mut_slice(), false)?;
        }

        Ok(())
    }
}

fn parse_mac(hwaddr: &str) -> Result<Vec<u8>> {
    let mut hw: Vec<u8> = vec![0; 6];

    let (hw0, hw1, hw2, hw3, hw4, hw5) = scan_fmt!(hwaddr, "{x}:{x}:{x}:{x}:{x}:{x}",
        [hex u8], [hex u8], [hex u8], [hex u8], [hex u8],
        [hex u8])?;

    hw[0] = hw0;
    hw[1] = hw1;
    hw[2] = hw2;
    hw[3] = hw3;
    hw[4] = hw4;
    hw[5] = hw5;

    Ok(hw)
}

fn parse_addr(ip_address: &str) -> Result<(__u8, Vec<u8>)> {
    let ip_data = parse_ipaddr(ip_address)?;
    let family: __u8;

    // ipv6
    if ip_data.len() == 16 {
        family = libc::AF_INET6 as __u8;
    } else {
        family = libc::AF_INET as __u8;
    }

    Ok((family, ip_data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RtnlHandle, NETLINK_ROUTE};
    use protocols::types::IPAddress;
    use std::process::Command;

    fn clean_env_for_test_add_one_arp_neighbor(dummy_name: &str, ip: &str) {
        // ip link delete dummy
        Command::new("ip")
            .args(&["link", "delete", dummy_name])
            .output()
            .expect("prepare: failed to delete dummy");

        // ip neigh del dev dummy ip
        Command::new("ip")
            .args(&["neigh", "del", dummy_name, ip])
            .output()
            .expect("prepare: failed to delete neigh");
    }

    fn prepare_env_for_test_add_one_arp_neighbor(dummy_name: &str, ip: &str) {
        clean_env_for_test_add_one_arp_neighbor(dummy_name, ip);
        // modprobe dummy
        Command::new("modprobe")
            .arg("dummy")
            .output()
            .expect("failed to run modprobe dummy");

        // ip link add dummy type dummy
        Command::new("ip")
            .args(&["link", "add", dummy_name, "type", "dummy"])
            .output()
            .expect("failed to add dummy interface");

        // ip addr add 192.168.0.2/16 dev dummy
        Command::new("ip")
            .args(&["addr", "add", "192.168.0.2/16", "dev", dummy_name])
            .output()
            .expect("failed to add ip for dummy");

        // ip link set dummy up;
        Command::new("ip")
            .args(&["link", "set", dummy_name, "up"])
            .output()
            .expect("failed to up dummy");
    }

    #[test]
    fn test_add_one_arp_neighbor() {
        // skip_if_not_root
        if !nix::unistd::Uid::effective().is_root() {
            println!("INFO: skipping {} which needs root", module_path!());
            return;
        }

        let mac = "6a:92:3a:59:70:aa";
        let to_ip = "169.254.1.1";
        let dummy_name = "dummy_for_arp";

        prepare_env_for_test_add_one_arp_neighbor(dummy_name, to_ip);

        let mut ip_address = IPAddress::new();
        ip_address.set_address(to_ip.to_string());

        let mut neigh = ARPNeighbor::new();
        neigh.set_toIPAddress(ip_address);
        neigh.set_device(dummy_name.to_string());
        neigh.set_lladdr(mac.to_string());
        neigh.set_state(0x80);

        let mut rtnl = RtnlHandle::new(NETLINK_ROUTE, 0).unwrap();

        rtnl.add_one_arp_neighbor(&neigh).unwrap();

        // ip neigh show dev dummy ip
        let stdout = Command::new("ip")
            .args(&["neigh", "show", "dev", dummy_name, to_ip])
            .output()
            .expect("failed to show neigh")
            .stdout;

        let stdout = std::str::from_utf8(&stdout).expect("failed to conveert stdout");

        assert_eq!(stdout, format!("{} lladdr {} PERMANENT\n", to_ip, mac));

        clean_env_for_test_add_one_arp_neighbor(dummy_name, to_ip);
    }
}
