// Copyright (c) 2019 Ant Financial
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! ttrpc-rust is a **non-core** subproject of containerd
//!
//! `ttrpc-rust` is the Rust version of [ttrpc](https://github.com/containerd/ttrpc). [ttrpc](https://github.com/containerd/ttrpc) is GRPC for low-memory environments.
//!
//! Example:
//!
//! Check [this](https://github.com/containerd/ttrpc-rust/tree/master/example)
//!
//! # Feature flags
//!
//! - `async`: Enables async server and client.
//! - `sync`: Enables traditional sync server and client (default enabled).
//! - `protobuf-codec`: Includes rust-protobuf (default enabled).

#![cfg_attr(docsrs, feature(doc_cfg))]

#[macro_use]
extern crate log;

#[macro_use]
pub mod error;
#[macro_use]
pub mod common;
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
mod compiled {
    include!(concat!(env!("OUT_DIR"), "/mod.rs"));
}
#[doc(inline)]
pub use compiled::ttrpc;

#[doc(inline)]
pub use crate::common::MessageHeader;
#[doc(inline)]
pub use crate::error::{get_status, Error, Result};
#[doc(inline)]
pub use crate::ttrpc::{Code, Request, Response, Status};

cfg_sync! {
    pub mod sync;
    pub use crate::sync::channel::{write_message};
    pub use crate::sync::utils::{response_to_channel, MethodHandler, TtrpcContext};
    pub use crate::sync::client;
    pub use crate::sync::client::Client;
    pub use crate::sync::server;
    pub use crate::sync::server::Server;
}

cfg_async! {
    pub mod asynchronous;
    #[doc(hidden)]
    pub use crate::asynchronous as r#async;
}
