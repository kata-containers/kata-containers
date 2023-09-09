// Copyright 2021-2022 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

pub const DEFAULT_SLICE: &str = "system.slice";
pub const SLICE_SUFFIX: &str = ".slice";
pub const SCOPE_SUFFIX: &str = ".scope";
pub const WHO_ENUM_ALL: &str = "all";
pub const SIGNAL_KILL: i32 = nix::sys::signal::SIGKILL as i32;
pub const UNIT_MODE_REPLACE: &str = "replace";
pub const NO_SUCH_UNIT_ERROR: &str = "org.freedesktop.systemd1.NoSuchUnit";

pub type Properties<'a> = Vec<(&'a str, zbus::zvariant::Value<'a>)>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CgroupHierarchy {
    Legacy,
    Unified,
}
