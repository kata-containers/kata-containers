// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

//! Server and client in async mode (alias r#async).

pub mod client;
pub mod server;
pub mod stream;
#[macro_use]
pub mod utils;

#[doc(inline)]
pub use crate::r#async::client::Client;
#[doc(inline)]
pub use crate::r#async::server::Server;
#[doc(inline)]
pub use crate::r#async::utils::{convert_response_to_buf, MethodHandler, TtrpcContext};
