// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

// Types are stubs during the migration; implementations land phase by phase.
#![allow(dead_code)]

pub(crate) mod platform;
pub(crate) mod probe;
pub(crate) mod topology;

mod pseries;
mod q35;
mod s390x;
mod virt;

#[cfg(test)]
mod tests;
