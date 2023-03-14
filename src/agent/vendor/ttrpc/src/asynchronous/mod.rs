// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

//! Server and client in async mode (alias r#async).

mod client;
mod server;
mod stream;
#[macro_use]
#[doc(hidden)]
mod utils;
mod unix_incoming;

#[doc(inline)]
pub use crate::r#async::client::Client;
#[doc(inline)]
pub use crate::r#async::server::Server;
#[doc(inline)]
pub use utils::{MethodHandler, TtrpcContext};
