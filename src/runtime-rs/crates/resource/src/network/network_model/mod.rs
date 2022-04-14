// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

pub mod none_model;
pub mod route_model;
pub mod tc_filter_model;

use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;

use super::NetworkPair;

const TC_FILTER_NET_MODEL_STR: &str = "tcfilter";
const ROUTE_NET_MODEL_STR: &str = "route";

pub enum NetworkModelType {
    NoneModel,
    TcFilter,
    Route,
}

#[async_trait]
pub trait NetworkModel: std::fmt::Debug + Send + Sync {
    fn model_type(&self) -> NetworkModelType;
    async fn add(&self, net_pair: &NetworkPair) -> Result<()>;
    async fn del(&self, net_pair: &NetworkPair) -> Result<()>;
}

pub fn new(model: &str) -> Result<Arc<dyn NetworkModel>> {
    match model {
        TC_FILTER_NET_MODEL_STR => Ok(Arc::new(
            tc_filter_model::TcFilterModel::new().context("new tc filter model")?,
        )),
        ROUTE_NET_MODEL_STR => Ok(Arc::new(
            route_model::RouteModel::new().context("new route model")?,
        )),
        _ => Ok(Arc::new(
            none_model::NoneModel::new().context("new none model")?,
        )),
    }
}
