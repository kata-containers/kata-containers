// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    collections::HashSet,
    fs::{File, OpenOptions},
    os::fd::{AsRawFd, RawFd},
};

use anyhow::{anyhow, Context, Result};
use kata_types::config::KATA_PATH;
use nix::{
    fcntl,
    sched::{setns, CloneFlags},
};
use serde::{Deserialize, Serialize};
use serde_json;

use crate::device::Tap;

use crate::{DEFAULT_HYBRID_VSOCK_NAME, JAILER_ROOT};

pub fn get_child_threads(pid: u32) -> HashSet<u32> {
    let mut result = HashSet::new();
    let path_name = format!("/proc/{}/task", pid);
    let path = std::path::Path::new(path_name.as_str());
    if path.is_dir() {
        if let Ok(dir) = path.read_dir() {
            for entity in dir {
                if let Ok(entity) = entity.as_ref() {
                    let file_name = entity.file_name();
                    let file_name = file_name.to_str().unwrap_or_default();
                    if let Ok(tid) = file_name.parse::<u32>() {
                        result.insert(tid);
                    }
                }
            }
        }
    }
    result
}

// Return the path for a _hypothetical_ sandbox: the path does *not* exist
// yet, and for this reason safe-path cannot be used.
pub fn get_sandbox_path(sid: &str) -> String {
    [KATA_PATH, sid].join("/")
}

pub fn get_hvsock_path(sid: &str) -> String {
    let jailer_root_path = get_jailer_root(sid);

    [jailer_root_path, DEFAULT_HYBRID_VSOCK_NAME.to_owned()].join("/")
}

pub fn get_jailer_root(sid: &str) -> String {
    let sandbox_path = get_sandbox_path(sid);

    [&sandbox_path, JAILER_ROOT].join("/")
}

// Clear the O_CLOEXEC which is set by default by Rust standard library on
// file descriptors that it opens.  This function is mostly meant to be
// called on descriptors to be passed to a child (hypervisor) process as
// O_CLOEXEC would obviously prevent that.
pub fn clear_cloexec(rawfd: RawFd) -> Result<()> {
    let cur_flags = fcntl::fcntl(rawfd, fcntl::FcntlArg::F_GETFD)?;
    let mut new_flags = fcntl::FdFlag::from_bits(cur_flags).ok_or(anyhow!(
        "couldn't construct FdFlag from flags value {:?}",
        cur_flags
    ))?;
    new_flags.remove(fcntl::FdFlag::FD_CLOEXEC);
    if let Err(err) = fcntl::fcntl(rawfd, fcntl::FcntlArg::F_SETFD(new_flags)) {
        info!(sl!(), "couldn't clear O_CLOEXEC on fd: {:?}", err);
        return Err(err.into());
    }

    Ok(())
}

pub fn enter_netns(netns_path: &str) -> Result<()> {
    if !netns_path.is_empty() {
        let netns =
            File::open(netns_path).context(anyhow!("open netns path {:?} failed.", netns_path))?;
        setns(netns.as_raw_fd(), CloneFlags::CLONE_NEWNET).context("set netns failed")?;
    }

    Ok(())
}

pub fn open_named_tuntap(if_name: &str, queues: u32) -> Result<Vec<File>> {
    let (multi_vq, vq_pairs) = if queues > 1 {
        (true, queues as usize)
    } else {
        (false, 1_usize)
    };

    let tap: Tap = Tap::open_named(if_name, multi_vq).context("open named tuntap device failed")?;
    let taps: Vec<Tap> = tap.into_mq_taps(vq_pairs).context("into mq taps failed.")?;

    let mut tap_files: Vec<std::fs::File> = Vec::new();
    for tap in taps {
        tap_files.push(tap.tap_file);
    }

    Ok(tap_files)
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

// QGS_SOCKET_PATH: the Unix Domain Socket Path served by Intel TDX Quote Generation Service
const QGS_SOCKET_PATH: &str = "/var/run/tdx-qgs/qgs.socket";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SocketAddress {
    #[serde(rename = "type")]
    pub typ: String,

    #[serde(rename = "cid", skip_serializing_if = "String::is_empty")]
    pub cid: String,

    #[serde(rename = "port", skip_serializing_if = "String::is_empty")]
    pub port: String,

    #[serde(rename = "path", skip_serializing_if = "String::is_empty")]
    pub path: String,
}

impl SocketAddress {
    pub fn new(port: u32) -> Self {
        if port == 0 {
            Self {
                typ: "unix".to_string(),
                cid: "".to_string(),
                port: "".to_string(),
                path: QGS_SOCKET_PATH.to_string(),
            }
        } else {
            Self {
                typ: "vsock".to_string(),
                cid: format!("{}", 2),
                port: port.to_string(),
                path: "".to_string(),
            }
        }
    }
}

impl std::fmt::Display for SocketAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        serde_json::to_string(self)
            .map_err(|_| std::fmt::Error)
            .and_then(|s| write!(f, "{}", s))
    }
}

pub fn bytes_to_megs(bytes: u64) -> u32 {
    (bytes / (1 << 20)) as u32
}

pub fn megs_to_bytes(bytes: u32) -> u64 {
    bytes as u64 * (1 << 20)
}

#[cfg(test)]
mod tests {
    use super::create_fds;
    use super::SocketAddress;

    #[test]
    fn test_ctreate_fds() {
        let device = "/dev/null";
        let num_fds = 3_usize;
        let fds = create_fds(device, num_fds);
        assert!(fds.is_ok());
        assert_eq!(fds.unwrap().len(), num_fds);
    }

    #[test]
    fn test_vsocket_address_new() {
        let socket = SocketAddress::new(8866);
        assert_eq!(socket.typ, "vsock");
        assert_eq!(socket.cid, "2");
        assert_eq!(socket.port, "8866");
    }

    #[test]
    fn test_unix_address_new() {
        let socket = SocketAddress::new(0);
        assert_eq!(socket.typ, "unix");
        assert_eq!(socket.path, "/var/run/tdx-qgs/qgs.socket");
    }

    #[test]
    fn test_socket_address_display() {
        let socket = SocketAddress::new(6688);
        let expected_json = r#"{"type":"vsock","cid":"2","port":"6688"}"#;
        assert_eq!(format!("{}", socket), expected_json);
    }

    #[test]
    fn test_socket_address_serialize_deserialize() {
        let socket = SocketAddress::new(0);
        let serialized = serde_json::to_string(&socket).unwrap();
        let expected_json = r#"{"type":"unix","path":"/var/run/tdx-qgs/qgs.socket"}"#;
        assert_eq!(expected_json, serialized);
    }

    #[test]
    fn test_socket_address_kebab_case() {
        let socket = SocketAddress::new(6868);
        let serialized = serde_json::to_string(&socket).unwrap();
        assert!(serialized.contains(r#""type":"#));
        assert!(serialized.contains(r#""cid":"#));
        assert!(serialized.contains(r#""port":"#));
    }
}
