// Copyright (c) 2022 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::io::Result;

use crate::config::{ConfigOps, TomlConfig};

/// Image configuration information.
#[derive(Debug, Default, Deserialize, Serialize, Clone)]
pub struct Image {
    /// Container image service.
    ///
    /// If enabled, the CRI image management service will offloaded to agent.
    #[serde(default)]
    pub service_offload: bool,

    /// Container image decryption keys provisioning.
    /// Applies only if service_offload is true.
    ///
    /// Keys can be provisioned locally (e.g. through a special command or
    /// a local file) or remotely (usually after the guest is remotely attested).
    /// The provision setting is a complete URL that lets the Kata agent decide
    ///  which method to use in order to fetch the keys.
    ///
    /// # Notes:
    /// - Keys can be stored in a local file, in a measured and attested initrd:
    ///   provision=data:///local/key/file
    /// - Keys could be fetched through a special command or binary from the
    ///   initrd (guest) image, e.g. a firmware call:
    ///   provision=file:///path/to/bin/fetcher/in/guest
    /// - Keys can be remotely provisioned. The Kata agent fetches them from e.g.
    ///   a HTTPS URL:
    ///   provision=https://my-key-broker.foo/tenant/<tenant-id>
    #[serde(default)]
    pub provison: String,
}

impl ConfigOps for Image {
    fn adjust_config(_conf: &mut TomlConfig) -> Result<()> {
        Ok(())
    }

    fn validate(_conf: &TomlConfig) -> Result<()> {
        Ok(())
    }
}
