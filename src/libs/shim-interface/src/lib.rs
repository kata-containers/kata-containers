// Copyright (c) 2022 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

//! shim-interface is a common library for different components of Kata Containers
//! to make function call through services inside the runtime(runtime-rs runtime).
//!
//! Shim management:
//! Currently, inside the shim, there is a shim management server running as the shim
//! starts, working as a RESTful server. To make function call in runtime from another
//! binary, using the utilities provided in this library is one of the methods.
//!
//! You may construct clients by construct a MgmtClient and let is make specific
//! HTTP request to the server. The server inside shim will multiplex the request
//! to its corresponding handler and run certain methods.

use std::path::Path;

use anyhow::{anyhow, Result};

pub mod shim_mgmt;

use kata_types::config::KATA_PATH;

// Used to be `shim-monitor.sock`, but due to a char limit of 100
// when running jailed we can safely reduce the size.
pub const SHIM_MGMT_SOCK_NAME: &str = "shim.sock";

// return sandbox's storage path
pub fn sb_storage_path() -> String {
    String::from(KATA_PATH)
}

// returns the address of the unix domain socket(UDS) for communication with shim
// management service using http
// normally returns `unix:///run/kata/{sid}/shim_monitor.sock` or
// `unix:///run/kata/firecracker/{sid}/root/shim_monitor.sock` when
// running jailed
pub fn mgmt_socket_addr(sid: &str, jailed_path: &str) -> Result<String> {
    if sid.is_empty() {
        return Err(anyhow!(
            "Empty sandbox id for acquiring socket address for shim_mgmt"
        ));
    }

    let p = match jailed_path {
        "" => Path::new(&sb_storage_path())
            .join(sid)
            .join(SHIM_MGMT_SOCK_NAME),
        _ => Path::new(jailed_path).join(sid).join(SHIM_MGMT_SOCK_NAME),
    };

    if let Some(p) = p.to_str() {
        Ok(format!("unix://{}", p))
    } else {
        Err(anyhow!("Bad socket path"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mgmt_socket_addr() {
        let sid = "414123";
        let addr = mgmt_socket_addr(sid, "").unwrap();
        assert_eq!(addr, "unix:///run/kata/414123/shim-monitor.sock");

        let sid = "";
        assert!(mgmt_socket_addr(sid).is_err());
    }
}
