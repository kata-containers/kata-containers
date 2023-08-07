// Copyright (c) 2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use serde::{Serialize, Deserialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug)]
pub struct Arch {
    pub name: String,
    pub version: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Project {
    pub description: String,
    pub url: Option<String>,
    pub version: Option<String>,
    pub tag: Option<String>,
    pub branch: Option<String>,
    pub architecture: Option<HashMap<String, Arch>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CheckResult {
    pub project_name: String,
    pub current_version: String,
    pub latest_version: String,
    pub up_to_date: bool,
    pub success: bool,
    pub message: Option<String>
}
