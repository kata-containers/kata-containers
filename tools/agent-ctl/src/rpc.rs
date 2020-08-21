// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Description: ttRPC logic entry point

use anyhow::Result;
use slog::{o, Logger};

use crate::client::client;
use crate::types::Config;

pub fn run(
    logger: &Logger,
    server_address: &str,
    bundle_dir: &str,
    interactive: bool,
    ignore_errors: bool,
    timeout_nano: i64,
    commands: Vec<&str>,
) -> Result<()> {
    let cfg = Config {
        server_address: server_address.to_string(),
        bundle_dir: bundle_dir.to_string(),
        timeout_nano: timeout_nano,
        interactive: interactive,
        ignore_errors: ignore_errors,
    };

    // Maintain the global logger for the duration of the ttRPC comms
    let _guard = slog_scope::set_global_logger(logger.new(o!("subsystem" => "rpc")));

    client(&cfg, commands)
}
