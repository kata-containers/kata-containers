// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//
#![allow(bare_trait_objects)]
#![allow(clippy::redundant_field_names)]

pub mod agent;
pub mod agent_ttrpc;
#[cfg(feature = "async")]
pub mod agent_ttrpc_async;
pub mod csi;
pub mod empty;
mod gogo;
pub mod health;
pub mod health_ttrpc;
#[cfg(feature = "async")]
pub mod health_ttrpc_async;
pub mod oci;
#[cfg(feature = "with-serde")]
mod serde_config;
pub mod trans;
pub mod types;
pub mod remote;
pub mod remote_ttrpc;
#[cfg(feature = "async")]
pub mod remote_ttrpc_async;
pub mod api;

#[cfg(feature = "with-serde")]
pub use serde_config::{
    deserialize_enum_or_unknown, deserialize_message_field, serialize_enum_or_unknown,
    serialize_message_field,
};

pub mod confidential_data_hub;
pub mod confidential_data_hub_ttrpc;

#[cfg(feature = "async")]
pub mod confidential_data_hub_ttrpc_async;
