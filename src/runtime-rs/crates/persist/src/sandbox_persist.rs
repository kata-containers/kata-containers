// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Persist
where
    Self: Sized,
{
    /// The type of the object representing the state of the component.
    type State;
    /// The type of the object holding the constructor arguments.
    type ConstructorArgs;

    /// Save a state of the component.
    async fn save(&self) -> Result<Self::State>;

    /// Restore a component from a specified state.
    async fn restore(constructor_args: Self::ConstructorArgs, state: Self::State) -> Result<Self>;
}
