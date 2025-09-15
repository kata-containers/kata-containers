// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

#[macro_use]
extern crate slog;

logging::logger_with_subsystem!(sl, "virt-container");

mod container_manager;
pub mod factory;
pub mod health_check;
pub mod sandbox;
pub mod sandbox_persist;

use std::path::PathBuf;
use std::sync::Arc;

use agent::{kata::KataAgent, AGENT_KATA};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use common::{message::Message, types::SandboxConfig, RuntimeHandler, RuntimeInstance};
use hypervisor::Hypervisor;
#[cfg(feature = "dragonball")]
use hypervisor::{dragonball::Dragonball, HYPERVISOR_DRAGONBALL};
use hypervisor::{firecracker::Firecracker, HYPERVISOR_FIRECRACKER};
use hypervisor::{qemu::Qemu, HYPERVISOR_QEMU};
use hypervisor::{remote::Remote, HYPERVISOR_REMOTE};
#[cfg(feature = "dragonball")]
use kata_types::config::DragonballConfig;
use kata_types::config::FirecrackerConfig;
use kata_types::config::RemoteConfig;
use kata_types::config::{hypervisor::register_hypervisor_plugin, QemuConfig, TomlConfig};

#[cfg(all(
    feature = "cloud-hypervisor",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
use hypervisor::ch::CloudHypervisor;
#[cfg(all(
    feature = "cloud-hypervisor",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
use kata_types::config::{hypervisor::HYPERVISOR_NAME_CH, CloudHypervisorConfig};
#[allow(unused_imports)]
use std::thread;
#[allow(unused_imports)]
use std::time::Duration;

use crate::factory::vm::VMConfig;
use resource::cpu_mem::initial_size::InitialSizeManager;
use resource::ResourceManager;
use sandbox::VIRTCONTAINER;
use tokio::sync::mpsc::Sender;
use tracing::instrument;
unsafe impl Send for VirtContainer {}
unsafe impl Sync for VirtContainer {}
#[derive(Debug)]
pub struct VirtContainer {}

