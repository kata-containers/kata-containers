// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::sync::Arc;

use crate::{message::Message, types::SandboxConfig, ContainerManager, Sandbox};
use anyhow::Result;
use async_trait::async_trait;
use kata_types::config::TomlConfig;
use resource::cpu_mem::initial_size::InitialSizeManager;
use tokio::sync::mpsc::Sender;

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
        init_size_manager: InitialSizeManager,
        sandbox_config: SandboxConfig,
    ) -> Result<RuntimeInstance>;

    fn cleanup(&self, id: &str) -> Result<()>;
}
