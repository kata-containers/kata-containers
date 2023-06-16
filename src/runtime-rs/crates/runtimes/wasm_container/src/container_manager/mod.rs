// Copyright (c) 2022-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

pub mod container;
pub mod manager;

pub use manager::WasmContainerManager;

use common::types::ContainerProcess;

fn logger_with_process(container_process: &ContainerProcess) -> slog::Logger {
    sl!().new(o!("container_id" => container_process.container_id.container_id.clone(), "exec_id" => container_process.exec_id.clone()))
}
