// Copyright 2021-2022 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

pub const DEFAULT_SLICE: &str = "system.slice";
pub const SLICE_SUFFIX: &str = ".slice";
pub const SCOPE_SUFFIX: &str = ".scope";
pub const UNIT_MODE: &str = "replace";

pub type Properties<'a> = Vec<(&'a str, zbus::zvariant::Value<'a>)>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CgroupHierarchy {
    Legacy,
    Unified,
}
