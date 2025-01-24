// Copyright (c) 2022 Alibaba Cloud
// Copyright (c) 2024 Ant Group
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

    let _ = fs::create_dir_all(kata_run_path.join(short_id)).context(format!(
        "failed to create directory {:?}",
        kata_run_path.join(short_id)
    ));

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
pub fn sb_storage_path() -> Result<&'static str> {
    //make sure the path existed
    std::fs::create_dir_all(KATA_PATH).context(format!("failed to create dir: {}", KATA_PATH))?;

    Ok(KATA_PATH)
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

    get_uds_with_sid(sid, &sb_storage_path()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path;
    use tempfile::tempdir;

    use test_utils::skip_if_not_root;

    #[test]
    fn test_mgmt_socket_addr() {
        // this test has to run as root, so has to manually cleanup afterwards
        skip_if_not_root!();

        let sid = "katatest";
        let sandbox_test = path::Path::new(KATA_PATH).join("katatest98654sandboxpath1");
        fs::create_dir_all(sandbox_test.as_path()).unwrap();
        let addr = mgmt_socket_addr(sid).unwrap();
        assert_eq!(
            addr,
            "unix:///run/kata/katatest98654sandboxpath1/shim-monitor.sock"
        );
        fs::remove_dir_all(sandbox_test).unwrap();
    }

    #[test]
    fn test_mgmt_socket_addr_with_sid_empty() {
        let sid = "";
        let result = mgmt_socket_addr(sid);
        assert!(result.is_err());
        if let Err(err) = result {
            let left = format!("{:?}", err.to_string());
            let left_unquoted = &left[1..left.len() - 1];
            let left_unescaped = left_unquoted.replace("\\\"", "\"");

            assert_eq!(
                left_unescaped,
                format!("Empty sandbox id for acquiring socket address for shim_mgmt")
            )
        }
    }

    #[test]
    fn test_get_uds_with_sid_ok() {
        let run_path = tempdir().unwrap();
        let dir1 = run_path.path().join("kata98654sandboxpath1");
        let dir2 = run_path.path().join("aata98654dangboxpath1");
        fs::create_dir_all(dir1.as_path()).unwrap();
        fs::create_dir_all(dir2.as_path()).unwrap();

        let result = get_uds_with_sid("kata", &run_path.path().display().to_string());
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            format!(
                "unix://{}",
                run_path
                    .path()
                    .join("kata98654sandboxpath1")
                    .join(SHIM_MGMT_SOCK_NAME)
                    .display()
            )
        )
    }

    #[test]
    fn test_get_uds_with_sid_with_zero() {
        let result = get_uds_with_sid("acdsdfe", KATA_PATH);
        assert!(result.is_err());
        if let Err(err) = result {
            let left = format!("{:?}", err.to_string());
            let left_unquoted = &left[1..left.len() - 1];
            let left_unescaped = left_unquoted.replace("\\\"", "\"");

            assert_eq!(
                left_unescaped,
                format!(
                    "sandbox with the provided prefix {:?} is not found",
                    "acdsdfe"
                )
            )
        }
    }

    #[test]
    fn test_get_uds_with_sid_with_invalid() {
        let result = get_uds_with_sid("^abcdse", KATA_PATH);
        assert!(result.is_err());
        if let Err(err) = result {
            let left = format!("{:?}", err.to_string());
            let left_unquoted = &left[1..left.len() - 1];
            let left_unescaped = left_unquoted.replace("\\\"", "\"");
            assert_eq!(
                left_unescaped,
                "The short id contains invalid characters.".to_owned()
            );
        }
    }

    #[test]
    fn test_get_uds_with_sid_more_than_one() {
        let run_path = tempdir().unwrap();

        let dir1 = run_path.path().join("kata98654sandboxpath1");
        let dir2 = run_path.path().join("kata98654dangboxpath1");
        let dir3 = run_path.path().join("aata98654dangboxpath1");
        fs::create_dir_all(dir1.as_path()).unwrap();
        fs::create_dir_all(dir2.as_path()).unwrap();
        fs::create_dir_all(dir3.as_path()).unwrap();

        let result = get_uds_with_sid("kata", &run_path.path().display().to_string());
        assert!(result.is_err());
        if let Err(err) = result {
            let left = format!("{:?}", err.to_string());
            let left_unquoted = &left[1..left.len() - 1];
            let left_unescaped = left_unquoted.replace("\\\"", "\"");
            assert_eq!(left_unescaped, format!("more than one sandbox exists with the provided prefix {:?}, please provide a unique prefix", "kata"))
        }
    }
}
