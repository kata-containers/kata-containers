// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

mod container;
use container::{Container, Exec};
mod container_inner;
mod io;
use container_inner::ContainerInner;
mod manager;
pub use manager::VirtContainerManager;
mod process;

use common::types::ContainerProcess;
use logging::{
    AGENT_LOGGER, RESOURCE_LOGGER, RUNTIMES_LOGGER, SERVICE_LOGGER, SHIM_LOGGER,
    VIRT_CONTAINER_LOGGER, VMM_DRAGONBALL_LOGGER, VMM_LOGGER,
};
use slog::Logger;
fn logger_with_process(container_process: &ContainerProcess) -> slog::Logger {
    sl!().new(o!("container_id" => container_process.container_id.container_id.clone(), "exec_id" => container_process.exec_id.clone()))
}
