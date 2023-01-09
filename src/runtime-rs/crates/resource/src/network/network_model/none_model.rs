// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use async_trait::async_trait;

use super::{NetworkModel, NetworkModelType};
use crate::network::NetworkPair;

#[derive(Debug)]
pub(crate) struct NoneModel {}

impl NoneModel {
    pub fn new() -> Result<Self> {
        Ok(Self {})
    }
}

#[async_trait]
impl NetworkModel for NoneModel {
    fn model_type(&self) -> NetworkModelType {
        NetworkModelType::NoneModel
    }

    async fn add(&self, _pair: &NetworkPair) -> Result<()> {
        Ok(())
    }

    async fn del(&self, _pair: &NetworkPair) -> Result<()> {
        Ok(())
    }
}
