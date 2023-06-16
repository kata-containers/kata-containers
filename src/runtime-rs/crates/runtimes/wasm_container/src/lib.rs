// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
#[macro_use]
extern crate slog;
#[macro_use]
extern crate lazy_static;

logging::logger_with_subsystem!(sl, "wasm-container");

mod container_manager;
mod sandbox;

use crate::container_manager::container::container::Container;
use crate::container_manager::manager::WasmContainerManager;
use crate::sandbox::sandbox::WasmSandbox;

use anyhow::Result;
use async_trait::async_trait;
use common::{message::Message, RuntimeHandler, RuntimeInstance};
use kata_types::config::TomlConfig;
use std::{collections::HashMap, fs::remove_dir_all, sync::Arc};
use tokio::sync::mpsc::Sender;
use tokio::sync::RwLock;

pub const BASE_PATH: &str = "/run/kata-containers/wasm-containers";

lazy_static! {
    pub static ref CONTAINER_BASE: String = [BASE_PATH, "containers"].join("/");
    pub static ref SANDBOX_BASE: String = [BASE_PATH, "sandboxes"].join("/");
}

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
        sid: &str,
        msg_sender: Sender<Message>,
        _config: Arc<TomlConfig>,
    ) -> Result<RuntimeInstance> {
        let pid = std::process::id();

        let _ = remove_dir_all(BASE_PATH);
        let _ = remove_dir_all(BASE_PATH);

        let containers: Arc<RwLock<HashMap<String, Container>>> = Default::default();
        let sandbox = Arc::new(WasmSandbox::new(sid, msg_sender, containers.clone())?);
        let container_manager =
            Arc::new(WasmContainerManager::new(sandbox.clone(), pid, containers).await);

        Ok(RuntimeInstance {
            sandbox,
            container_manager,
        })
    }

    fn cleanup(&self, _id: &str) -> Result<()> {
        Ok(())
    }
}
