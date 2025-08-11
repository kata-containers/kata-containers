// Copyright (c) 2019,2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

#[cfg(not(test))]
mod container;
#[cfg(not(test))]
pub use container::ContainerCgroupManager;
pub mod device;
pub mod notifier;
#[cfg(not(test))]
mod sandbox;
#[cfg(not(test))]
pub use sandbox::SandboxCgroupManager;
#[cfg(test)]
mod container_mock;
#[cfg(test)]
pub use container_mock::ContainerCgroupManager;
#[cfg(test)]
pub mod sandbox_mock;
#[cfg(test)]
pub use sandbox_mock::SandboxCgroupManager;
pub mod utils;
