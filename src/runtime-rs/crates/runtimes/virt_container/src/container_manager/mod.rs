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

fn logger_with_process(container_process: &ContainerProcess) -> slog::Logger {
    sl!().new(o!("container_id" => container_process.container_id.container_id.clone(), "exec_id" => container_process.exec_id.clone()))
}
