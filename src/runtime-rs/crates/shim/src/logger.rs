// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{Context, Result};

use crate::Error;

pub(crate) fn set_logger(path: &str, sid: &str, is_debug: bool) -> Result<slog_async::AsyncGuard> {
    // Since slog-async-logger is a synchronous thread actually, the log fifo can not be
    // asynchronous. Otherwise, writer might report EAGAIN(11) when it writes large amounts
    // of log to nonblock fifo, which will cause shim panic.
    let fifo = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
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

    Ok(async_guard)
}
