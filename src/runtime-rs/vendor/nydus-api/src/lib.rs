// Copyright 2020 Ant Group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

//! APIs for the Nydus Image Service
//!
//! The `nydus-api` crate defines API and related data structures for Nydus Image Service.
//! All data structures used by the API are encoded in JSON format.

#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate nydus_error;

pub mod http;
pub(crate) mod http_endpoint_common;
pub(crate) mod http_endpoint_v1;
pub(crate) mod http_endpoint_v2;
