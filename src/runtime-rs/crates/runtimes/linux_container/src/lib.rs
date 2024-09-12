// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use common::{message::Message, types::SandboxConfig, RuntimeHandler, RuntimeInstance};
use kata_types::config::TomlConfig;
use resource::cpu_mem::initial_size::InitialSizeManager;
use tokio::sync::mpsc::Sender;

pub struct LinuxContainer {}

#[async_trait]
impl RuntimeHandler for LinuxContainer {
    fn init() -> Result<()> {
        Ok(())
    }

    fn name() -> String {
        "linux_container".to_string()
    }

    fn new_handler() -> Arc<dyn RuntimeHandler> {
        Arc::new(LinuxContainer {})
    }

    async fn new_instance(
        &self,
        _sid: &str,
        _msg_sender: Sender<Message>,
        _config: Arc<TomlConfig>,
        _init_size_manager: InitialSizeManager,
        _sandbox_config: SandboxConfig,
    ) -> Result<RuntimeInstance> {
        todo!()
    }

    fn cleanup(&self, _id: &str) -> Result<()> {
        todo!()
    }
}
