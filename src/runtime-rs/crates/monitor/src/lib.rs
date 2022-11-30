// Copyright 2021-2022 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

#[macro_use]
extern crate slog;

#[macro_use]
extern crate lazy_static;

logging::logger_with_subsystem!(sl, "monitor");

pub mod http_server;
pub mod metrics;
