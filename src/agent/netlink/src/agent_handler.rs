// Copyright (c) 2020 Ant Financial
// Copyright (C) 2020 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0
//

//! Dedicated Netlink interfaces for Kata agent protocol handler.

use std::convert::TryFrom;

use protobuf::RepeatedField;
use protocols::types::{ARPNeighbor, IPAddress, IPFamily, Interface, Route};

use super::*;

#[cfg(feature = "with-log")]
// Convenience macro to obtain the scope logger
macro_rules! sl {
    () => {
        slog_scope::logger().new(o!("subsystem" => "netlink"))
    };
}

impl super::RtnlHandle {
    pub fn update_interface(&mut self, iface: &Interface) -> Result<Interface> {
        // the reliable way to find link is using hardware address
        // as filter. However, hardware filter might not be supported
        // by netlink, we may have to dump link list and the find the
        // target link. filter using name or family is supported, but
        // we cannot use that to find target link.
        // let's try if hardware address filter works. -_-

        let ifinfo = self.find_link_by_hwaddr(iface.hwAddr.as_str())?;

        // bring down interface if it is up
        if ifinfo.ifi_flags & libc::IFF_UP as u32 != 0 {
            self.set_link_status(&ifinfo, false)?;
        }

        // delete all addresses associated with the link
        let del_addrs: Vec<RtIPAddr> = self.get_link_addresses(&ifinfo)?;
        self.delete_all_addrs(&ifinfo, del_addrs.as_ref())?;

        // add new ip addresses in request
        for grpc_addr in &iface.IPAddresses {
            let rtip = RtIPAddr::try_from(grpc_addr.clone())?;
            self.add_one_address(&ifinfo, &rtip)?;
        }

        let mut v: Vec<u8> = vec![0; DEFAULT_NETLINK_BUF_SIZE];
        // Safe because we have allocated enough buffer space.
        let nlh = unsafe { &mut *(v.as_mut_ptr() as *mut nlmsghdr) };
        let ifi = unsafe { &mut *(NLMSG_DATA!(nlh) as *mut ifinfomsg) };

        // set name, set mtu, IFF_NOARP. in one rtnl_talk.
        nlh.nlmsg_len = NLMSG_LENGTH!(mem::size_of::<ifinfomsg>() as u32) as __u32;
        nlh.nlmsg_type = RTM_NEWLINK;
        nlh.nlmsg_flags = NLM_F_REQUEST;
        self.assign_seqnum(nlh);

        ifi.ifi_family = ifinfo.ifi_family;
        ifi.ifi_type = ifinfo.ifi_type;
        ifi.ifi_index = ifinfo.ifi_index;
        if iface.raw_flags & libc::IFF_NOARP as u32 != 0 {
            ifi.ifi_change |= libc::IFF_NOARP as u32;
            ifi.ifi_flags |= libc::IFF_NOARP as u32;
        }

        // Safe because we have allocated enough buffer space.
        unsafe {
            nlh.addattr32(IFLA_MTU, iface.mtu as u32);

            // if str is null terminated, use addattr_var.
            // otherwise, use addattr_str
            nlh.addattr_var(IFLA_IFNAME, iface.name.as_ref());
        }

        self.rtnl_talk(v.as_mut_slice(), false)?;

        // TODO: why the result is ignored here?
        let _ = self.set_link_status(&ifinfo, true);

        Ok(iface.clone())
    }

    /// Delete this interface/link per request
    pub fn remove_interface(&mut self, iface: &Interface) -> Result<Interface> {
        let ifinfo = self.find_link_by_hwaddr(iface.hwAddr.as_str())?;

        self.set_link_status(&ifinfo, false)?;

        let mut v: Vec<u8> = vec![0; DEFAULT_NETLINK_BUF_SIZE];
        // Safe because we have allocated enough buffer space.
        let nlh = unsafe { &mut *(v.as_mut_ptr() as *mut nlmsghdr) };
        let ifi = unsafe { &mut *(NLMSG_DATA!(nlh) as *mut ifinfomsg) };

        // No attributes needed?
        nlh.nlmsg_len = NLMSG_LENGTH!(mem::size_of::<ifinfomsg>()) as __u32;
        nlh.nlmsg_type = RTM_DELLINK;
        nlh.nlmsg_flags = NLM_F_REQUEST;
        self.assign_seqnum(nlh);

        ifi.ifi_family = ifinfo.ifi_family;
        ifi.ifi_index = ifinfo.ifi_index;
        ifi.ifi_type = ifinfo.ifi_type;

        self.rtnl_talk(v.as_mut_slice(), false)?;

        Ok(iface.clone())
    }

