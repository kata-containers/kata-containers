// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fs::OpenOptions;

use anyhow::{Context, Result};
use kata_types::config::{RuntimeLogDestination, TomlConfig};

pub(crate) fn set_logger(path: &str, sid: &str, is_debug: bool) -> Result<slog_async::AsyncGuard> {
    let level = if is_debug {
        slog::Level::Debug
    } else {
        slog::Level::Info
    };

    // Logger startup precedes runtime initialization; only deserialize here.
    let config_path = common::config::runtime_config_path(None)?;
    let (config, _) =
        TomlConfig::load_raw_from_file(config_path).context("load shim log destination")?;
    let destination = match config.runtime.log_destination {
        RuntimeLogDestination::Journald => logging::LogDestination::Journal,
        RuntimeLogDestination::File => {
            // containerd owns `log` as a FIFO; use a separate regular file.
            let file_path = std::path::Path::new(path).with_extension("json");
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&file_path)
                .with_context(|| format!("open shim log {}", file_path.display()))?;
            logging::LogDestination::File(Box::new(file))
        }
    };
    let (logger, async_guard) =
        logging::create_logger_with_destination("kata-runtime", sid, level, destination);

    // not reset global logger when drop
    slog_scope::set_global_logger(logger).cancel_reset();

    if is_debug {
        slog_stdlog::init_with_level(log::Level::Debug).context("init stdlog with debug")?;
    } else {
        slog_stdlog::init_with_level(log::Level::Info).context("init stdlog with info")?;
    }

    // Regist component loggers for later use, there loggers are set directly by configuration
    logging::register_component_logger("agent");
    logging::register_component_logger("runtimes");
    logging::register_component_logger("hypervisor");

    Ok(async_guard)
}
