// Copyright (c) 2021 Alibaba Cloud
// Copyright (c) 2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

pub mod config;
pub mod container;
pub mod sandbox;
pub mod spec_info;

pub use config::TomlConfig;
pub use container::Container;
pub use sandbox::Sandbox;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("not implemented")]
    NotImplemented,
}

pub type Result<T> = std::result::Result<T, Error>;
