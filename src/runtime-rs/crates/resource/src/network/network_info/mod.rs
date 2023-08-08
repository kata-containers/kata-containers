// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

pub(crate) mod network_info_from_dan;
pub(crate) mod network_info_from_link;

use agent::{ARPNeighbor, Interface, Route};
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait NetworkInfo: std::fmt::Debug + Send + Sync {
    async fn interface(&self) -> Result<Interface>;
    async fn routes(&self) -> Result<Vec<Route>>;
    async fn neighs(&self) -> Result<Vec<ARPNeighbor>>;
}
