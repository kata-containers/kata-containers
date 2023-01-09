// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use strum_macros::EnumString;

// Builtin check command handler type.
pub type BuiltinCmdFp = fn(args: &str) -> Result<()>;

// CheckType encodes the name of each check provided by kata-ctl.
#[derive(Debug, strum_macros::Display, EnumString, PartialEq)]
pub enum CheckType {
    CheckCpu,
    CheckNetwork,
}

// PermissionType is used to show whether a check needs to run with elevated (super-user)
// privileges, or whether it can run as normal user.
#[derive(strum_macros::Display, EnumString, PartialEq)]
pub enum PermissionType {
    Privileged,
    NonPrivileged,
}

// CheckItem is used to encode the check metadata that each architecture
// returns in a list of CheckItem's using the architecture implementation
// of get_checks().
pub struct CheckItem<'a> {
    pub name: CheckType,
    pub descr: &'a str,
    pub fp: BuiltinCmdFp,
    pub perm: PermissionType,
}
