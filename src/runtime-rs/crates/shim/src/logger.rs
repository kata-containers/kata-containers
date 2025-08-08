// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{Context, Result};

pub(crate) fn set_logger(_path: &str, sid: &str, is_debug: bool) -> Result<slog_async::AsyncGuard> {
    let level = if is_debug {
        slog::Level::Debug
    } else {
        slog::Level::Info
    };

    // Use journal logger to send logs to systemd journal with "kata" identifier
    let (logger, async_guard) = logging::create_logger_with_destination(
        "kata-runtime",
        sid,
        level,
        logging::LogDestination::Journal,
    );

    // not reset global logger when drop
    slog_scope::set_global_logger(logger).cancel_reset();

    let level = if is_debug {
        log::Level::Debug
    } else {
        log::Level::Info
    };
    slog_stdlog::init_with_level(level).context(format!("init with level {}", level))?;

    // Regist component loggers for later use, there loggers are set directly by configuration
    logging::register_component_logger("agent");
    logging::register_component_logger("runtimes");
    logging::register_component_logger("hypervisor");

    Ok(async_guard)
}
