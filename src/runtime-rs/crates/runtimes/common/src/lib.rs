// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

mod container_manager;
pub use container_manager::{should_stop_vm_on_missing_task, ContainerManager};
pub mod error;
pub mod message;
mod runtime_handler;
pub use runtime_handler::{RuntimeHandler, RuntimeInstance};
mod sandbox;
pub use sandbox::{Sandbox, SandboxNetworkEnv};
pub mod types;
