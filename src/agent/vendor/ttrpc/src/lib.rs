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

#[macro_use]
extern crate log;

#[macro_use]
pub mod error;
#[macro_use]
mod channel;
// TODO: address this after merging linters
#[allow(clippy::type_complexity, clippy::redundant_clone)]
pub mod client;
// TODO: address this after merging linters
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub mod server;
pub mod ttrpc;

pub use crate::channel::{
    write_message, MessageHeader, MESSAGE_TYPE_REQUEST, MESSAGE_TYPE_RESPONSE,
};
pub use crate::client::Client;
pub use crate::error::{get_status, Error, Result};
pub use crate::server::{response_to_channel, MethodHandler, Server, TtrpcContext};
pub use crate::ttrpc::{Code, Request, Response, Status};
