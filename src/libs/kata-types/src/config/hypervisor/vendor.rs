// Copyright (c) 2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

//! A sample for vendor to customize the hypervisor implementation.

use super::*;

/// Vendor customization runtime configuration.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct HypervisorVendor {}

impl ConfigOps for HypervisorVendor {}
