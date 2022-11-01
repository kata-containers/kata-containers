// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

#![allow(dead_code)]

use anyhow::{anyhow, Result};

const NON_PRIV_USER: &str = "nobody";

pub fn drop_privs() -> Result<()> {
    if nix::unistd::Uid::effective().is_root() {
        privdrop::PrivDrop::default()
            .chroot("/")
            .user(NON_PRIV_USER)
            .apply()
            .map_err(|e| anyhow!("Failed to drop privileges to user {}: {}", NON_PRIV_USER, e))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drop_privs() {
        let res = drop_privs();
        assert!(res.is_ok());
    }
}