#[async_trait]
impl RuntimeHandler for VirtContainer {
    fn init() -> Result<()> {
        // Before start logging with virt-container, regist it
        logging::register_subsystem_logger("runtimes", "virt-container");

        // register
        #[cfg(feature = "dragonball")]
        let dragonball_config = Arc::new(DragonballConfig::new());
        #[cfg(feature = "dragonball")]
        register_hypervisor_plugin("dragonball", dragonball_config);

        let firecracker_config = Arc::new(FirecrackerConfig::new());
        register_hypervisor_plugin("firecracker", firecracker_config);

        let qemu_config = Arc::new(QemuConfig::new());
        register_hypervisor_plugin("qemu", qemu_config);

        #[cfg(all(
            feature = "cloud-hypervisor",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ))]
        {
            let ch_config = Arc::new(CloudHypervisorConfig::new());
            register_hypervisor_plugin(HYPERVISOR_NAME_CH, ch_config);
        }

        let remote_config = Arc::new(RemoteConfig::new());
        register_hypervisor_plugin("remote", remote_config);

        Ok(())
    }

    fn name() -> String {
        VIRTCONTAINER.to_string()
    }

    fn new_handler() -> Arc<dyn RuntimeHandler> {
        Arc::new(VirtContainer {})
    }

    #[instrument]
    async fn new_instance(
        &self,
        sid: &str,
        msg_sender: Sender<Message>,
        config: Arc<TomlConfig>,
        init_size_manager: InitialSizeManager,
        sandbox_config: SandboxConfig,
    ) -> Result<RuntimeInstance> {
        let factory = config.factory.clone();
        let (hypervisor, agent);
        //如果有Factory设置，则从Factory里读取相关配置
        if factory.template {
            // 这里的config已经被更新过了，所以还是重新加载一下toml_config
            let (toml_config, _) = TomlConfig::load_from_default().context("load toml config")?;
            //Build VMConfig
            let mut vm_config = VMConfig {
                hypervisor_name: toml_config.runtime.hypervisor_name.clone(),
                agent_name: toml_config.runtime.agent_name.clone(),
                hypervisor_config: toml_config
                    .hypervisor
                    .get(&toml_config.runtime.hypervisor_name)
                    .cloned()
                    .unwrap_or_default(),
                agent_config: toml_config
                    .agent
                    .get(&toml_config.runtime.agent_name)
                    .cloned()
                    .unwrap_or_default(),
            };
            let template_path = toml_config.factory.template_path.clone();
            info!(
                sl!(),
                "lib::new_instance(): build VMConfig. VMConfig:{:?}", vm_config
            );
            let vm = factory::get_vm(&mut vm_config, PathBuf::from(template_path)).await?;
            info!(sl!(), "lib::new_instance(): sleep 300s start");
            // thread::sleep(Duration::from_secs(300));
            info!(sl!(), "lib::new_instance(): sleep 300s awake");
            info!(sl!(), "lib::new_instance(): get vm from template.  vm: new_vm() VM id={}, cpu={}, memory={}", vm.id, vm.cpu, vm.memory  );
            // vm.resume().await?;
            hypervisor = vm.hypervisor.clone();
            // hypervisor = new_hypervisor(&config).await.context("new hypervisor")?;
            agent = vm.agent.clone();
            // vm.assign_sandbox(self).await?;
            let addr = vm.hypervisor.get_agent_socket().await?;
            info!(
                sl!(),
                "lib::new_instance(): template vm agent socket addr = {:?}", addr
            );
        } else {
            hypervisor = new_hypervisor(&config).await.context("new hypervisor")?;
            agent = new_agent(&config).context("new agent")?;
        }

        // get uds from hypervisor and get config from toml_config
        // let agent = new_agent(&config).context("new agent")?;
        let resource_manager = Arc::new(
            ResourceManager::new(
                sid,
                agent.clone(),
                hypervisor.clone(),
                config,
                init_size_manager,
            )
            .await?,
        );
        let pid = std::process::id();
        info!(sl!(), "lib::new_instance(): sid={}", sid);
        let sandbox = sandbox::VirtSandbox::new(
            sid,
            msg_sender,
            agent.clone(),
            hypervisor.clone(),
            resource_manager.clone(),
            sandbox_config,
            factory,
        )
        .await
        .context("new virt sandbox")?;
        let container_manager = container_manager::VirtContainerManager::new(
            sid,
            pid,
            agent,
            hypervisor,
            resource_manager,
        );
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

// pub async fn p_new_hypervisor(toml_config: &TomlConfig) -> Result<Arc<dyn Hypervisor>> {
//     new_hypervisor(toml_config).await
// }

pub async fn new_hypervisor(toml_config: &TomlConfig) -> Result<Arc<dyn Hypervisor>> {
    let hypervisor_name = &toml_config.runtime.hypervisor_name;
    let hypervisor_config = toml_config
        .hypervisor
        .get(hypervisor_name)
        .ok_or_else(|| anyhow!("failed to get hypervisor for {}", &hypervisor_name))
        .context("get hypervisor")?;

    // TODO: support other hypervisor
    // issue: https://github.com/kata-containers/kata-containers/issues/4634
    match hypervisor_name.as_str() {
        #[cfg(feature = "dragonball")]
        HYPERVISOR_DRAGONBALL => {
            let hypervisor = Dragonball::new();
            hypervisor
                .set_hypervisor_config(hypervisor_config.clone())
                .await;
            if toml_config.runtime.use_passfd_io {
                hypervisor
                    .set_passfd_listener_port(toml_config.runtime.passfd_listener_port)
                    .await;
            }
            Ok(Arc::new(hypervisor))
        }
        HYPERVISOR_QEMU => {
            let hypervisor = Qemu::new();
            hypervisor
                .set_hypervisor_config(hypervisor_config.clone())
                .await;
            Ok(Arc::new(hypervisor))
        }
        HYPERVISOR_FIRECRACKER => {
            let hypervisor = Firecracker::new();
            hypervisor
                .set_hypervisor_config(hypervisor_config.clone())
                .await;
            Ok(Arc::new(hypervisor))
        }
        #[cfg(all(
            feature = "cloud-hypervisor",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ))]
        HYPERVISOR_NAME_CH => {
            let hypervisor = CloudHypervisor::new();
            hypervisor
                .set_hypervisor_config(hypervisor_config.clone())
                .await;
            Ok(Arc::new(hypervisor))
        }
        HYPERVISOR_REMOTE => {
            let hypervisor = Remote::new();
            hypervisor
                .set_hypervisor_config(hypervisor_config.clone())
                .await;
            Ok(Arc::new(hypervisor))
        }
        _ => Err(anyhow!("Unsupported hypervisor {}", &hypervisor_name)),
    }
}

pub fn new_agent(toml_config: &TomlConfig) -> Result<Arc<KataAgent>> {
    let agent_name = &toml_config.runtime.agent_name;
    let agent_config = toml_config
        .agent
        .get(agent_name)
        .ok_or_else(|| anyhow!("failed to get agent for {}", &agent_name))
        .context("get agent")?;
    match agent_name.as_str() {
        AGENT_KATA => {
            let agent = KataAgent::new(agent_config.clone());
            Ok(Arc::new(agent))
        }
        _ => Err(anyhow!("Unsupported agent {}", &agent_name)),
    }
}

#[cfg(test)]
mod test {

    use super::*;

    fn default_toml_config_agent() -> Result<TomlConfig> {
        let config_content = r#"
[agent.kata]
container_pipe_size=1

[runtime]
agent_name="kata"
        "#;
        TomlConfig::load(config_content).map_err(|e| anyhow!("can not load config toml: {}", e))
    }

    #[test]
    fn test_new_agent() {
        let toml_config = default_toml_config_agent().unwrap();

        let res = new_agent(&toml_config);
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_new_hypervisor() {
        VirtContainer::init().unwrap();

        let toml_config = {
            let config_content = r#"
[hypervisor.qemu]
path = "/bin/echo"
kernel = "/bin/echo"
image = "/bin/echo"
firmware = ""

[runtime]
hypervisor_name="qemu"
"#;
            TomlConfig::load(config_content).map_err(|e| anyhow!("can not load config toml: {}", e))
        }
        .unwrap();

        let res = new_hypervisor(&toml_config).await;
        assert!(res.is_ok());
    }
}
