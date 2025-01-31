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
    Cpu,
    Network,
    KernelModules,
    KvmIsUsable,
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

// Builtin module parameter check handler type.
//
// BuiltinModuleParamFp represents a predicate function to determine if a
// kernel parameter _value_ is as expected. If not, the returned Error will
// explain what is wrong.
//
// Parameters:
//
// - module: name of kernel module.
// - param: name of parameter for the kernel module.
// - value: value of the kernel parameter.
pub type BuiltinModuleParamFp = fn(module: &str, param: &str, value: &str) -> Result<()>;

// KernelParamType encodes the value and a handler
// function for kernel module parameters
#[allow(dead_code)]
#[derive(Clone)]
pub enum KernelParamType<'a> {
    Simple(&'a str),
    Predicate(BuiltinModuleParamFp),
}

// Parameters is used to encode the module parameters
#[allow(dead_code)]
#[derive(Clone)]
pub struct KernelParam<'a> {
    pub name: &'a str,
    pub value: KernelParamType<'a>,
}

// KernelModule is used to describe a kernel module along with its required parameters.
#[allow(dead_code)]
pub struct KernelModule<'a> {
    pub name: &'a str,
    pub params: &'a [KernelParam<'a>],
}
