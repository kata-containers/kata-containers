// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

#[macro_use]
extern crate slog;

macro_rules! sl {
    () => {
            slog_scope::logger().new(slog::o!("subsystem" => "shim"))
    };
}

mod args;
pub use args::Args;
mod error;
pub use error::Error;
mod logger;
mod panic_hook;
mod shim;
pub use shim::ShimExecutor;
#[rustfmt::skip]
pub mod config;
mod shim_delete;
mod shim_run;
mod shim_start;
