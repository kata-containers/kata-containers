// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

//! The server side of shim management implementation, receive HTTP
//! requests and multiplex them to corresponding functions inside shim
//!
//! To call services in a RESTful convention, use the client
//! from libs/shim-interface library

mod handlers;
pub mod server;
