// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use kata_types::config::TomlConfig;
use tokio::sync::mpsc::Sender;

use crate::{message::Message, ContainerManager, Sandbox};

#[derive(Clone)]
pub struct RuntimeInstance {
    pub sandbox: Arc<dyn Sandbox>,
    pub container_manager: Arc<dyn ContainerManager>,
}

#[async_trait]
pub trait RuntimeHandler: Send + Sync {
    fn init() -> Result<()>
    where
        Self: Sized;

    fn name() -> String
    where
        Self: Sized;

    fn new_handler() -> Arc<dyn RuntimeHandler>
    where
        Self: Sized;

    async fn new_instance(
        &self,
        sid: &str,
        msg_sender: Sender<Message>,
        config: Arc<TomlConfig>,
    ) -> Result<RuntimeInstance>;

    fn cleanup(&self, id: &str) -> Result<()>;
}

impl RuntimeInstance {
    // NOTE: if static resource management is configured, a warning is logged
    // hotplug vcpu/memory, and the cpu will not be updated since the sandbox
    // should be static
    // The updated resource is calculated from:
    //   - vcpu: the sum of each ctr, plus default vcpu
    //   - memory: the sum of each ctr, plus default memory, and setup swap
    pub async fn update_sandbox_resource(&self) -> Result<()> {
        // calculate the number of vcpu needed in total
        let nr_vcpus = self.container_manager.total_vcpus().await?;

        //todo: calculate memory (sandbox_mem and swap size)

        self.sandbox
            .update_cpu_resource(nr_vcpus)
            .await
            .context("failed to update_cpu_resource")?;

        // todo: update new memory and online

        Ok(())
    }
}
