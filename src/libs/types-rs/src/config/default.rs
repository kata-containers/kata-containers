// Copyright (c) 2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

//! Default configuration values.
#![allow(missing_docs)]

use lazy_static::lazy_static;

lazy_static! {
    /// Default configuration file paths.
    pub static ref DEFAULT_RUNTIME_CONFIGURATIONS: Vec::<&'static str> = vec![
        "/etc/kata-containers2/configuration.toml",
        "/usr/share/defaults/kata-containers2/configuration.toml",
        "/etc/kata-containers/configuration_v2.toml",
        "/usr/share/defaults/kata-containers/configuration_v2.toml",
        "/etc/kata-containers/configuration.toml",
        "/usr/share/defaults/kata-containers/configuration.toml",
    ];
}

pub const DEFAULT_INTERNETWORKING_MODEL: &str = "tcfilter";
