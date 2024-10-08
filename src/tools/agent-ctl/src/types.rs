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
    pub hybrid_vsock_port: u64,
    pub interactive: bool,
    pub hybrid_vsock: bool,
    pub ignore_errors: bool,
    pub no_auto_values: bool,
}

// CopyFile input struct
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CopyFileInput {
    pub src: String,
    pub dest: String,
}

// SetPolicy input request
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SetPolicyInput {
    pub policy_file: String,
}

// CreateContainer input
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CreateContainerInput {
    pub image: String,
    pub id: String,
}
