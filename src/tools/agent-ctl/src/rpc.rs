// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Description: ttRPC logic entry point

use anyhow::Result;
use slog::{o, Logger};

use crate::client::client;
use crate::types::Config;

pub fn run(logger: &Logger, cfg: &Config, commands: Vec<&str>) -> Result<()> {
    // Maintain the global logger for the duration of the ttRPC comms
    let _guard = slog_scope::set_global_logger(logger.new(o!("subsystem" => "rpc")));

    client(cfg, commands)
}
