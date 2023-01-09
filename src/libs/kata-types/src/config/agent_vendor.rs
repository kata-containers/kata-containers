// Copyright (c) 2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use super::*;

/// Vendor customization agent configuration.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct AgentVendor {}

impl ConfigOps for AgentVendor {}
