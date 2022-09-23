// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

#[macro_use]
extern crate slog;

logging::logger_with_subsystem!(sl, "runtimes");

pub mod manager;
pub use manager::RuntimeHandlerManager;
mod shim_mgmt;
pub use shim_mgmt::{client::MgmtClient, server::sb_storage_path};
mod static_resource;
pub mod tracer;
