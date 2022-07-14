// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

#[macro_use]
extern crate slog;

logging::logger_with_subsystem!(sl, "virt-container");

mod container_manager;
pub mod health_check;
pub mod sandbox;

use std::sync::Arc;

use agent::kata::KataAgent;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use common::{message::Message, RuntimeHandler, RuntimeInstance};
use hypervisor::{dragonball::Dragonball, Hypervisor};
use kata_types::config::{hypervisor::register_hypervisor_plugin, DragonballConfig, TomlConfig};
use resource::ResourceManager;
use tokio::sync::mpsc::Sender;

const HYPERVISOR_DRAGONBALL: &str = "dragonball";

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
        sid: &str,
        msg_sender: Sender<Message>,
        config: &TomlConfig,
    ) -> Result<RuntimeInstance> {
        let hypervisor = new_hypervisor(config).await.context("new hypervisor")?;

        // get uds from hypervisor and get config from toml_config
        let agent = Arc::new(KataAgent::new(kata_types::config::Agent {
            debug: true,
            enable_tracing: false,
            server_port: 1024,
            log_port: 1025,
            dial_timeout_ms: 10,
            reconnect_timeout_ms: 3_000,
            request_timeout_ms: 30_000,
            health_check_request_timeout_ms: 90_000,
            kernel_modules: Default::default(),
            container_pipe_size: 0,
            debug_console_enabled: false,
        }));

        let resource_manager = Arc::new(ResourceManager::new(
            sid,
            agent.clone(),
            hypervisor.clone(),
            config,
        )?);
        let pid = std::process::id();

        let sandbox = sandbox::VirtSandbox::new(
            sid,
            msg_sender,
            agent.clone(),
            hypervisor,
            resource_manager.clone(),
        )
        .await
        .context("new virt sandbox")?;
        let container_manager =
            container_manager::VirtContainerManager::new(sid, pid, agent, resource_manager);
        Ok(RuntimeInstance {
            sandbox: Arc::new(sandbox),
            container_manager: Arc::new(container_manager),
        })
    }

    fn cleanup(&self, _id: &str) -> Result<()> {
        // TODO
        Ok(())
    }
}

async fn new_hypervisor(toml_config: &TomlConfig) -> Result<Arc<dyn Hypervisor>> {
    let hypervisor_name = &toml_config.runtime.hypervisor_name;
    let hypervisor_config = toml_config
        .hypervisor
        .get(hypervisor_name)
        .ok_or_else(|| anyhow!("failed to get hypervisor for {}", &hypervisor_name))
        .context("get hypervisor")?;

    // TODO: support other hypervisor
    // issue: https://github.com/kata-containers/kata-containers/issues/4634
    match hypervisor_name.as_str() {
        HYPERVISOR_DRAGONBALL => {
            let mut hypervisor = Dragonball::new();
            hypervisor
                .set_hypervisor_config(hypervisor_config.clone())
                .await;
            Ok(Arc::new(hypervisor))
        }
        _ => Err(anyhow!("Unsupported hypervisor {}", &hypervisor_name)),
    }
}
