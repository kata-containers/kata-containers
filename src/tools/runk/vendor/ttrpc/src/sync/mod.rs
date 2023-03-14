// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

//! Server and Client in sync mode.

mod channel;
mod client;
mod server;

#[macro_use]
mod utils;

pub use client::Client;
pub use server::Server;

#[doc(hidden)]
pub use utils::response_to_channel;
pub use utils::{MethodHandler, TtrpcContext};
