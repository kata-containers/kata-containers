// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::os::unix::fs::OpenOptionsExt;
use std::sync::Arc;

use anyhow::{Context, Result};
use logging::{VMM_LOGGER, VMM_DRAGONBALL_LOGGER, AGENT_LOGGER, HYPERVISOR_LOGGER, RESOURCE_LOGGER, RUNTIMES_LOGGER, VIRT_CONTAINER_LOGGER, SERVICE_LOGGER, SHIM_LOGGER};
use crate::Error;

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

    // Set LOGGERS
    let vmm_fifo = std::fs::OpenOptions::new()
        .custom_flags(libc::O_NONBLOCK)
        .create(true)
        .write(true)
        .append(true)
        .open(path)
        .context(Error::FileOpen(path.to_string()))?;

    // let level = if is_debug {
    //     slog::Level::Debug
    // } else {
    //     slog::Level::Info
    // };

    let (vmm_logger, _async_guard) =
        logging::create_logger("kata-runtime", sid, slog::Level::Debug, vmm_fifo);

    let vmm_dragonball = vmm_logger.new(slog::o!("subsystem" => "hypervisor"));
    VMM_LOGGER.store(Arc::new(vmm_logger));
    VMM_DRAGONBALL_LOGGER.store(Arc::new(vmm_dragonball));

    AGENT_LOGGER.store(Arc::new(slog_scope::logger().new(slog::o!("subsystem" => "agent"))));
    HYPERVISOR_LOGGER.store(Arc::new(slog_scope::logger().new(slog::o!("subsystem" => "hypervisor"))));
    RESOURCE_LOGGER.store(Arc::new(slog_scope::logger().new(slog::o!("subsystem" => "resource"))));
    RUNTIMES_LOGGER.store(Arc::new(slog_scope::logger().new(slog::o!("subsystem" => "runtimes"))));
    VIRT_CONTAINER_LOGGER.store(Arc::new(slog_scope::logger().new(slog::o!("subsystem" => "virt-container"))));
    SERVICE_LOGGER.store(Arc::new(slog_scope::logger().new(slog::o!("subsystem" => "service"))));
    SHIM_LOGGER.store(Arc::new(slog_scope::logger().new(slog::o!("subsystem" => "shim"))));

    Ok(async_guard)
}
