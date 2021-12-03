// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use common::{message::Message, RuntimeHandler, RuntimeInstance};
use kata_types::config::{hypervisor::register_hypervisor_plugin, DragonballConfig};
use tokio::sync::mpsc::Sender;

unsafe impl Send for VirtContainer {}
unsafe impl Sync for VirtContainer {}
pub struct VirtContainer {}

#[async_trait]
impl RuntimeHandler for VirtContainer {
    fn init() -> Result<()> {
        // register
        let dragonball_config = Arc::new(DragonballConfig::new());
        register_hypervisor_plugin("dragonball", dragonball_config);
        Ok(())
    }

    fn name() -> String {
        "virt_container".to_string()
    }

    fn new_handler() -> Arc<dyn RuntimeHandler> {
        Arc::new(VirtContainer {})
    }

    async fn new_instance(
        &self,
        _sid: &str,
        _msg_sender: Sender<Message>,
    ) -> Result<RuntimeInstance> {
        todo!()
    }

    fn cleanup(&self, _id: &str) -> Result<()> {
        // TODO
        Ok(())
    }
}
