// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

//! Server and Client in sync mode.

#[macro_use]
pub mod channel;
pub mod client;
// TODO: address this after merging linters
#[allow(clippy::too_many_arguments)]
pub mod server;

#[macro_use]
pub mod utils;
