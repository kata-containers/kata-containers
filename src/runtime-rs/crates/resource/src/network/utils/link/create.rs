// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    fs::{File, OpenOptions},
    os::unix::io::AsRawFd,
    path::Path,
    {io, mem},
};

use anyhow::{Context, Result};
use nix::ioctl_write_ptr;

use super::macros::{get_name, set_name};

type IfName = [u8; libc::IFNAMSIZ];

#[derive(Copy, Clone, Debug)]
#[repr(C)]
struct CreateLinkMap {
    pub mem_start: libc::c_ulong,
    pub mem_end: libc::c_ulong,
    pub base_addr: libc::c_ushort,
    pub irq: libc::c_uchar,
    pub dma: libc::c_uchar,
    pub port: libc::c_uchar,
}

#[repr(C)]
union CreateLinkIfru {
    pub ifr_addr: libc::sockaddr,
    pub ifr_dst_addr: libc::sockaddr,
    pub ifr_broad_addr: libc::sockaddr,
    pub ifr_netmask: libc::sockaddr,
    pub ifr_hw_addr: libc::sockaddr,
    pub ifr_flags: libc::c_short,
    pub ifr_if_index: libc::c_int,
    pub ifr_metric: libc::c_int,
    pub ifr_mtu: libc::c_int,
    pub ifr_map: CreateLinkMap,
    pub ifr_slave: IfName,
    pub ifr_new_name: IfName,
    pub ifr_data: *mut libc::c_char,
}

#[repr(C)]
struct CreateLinkReq {
    pub ifr_name: IfName,
    pub ifr_ifru: CreateLinkIfru,
}

impl CreateLinkReq {
    pub fn from_name(name: &str) -> io::Result<CreateLinkReq> {
        let mut req: CreateLinkReq = unsafe { mem::zeroed() };
        req.set_name(name)?;
        Ok(req)
    }

    pub fn set_name(&mut self, name: &str) -> io::Result<()> {
        set_name!(self.ifr_name, name)
    }

    pub fn get_name(&self) -> io::Result<String> {
        get_name!(self.ifr_name)
    }

    pub unsafe fn set_raw_flags(&mut self, raw_flags: libc::c_short) {
        self.ifr_ifru.ifr_flags = raw_flags;
    }
}

const DEVICE_PATH: &str = "/dev/net/tun";

ioctl_write_ptr!(tun_set_iff, b'T', 202, libc::c_int);
ioctl_write_ptr!(tun_set_persist, b'T', 203, libc::c_int);

#[derive(Clone, Copy, Debug)]
pub enum LinkType {
    #[allow(dead_code)]
    Tun,
    Tap,
}

pub fn create_link(name: &str, link_type: LinkType, queues: usize) -> Result<()> {
    let mut flags = libc::IFF_VNET_HDR;
    flags |= match link_type {
        LinkType::Tun => libc::IFF_TUN,
        LinkType::Tap => libc::IFF_TAP,
    };

    let queues = if queues == 0 { 1 } else { queues };
    if queues > 1 {
        flags |= libc::IFF_MULTI_QUEUE | libc::IFF_NO_PI;
    } else {
        flags |= libc::IFF_ONE_QUEUE;
    };

    // create first queue
    let mut files = vec![];
    let (file, result_name) = create_queue(name, flags)?;
    unsafe {
        tun_set_persist(file.as_raw_fd(), &1).context("tun set persist")?;
    }
    files.push(file);

    // create other queues
    if queues > 1 {
        for _ in 0..queues - 1 {
            files.push(create_queue(&result_name, flags)?.0);
        }
    }

    info!(sl!(), "create link with fds {:?}", files);
    Ok(())
}

fn create_queue(name: &str, flags: libc::c_int) -> Result<(File, String)> {
    let path = Path::new(DEVICE_PATH);
    let file = OpenOptions::new().read(true).write(true).open(path)?;
    let mut req = CreateLinkReq::from_name(name)?;
    unsafe {
        req.set_raw_flags(flags as libc::c_short);
        tun_set_iff(file.as_raw_fd(), &mut req as *mut _ as *mut _).context("tun set iff")?;
    };
    Ok((file, req.get_name()?))
}

#[cfg(test)]
pub mod net_test_utils {
    use crate::network::network_model::tc_filter_model::fetch_index;

    // remove a link by its name
    #[allow(dead_code)]
    pub async fn delete_link(
        handle: &rtnetlink::Handle,
        name: &str,
    ) -> Result<(), rtnetlink::Error> {
        let link_index = fetch_index(handle, name)
            .await
            .expect("failed to fetch index");
        // the ifindex of a link will not change during its lifetime, so the index
        // remains the same between the query above and the deletion below
        handle.link().del(link_index).execute().await
    }
}

#[cfg(test)]
mod tests {
    use scopeguard::defer;
    use test_utils::skip_if_not_root;

    use crate::network::{
        network_pair::get_link_by_name, utils::link::create::net_test_utils::delete_link,
    };

    use super::*;

    #[actix_rt::test]
    async fn test_create_link() {
        let name_tun = "___test_tun";
        let name_tap = "___test_tap";

        // tests should be taken under root
        skip_if_not_root!();

        if let Ok((conn, handle, _)) =
            rtnetlink::new_connection().context("failed to create netlink connection")
        {
            let thread_handler = tokio::spawn(conn);
            defer!({
                thread_handler.abort();
            });

            assert!(create_link(name_tun, LinkType::Tun, 2).is_ok());
            assert!(create_link(name_tap, LinkType::Tap, 2).is_ok());
            assert!(get_link_by_name(&handle, name_tap).await.is_ok());
            assert!(get_link_by_name(&handle, name_tun).await.is_ok());
            assert!(delete_link(&handle, name_tun).await.is_ok());
            assert!(delete_link(&handle, name_tap).await.is_ok());

            // link does not present
            assert!(get_link_by_name(&handle, name_tun).await.is_err());
            assert!(get_link_by_name(&handle, name_tap).await.is_err());
        }
    }
}
