// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::os::unix::fs::OpenOptionsExt;
use std::sync::Arc;

use crate::Error;
use anyhow::{Context, Result};
use logging::{
    AGENT_LOGGER, RESOURCE_LOGGER, RUNTIMES_LOGGER, SERVICE_LOGGER, SHIM_LOGGER,
    VIRT_CONTAINER_LOGGER, VMM_DRAGONBALL_LOGGER, VMM_LOGGER,
};

pub(crate) fn set_logger(path: &str, sid: &str, is_debug: bool) -> Result<slog_async::AsyncGuard> {
    let fifo = std::fs::OpenOptions::new()
        .custom_flags(libc::O_NONBLOCK)
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

    VMM_LOGGER.store(Arc::new(
        slog_scope::logger().new(slog::o!("subsystem" => "hypervisor")),
    ));
    VMM_DRAGONBALL_LOGGER.store(Arc::new(
        slog_scope::logger().new(slog::o!("subsystem" => "vmm-dragonball")),
    ));

    AGENT_LOGGER.store(Arc::new(
        slog_scope::logger().new(slog::o!("subsystem" => "agent")),
    ));
    RESOURCE_LOGGER.store(Arc::new(
        slog_scope::logger().new(slog::o!("subsystem" => "resource")),
    ));
    RUNTIMES_LOGGER.store(Arc::new(
        slog_scope::logger().new(slog::o!("subsystem" => "runtimes")),
    ));
    VIRT_CONTAINER_LOGGER.store(Arc::new(
        slog_scope::logger().new(slog::o!("subsystem" => "virt-container")),
    ));
    SERVICE_LOGGER.store(Arc::new(
        slog_scope::logger().new(slog::o!("subsystem" => "service")),
    ));
    SHIM_LOGGER.store(Arc::new(
        slog_scope::logger().new(slog::o!("subsystem" => "shim")),
    ));

    Ok(async_guard)
}
