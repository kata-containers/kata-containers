/*
   Copyright The containerd Authors.

   Licensed under the Apache License, Version 2.0 (the "License");
   you may not use this file except in compliance with the License.
   You may obtain a copy of the License at

       http://www.apache.org/licenses/LICENSE-2.0

   Unless required by applicable law or agreed to in writing, software
   distributed under the License is distributed on an "AS IS" BASIS,
   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
   See the License for the specific language governing permissions and
   limitations under the License.
*/

//! `containerd-shim-protos` contains TTRPC bindings and client/server code to interact with
//! containerd's runtime v2 shims.
//!
//! # Runtime
//! This crate is mainly expected to be useful to interact with containerd's shim runtime.
//! Runtime v2 introduces a first class shim API for runtime authors to integrate with containerd.
//! The shim API is minimal and scoped to the execution lifecycle of a container.
//!
//! To learn how containerd's shim v2 runtime works in details, please refer to the [documentation](https://github.com/containerd/containerd/blob/main/runtime/v2/README.md).
//!
//! # Design
//! The `containerd-shim-protos` crate provides [Protobuf](https://github.com/protocolbuffers/protobuf.git) message
//! and [TTRPC](https://github.com/containerd/ttrpc.git) service definitions for the
//! [Containerd shim v2](https://github.com/containerd/containerd/blob/main/runtime/v2/task/shim.proto) protocol.
//!
//! The message and service definitions are auto-generated from protobuf source files under `vendor/`
//! by using [ttrpc-codegen](https://github.com/containerd/ttrpc-rust/tree/master/ttrpc-codegen). So please do not
//! edit those auto-generated source files.
//!
//! If upgrading/modification is needed, please follow the steps:
//! - Synchronize the latest protobuf source files from the upstream projects into directory 'vendor/'.
//! - Re-generate the source files by `cargo build --features=generate_bindings`.
//! - Commit the synchronized protobuf source files and auto-generated source files, keeping them in synchronization.
//!
//! # Examples
//!
//! Here is a quick example how to use the crate:
//! ```no_run
//! use containerd_shim_protos as client;
//!
//! use client::api;
//! use client::ttrpc::context::Context;
//!
//! // Create TTRPC client
//! let client = client::Client::connect("unix:///socket.sock").unwrap();
//!
//! // Get task client
//! let task_client = client::TaskClient::new(client);
//! let context = Context::default();
//!
//! // Send request and receive response
//! let request = api::ConnectRequest::default();
//! let response = task_client.connect(Context::default(), &request);
//! ```
//!

// Supress warning: redundant field names in struct initialization
#![allow(clippy::redundant_field_names)]

pub use protobuf;
pub use ttrpc;

/// Generated event structures.
#[rustfmt::skip]
pub mod events;
#[rustfmt::skip]
pub mod cgroups;
#[rustfmt::skip]
pub mod shim;
#[rustfmt::skip]
pub mod types;

/// Includes event names shims can publish to containerd.
pub mod topics;

pub mod shim_sync {
    /// TTRPC client reexport for easier access.
    pub use ttrpc::Client;

    /// Shim task service.
    pub use crate::shim::shim_ttrpc::{create_task, Task, TaskClient};

    /// Shim events service.
    pub use crate::shim::events_ttrpc::{create_events, Events, EventsClient};
}

pub use shim_sync::*;

#[cfg(feature = "async")]
pub mod shim_async {
    /// TTRPC client reexport for easier access.
    pub use ttrpc::asynchronous::Client;

    /// Shim task service.
    pub use crate::shim::shim_ttrpc_async::{create_task, Task, TaskClient};

    /// Shim events service.
    pub use crate::shim::events_ttrpc_async::{create_events, Events, EventsClient};
}

/// Reexport auto-generated public data structures.
pub mod api {
    pub use crate::shim::empty::*;
    pub use crate::shim::events::*;
    pub use crate::shim::mount::*;
    pub use crate::shim::shim::*;
    pub use crate::shim::task::*;
}
