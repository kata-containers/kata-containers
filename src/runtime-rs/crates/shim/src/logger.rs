// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::os::unix::fs::OpenOptionsExt;

use anyhow::{Context, Result};

use crate::Error;

pub(crate) fn set_logger(path: &str, sid: &str, is_debug: bool) -> Result<slog_async::AsyncGuard> {
    let fifo = std::fs::OpenOptions::new()
        .custom_flags(libc::O_NONBLOCK)
        .create(true)
        .append(true)
        .open(path)
        .context(Error::FileOpen(path.to_string()))?;

    let level = if is_debug {
        slog::Level::Debug
    } else {
        slog::Level::Info
    };

    let (logger, async_guard) = logging::create_logger("kata-runtime", sid, level, fifo);

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
