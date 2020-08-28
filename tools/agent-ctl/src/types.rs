// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Type used to pass optional state between cooperating API calls.
pub type Options = HashMap<String, String>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server_address: String,
    pub bundle_dir: String,
    pub timeout_nano: i64,
    pub interactive: bool,
    pub ignore_errors: bool,
}