    pub fn list_interfaces(&mut self) -> Result<Vec<Interface>> {
        let mut ifaces: Vec<Interface> = Vec::new();
        let (_slv, lv) = self.dump_all_links()?;
        let (_sav, av) = self.dump_all_addresses(0)?;

        for link in &lv {
            // Safe because dump_all_links() returns valid pointers.
            let nlh = unsafe { &**link };
            if nlh.nlmsg_type != RTM_NEWLINK && nlh.nlmsg_type != RTM_DELLINK {
                continue;
            }

            if nlh.nlmsg_len < NLMSG_SPACE!(mem::size_of::<ifinfomsg>()) {
                info!(
                    sl!(),
                    "invalid nlmsg! nlmsg_len: {}, nlmsg_space: {}",
                    nlh.nlmsg_len,
                    NLMSG_SPACE!(mem::size_of::<ifinfomsg>())
                );
                break;
            }

            // Safe because we have just validated available buffer space above.
            let ifi = unsafe { &*(NLMSG_DATA!(nlh) as *const ifinfomsg) };
            let rta: *mut rtattr = IFLA_RTA!(ifi as *const ifinfomsg) as *mut rtattr;
            let rtalen = IFLA_PAYLOAD!(nlh) as u32;
            let attrs = unsafe { parse_attrs(rta, rtalen, (IFLA_MAX + 1) as usize)? };

            // fill out some fields of Interface,
            let mut iface: Interface = Interface::default();

            // Safe because parse_attrs() returns valid pointers.
            unsafe {
                if !attrs[IFLA_IFNAME as usize].is_null() {
                    let t = attrs[IFLA_IFNAME as usize];
                    iface.name = String::from_utf8(getattr_var(t as *const rtattr))?;
                }

                if !attrs[IFLA_MTU as usize].is_null() {
                    let t = attrs[IFLA_MTU as usize];
                    iface.mtu = getattr32(t) as u64;
                }

                if !attrs[IFLA_ADDRESS as usize].is_null() {
                    let alen = RTA_PAYLOAD!(attrs[IFLA_ADDRESS as usize]);
                    let a: *const u8 = RTA_DATA!(attrs[IFLA_ADDRESS as usize]) as *const u8;
                    iface.hwAddr = parser::format_address(a, alen as u32)?;
                }
            }

            // get ip address info from av
            let mut ads: Vec<IPAddress> = Vec::new();
            for address in &av {
                // Safe because dump_all_addresses() returns valid pointers.
                let alh = unsafe { &**address };
                if alh.nlmsg_type != RTM_NEWADDR {
                    continue;
                }

                let tlen = NLMSG_SPACE!(mem::size_of::<ifaddrmsg>());
                if alh.nlmsg_len < tlen {
                    info!(
                        sl!(),
                        "invalid nlmsg! nlmsg_len: {}, nlmsg_space: {}", alh.nlmsg_len, tlen
                    );
                    break;
                }

                // Safe becahse we have checked avialable buffer space by NLMSG_SPACE above.
                let ifa = unsafe { &*(NLMSG_DATA!(alh) as *const ifaddrmsg) };
                let arta: *mut rtattr = IFA_RTA!(ifa) as *mut rtattr;
                let artalen = IFA_PAYLOAD!(alh) as u32;

                if ifa.ifa_index as u32 == ifi.ifi_index as u32 {
                    // found target addresses, parse attributes and fill out Interface
                    let addrs = unsafe { parse_attrs(arta, artalen, (IFA_MAX + 1) as usize)? };

                    // fill address field of Interface
                    let mut one: IPAddress = IPAddress::default();
                    let tattr: *const rtattr = if !addrs[IFA_ADDRESS as usize].is_null() {
                        addrs[IFA_ADDRESS as usize]
                    } else {
                        addrs[IFA_LOCAL as usize]
                    };

                    one.mask = format!("{}", ifa.ifa_prefixlen);
                    one.family = IPFamily::v4;
                    if ifa.ifa_family == libc::AF_INET6 as u8 {
                        one.family = IPFamily::v6;
                    }

                    // Safe because parse_attrs() returns valid pointers.
                    unsafe {
                        let a: *const u8 = RTA_DATA!(tattr) as *const u8;
                        let alen = RTA_PAYLOAD!(tattr);
                        one.address = parser::format_address(a, alen as u32)?;
                    }

                    ads.push(one);
                }
            }

            iface.IPAddresses = RepeatedField::from_vec(ads);
            ifaces.push(iface);
        }

        Ok(ifaces)
    }

