// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use common::{message::Message, RuntimeHandler, RuntimeInstance};
use kata_types::config::TomlConfig;
use tokio::sync::mpsc::Sender;
unsafe impl Send for WasmContainer {}
unsafe impl Sync for WasmContainer {}
pub struct WasmContainer {}

#[async_trait]
impl RuntimeHandler for WasmContainer {
    fn init() -> Result<()> {
        Ok(())
    }

    fn name() -> String {
        "wasm_container".to_string()
    }

    fn new_handler() -> Arc<dyn RuntimeHandler> {
        Arc::new(WasmContainer {})
    }

    async fn new_instance(
        &self,
        _sid: &str,
        _msg_sender: Sender<Message>,
        _config: Arc<TomlConfig>,
    ) -> Result<RuntimeInstance> {
        todo!()
    }

    fn cleanup(&self, _id: &str) -> Result<()> {
        todo!()
    }
}
