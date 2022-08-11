// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::sync::Arc;

use super::{Endpoint, NetworkInfo};

#[derive(Debug)]
pub(crate) struct NetworkEntity {
    pub(crate) endpoint: Arc<dyn Endpoint>,
    pub(crate) network_info: Arc<dyn NetworkInfo>,
}

impl NetworkEntity {
    pub fn new(endpoint: Arc<dyn Endpoint>, network_info: Arc<dyn NetworkInfo>) -> Self {
        Self {
            endpoint,
            network_info,
        }
    }
}