    pub fn update_routes(&mut self, rt: &[Route]) -> Result<Vec<Route>> {
        let rs = self.get_all_routes()?;
        self.delete_all_routes(&rs)?;

        for grpcroute in rt {
            if grpcroute.gateway.as_str() == "" {
                let r = RtRoute::try_from(grpcroute.clone())?;
                if r.index == -1 {
                    continue;
                }
                self.add_one_route(&r)?;
            }
        }

        for grpcroute in rt {
            if grpcroute.gateway.as_str() != "" {
                let r = RtRoute::try_from(grpcroute.clone())?;
                if r.index == -1 {
                    continue;
                }
                self.add_one_route(&r)?;
            }
        }

        Ok(rt.to_owned())
    }

    pub fn list_routes(&mut self) -> Result<Vec<Route>> {
        // currently, only dump routes from main table for ipv4
        // ie, rtmsg.rtmsg_family = AF_INET, set RT_TABLE_MAIN
        // attribute in dump request
        // Fix Me: think about othe tables, ipv6..
        let mut rs: Vec<Route> = Vec::new();
        let (_srv, rv) = self.dump_all_routes()?;

        // parse out routes and store in rs
        for r in &rv {
            // Safe because dump_all_routes() returns valid pointers.
            let nlh = unsafe { &**r };
            if nlh.nlmsg_type != RTM_NEWROUTE && nlh.nlmsg_type != RTM_DELROUTE {
                info!(sl!(), "not route message!");
                continue;
            }
            let tlen = NLMSG_SPACE!(mem::size_of::<rtmsg>());
            if nlh.nlmsg_len < tlen {
                info!(
                    sl!(),
                    "invalid nlmsg! nlmsg_len: {}, nlmsg_spae: {}", nlh.nlmsg_len, tlen
                );
                break;
            }

            // Safe because we have just validated available buffer space above.
            let rtm = unsafe { &mut *(NLMSG_DATA!(nlh) as *mut rtmsg) };
            if rtm.rtm_table != RT_TABLE_MAIN as u8 {
                continue;
            }
            let rta: *mut rtattr = RTM_RTA!(rtm) as *mut rtattr;
            let rtalen = RTM_PAYLOAD!(nlh) as u32;
            let attrs = unsafe { parse_attrs(rta, rtalen, (RTA_MAX + 1) as usize)? };

            let t = attrs[RTA_TABLE as usize];
            if !t.is_null() {
                // Safe because parse_attrs() returns valid pointers
                let table = unsafe { getattr32(t) };
                if table != RT_TABLE_MAIN {
                    continue;
                }
            }

            // find source, destination, gateway, scope, and and device name
            let mut t = attrs[RTA_DST as usize];
            let mut rte: Route = Route::default();

            // Safe because parse_attrs() returns valid pointers
            unsafe {
                // destination
                if !t.is_null() {
                    let data: *const u8 = RTA_DATA!(t) as *const u8;
                    let len = RTA_PAYLOAD!(t) as u32;
                    rte.dest =
                        format!("{}/{}", parser::format_address(data, len)?, rtm.rtm_dst_len);
                }

                // gateway
                t = attrs[RTA_GATEWAY as usize];
                if !t.is_null() {
                    let data: *const u8 = RTA_DATA!(t) as *const u8;
                    let len = RTA_PAYLOAD!(t) as u32;
                    rte.gateway = parser::format_address(data, len)?;

                    // for gateway, destination is 0.0.0.0
                    rte.dest = "0.0.0.0".to_string();
                }

                // source
                t = attrs[RTA_SRC as usize];
                if t.is_null() {
                    t = attrs[RTA_PREFSRC as usize];
                }
                if !t.is_null() {
                    let data: *const u8 = RTA_DATA!(t) as *const u8;
                    let len = RTA_PAYLOAD!(t) as u32;
                    rte.source = parser::format_address(data, len)?;

                    if rtm.rtm_src_len != 0 {
                        rte.source = format!("{}/{}", rte.source.as_str(), rtm.rtm_src_len);
                    }
                }

                // scope
                rte.scope = rtm.rtm_scope as u32;

                // oif
                t = attrs[RTA_OIF as usize];
                if !t.is_null() {
                    let data = &*(RTA_DATA!(t) as *const i32);
                    assert_eq!(RTA_PAYLOAD!(t), 4);

                    rte.device = self
                        .get_name_by_index(*data)
                        .unwrap_or_else(|_| "unknown".to_string());
                }
            }

            rs.push(rte);
        }

        Ok(rs)
    }

