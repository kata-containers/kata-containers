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

use std::fs;

use anyhow::{anyhow, Context, Result};

pub mod shim_mgmt;

use kata_sys_util::validate::verify_id;
use kata_types::config::KATA_PATH;

pub const SHIM_MGMT_SOCK_NAME: &str = "shim-monitor.sock";

fn get_uds_with_sid(short_id: &str, path: &str) -> Result<String> {
    verify_id(short_id).context("The short id contains invalid characters.")?;

    let kata_run_path = fs::canonicalize(path).context("failed to canonicalize path")?;

    let p = kata_run_path.join(short_id).join(SHIM_MGMT_SOCK_NAME);
    if p.exists() {
        return Ok(format!("unix://{}", p.display()));
    }

    let target_ids: Vec<String> = fs::read_dir(&kata_run_path)?
        .filter_map(|e| {
            let x = e.ok()?.file_name().to_string_lossy().into_owned();
            x.as_str().starts_with(short_id).then_some(x)
        })
        .collect::<Vec<_>>();

    match target_ids.len() {
        0 => Err(anyhow!(
            "sandbox with the provided prefix {short_id:?} is not found"
        )),
        1 => {
            // One element and only one exists.
            Ok(format!(
                "unix://{}",
                kata_run_path
                    .join(target_ids[0].as_str())
                    .join(SHIM_MGMT_SOCK_NAME)
                    .display()
            ))
        }
        _ => {
            // n > 1 return error
            Err(anyhow!(
                "more than one sandbox exists with the provided prefix {short_id:?}, please provide a unique prefix"
            ))
        }
    }
}

// return sandbox's storage path
pub fn sb_storage_path() -> String {
    String::from(KATA_PATH)
}

// returns the address of the unix domain socket(UDS) for communication with shim
// management service using http
// normally returns "unix:///run/kata/{sid}/shim_monitor.sock"
pub fn mgmt_socket_addr(sid: &str) -> Result<String> {
    if sid.is_empty() {
        return Err(anyhow!(
            "Empty sandbox id for acquiring socket address for shim_mgmt"
        ));
    }

    get_uds_with_sid(sid, &sb_storage_path())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mgmt_socket_addr() {
        let sid = "414123";
        let addr = mgmt_socket_addr(sid).unwrap();
        assert_eq!(addr, "unix:///run/kata/414123/shim-monitor.sock");

        let sid = "";
        assert!(mgmt_socket_addr(sid).is_err());
    }
}