    pub fn add_arp_neighbors(&mut self, neighs: &[ARPNeighbor]) -> Result<()> {
        for neigh in neighs {
            self.add_one_arp_neighbor(&neigh)?;
        }

        Ok(())
    }

    pub fn add_one_arp_neighbor(&mut self, neigh: &ARPNeighbor) -> Result<()> {
        let to_ip = match neigh.toIPAddress.as_ref() {
            None => return nix_errno(Errno::EINVAL),
            Some(v) => {
                if v.address.is_empty() {
                    return nix_errno(Errno::EINVAL);
                }
                v.address.as_ref()
            }
        };

        let dev = self.find_link_by_name(&neigh.device)?;

        let mut v: Vec<u8> = vec![0; DEFAULT_NETLINK_BUF_SIZE];
        // Safe because we have allocated enough buffer space.
        let nlh = unsafe { &mut *(v.as_mut_ptr() as *mut nlmsghdr) };
        let ndm = unsafe { &mut *(NLMSG_DATA!(nlh) as *mut ndmsg) };

        nlh.nlmsg_len = NLMSG_LENGTH!(std::mem::size_of::<ndmsg>()) as u32;
        nlh.nlmsg_type = RTM_NEWNEIGH;
        nlh.nlmsg_flags = NLM_F_REQUEST | NLM_F_CREATE | NLM_F_EXCL;
        self.assign_seqnum(nlh);

        ndm.ndm_family = libc::AF_UNSPEC as __u8;
        ndm.ndm_state = IFA_F_PERMANENT as __u16;
        // process lladdr
        if neigh.lladdr != "" {
            let llabuf = parser::parse_mac_addr(&neigh.lladdr)?;

            // Safe because we have allocated enough buffer space.
            unsafe { nlh.addattr_var(NDA_LLADDR, llabuf.as_ref()) };
        }

        let (family, ip_data) = parser::parse_ip_addr_with_family(&to_ip)?;
        ndm.ndm_family = family;
        // Safe because we have allocated enough buffer space.
        unsafe { nlh.addattr_var(NDA_DST, ip_data.as_ref()) };

        // process state
        if neigh.state != 0 {
            ndm.ndm_state = neigh.state as __u16;
        }

        // process flags
        ndm.ndm_flags = (*ndm).ndm_flags | neigh.flags as __u8;

        // process dev
        ndm.ndm_ifindex = dev.ifi_index;

        // send
        self.rtnl_talk(v.as_mut_slice(), false)?;

        Ok(())
    }
}

impl TryFrom<IPAddress> for RtIPAddr {
    type Error = nix::Error;

    fn try_from(ipi: IPAddress) -> std::result::Result<Self, Self::Error> {
        let ip_family = if ipi.family == IPFamily::v4 {
            libc::AF_INET
        } else {
            libc::AF_INET6
        } as __u8;

        let ip_mask = parser::parse_u8(ipi.mask.as_str(), 10)?;
        let addr = parser::parse_ip_addr(ipi.address.as_ref())?;

        Ok(Self {
            ip_family,
            ip_mask,
            addr,
        })
    }
}

impl TryFrom<Route> for RtRoute {
    type Error = nix::Error;

    fn try_from(r: Route) -> std::result::Result<Self, Self::Error> {
        // only handle ipv4

        let index = {
            let mut rh = RtnlHandle::new(NETLINK_ROUTE, 0)?;
            match rh.find_link_by_name(r.device.as_str()) {
                Ok(ifi) => ifi.ifi_index,
                Err(_) => -1,
            }
        };

        let (dest, dst_len) = if r.dest.is_empty() {
            (Some(vec![0 as u8; 4]), 0)
        } else {
            let (dst, mask) = parser::parse_cidr(r.dest.as_str())?;
            (Some(dst), mask)
        };

        let (source, src_len) = if r.source.is_empty() {
            (None, 0)
        } else {
            let (src, mask) = parser::parse_cidr(r.source.as_str())?;
            (Some(src), mask)
        };

        let gateway = if r.gateway.is_empty() {
            None
        } else {
            Some(parser::parse_ip_addr(r.gateway.as_str())?)
        };

        Ok(Self {
            dest,
            source,
            src_len,
            dst_len,
            index,
            gateway,
            scope: r.scope as u8,
            protocol: RTPROTO_UNSPEC,
        })
    }
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
