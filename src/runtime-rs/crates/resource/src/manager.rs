// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use std::sync::Arc;

use agent::{Agent, Storage};
use anyhow::{Context, Result};
use async_trait::async_trait;
use hypervisor::device::device_manager::DeviceManager;
use hypervisor::Hypervisor;
use kata_types::config::TomlConfig;
use kata_types::mount::Mount;
use oci::{Linux, LinuxResources};
use oci_spec::runtime as oci;
use persist::sandbox_persist::Persist;
use tokio::sync::RwLock;
use tracing::instrument;

use crate::cdi_devices::ContainerDevice;
use crate::cpu_mem::initial_size::InitialSizeManager;
use crate::network::{NetworkConfig, NetworkWithNetNsConfig};
use crate::resource_persist::ResourceState;
use crate::ResourceUpdateOp;
use crate::{manager_inner::ResourceManagerInner, rootfs::Rootfs, volume::Volume, ResourceConfig};

pub struct ManagerArgs {
    pub sid: String,
    pub agent: Arc<dyn Agent>,
    pub hypervisor: Arc<dyn Hypervisor>,
    pub config: TomlConfig,
}

pub struct ResourceManager {
    inner: Arc<RwLock<ResourceManagerInner>>,
}

impl std::fmt::Debug for ResourceManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResourceManager").finish()
    }
}

impl ResourceManager {
    pub async fn new(
        sid: &str,
        agent: Arc<dyn Agent>,
        hypervisor: Arc<dyn Hypervisor>,
        toml_config: Arc<TomlConfig>,
        init_size_manager: InitialSizeManager,
    ) -> Result<Self> {
        // Regist resource logger for later use.
        logging::register_subsystem_logger("runtimes", "resource");

        Ok(Self {
            inner: Arc::new(RwLock::new(
                ResourceManagerInner::new(sid, agent, hypervisor, toml_config, init_size_manager)
                    .await?,
            )),
        })
    }

    pub async fn config(&self) -> Arc<TomlConfig> {
        let inner = self.inner.read().await;
        inner.config()
    }

    pub async fn get_device_manager(&self) -> Arc<RwLock<DeviceManager>> {
        let inner = self.inner.read().await;
        inner.get_device_manager()
    }

    #[instrument]
    pub async fn prepare_before_start_vm(&self, device_configs: Vec<ResourceConfig>) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.prepare_before_start_vm(device_configs).await
    }

    /// Create the network without attaching anything, so the caller can put
    /// what construction made in the saved state before attach_network()
    /// acts on the host and the VM. A created network whose attach did not
    /// finish is never replaced: it holds the only record of its endpoints.
    pub async fn create_network(&self, network_config: NetworkConfig) -> Result<()> {
        // Construction and registration run detached from this caller. A
        // caller cancelled at its await would otherwise let the network
        // thread finish and drop a built network — its host-side links
        // already made — before anything a cleanup can find had hold of it.
        // A dropped JoinHandle does not abort the task, so the network is
        // registered however the caller fares.
        let inner = self.inner.clone();
        tokio::spawn(async move {
            let mut inner = inner.write().await;
            inner.create_network(network_config).await
        })
        .await
        .context("await the network creation task")?
    }

    /// Attach the created network's endpoints. A retry after a failure
    /// resumes after the endpoints that attached; nothing here keeps a
    /// concurrent cleanup from detaching endpoints mid-attach, so callers
    /// must not race it against one.
    pub async fn attach_network(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.attach_network().await
    }

    /// Whether a created network is still waiting for its attach, so its
    /// setup can be resumed rather than redone.
    pub async fn network_pending_attach(&self) -> bool {
        let inner = self.inner.read().await;
        inner.network_pending_attach()
    }

    #[instrument]
    pub async fn setup_after_start_vm(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.setup_after_start_vm().await
    }

    /// Poll the netns until interfaces exist (Docker 26+). Returns whether
    /// the sandbox still needs its network set up: false when rescanning
    /// does not apply, the network is already configured, or no interface
    /// showed up within the window. The poll is a lightweight netlink scan
    /// (no endpoint creation, no hypervisor attachment) run via
    /// `spawn_blocking`, so the async executor and the resource-manager lock
    /// are not held during it. The caller then creates, records and attaches
    /// the network, and pushes it to the agent with
    /// [`Self::apply_rescanned_network`].
    pub async fn rescan_found_interfaces(&self, net_cfg: &NetworkWithNetNsConfig) -> Result<bool> {
        use anyhow::anyhow;
        use std::time::{Duration, Instant};

        // Fast early-out checks (read lock only).
        {
            let inner = self.inner.read().await;
            if inner.rescan_should_skip(net_cfg) {
                return Ok(false);
            }
            if inner.network_has_interfaces().await? {
                return Ok(false);
            }
        }

        const POLL: Duration = Duration::from_millis(50);
        let deadline = Instant::now() + Duration::from_secs(5);

        let netns_path = net_cfg.netns_path.clone();
        let found = tokio::task::spawn_blocking(move || -> Result<bool> {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_io()
                .build()?;
            loop {
                if rt.block_on(crate::network::netns_has_interfaces(&netns_path))? {
                    return Ok(true);
                }
                if Instant::now() >= deadline {
                    return Ok(false);
                }
                std::thread::sleep(POLL);
            }
        })
        .await
        .map_err(|e| anyhow!("{:?}", e))
        .context("netns poll task join")?
        .context("netns poll")?;

        if !found {
            info!(
                sl!(),
                "no network interfaces after rescan timeout; networking may rely on hooks only"
            );
        }
        Ok(found)
    }

    /// Push the rescanned, attached network's configuration to the guest
    /// agent (read lock only).
    pub async fn apply_rescanned_network(&self) -> Result<()> {
        let inner = self.inner.read().await;
        let Some(network) = inner.network() else {
            return Ok(());
        };
        let ifs = network
            .interfaces()
            .await
            .context("rescan get interfaces")?;
        if ifs.is_empty() {
            return Ok(());
        }
        inner
            .apply_network_to_agent(network.as_ref())
            .await
            .context("apply network to agent")
    }

    pub async fn get_storage_for_sandbox(&self, shm_size: u64) -> Result<Vec<Storage>> {
        let inner = self.inner.read().await;
        inner.get_storage_for_sandbox(shm_size).await
    }

    pub async fn handler_rootfs(
        &self,
        cid: &str,
        root: &oci::Root,
        bundle_path: &str,
        rootfs_mounts: &[Mount],
        annotations: &HashMap<String, String>,
    ) -> Result<Arc<dyn Rootfs>> {
        let inner = self.inner.read().await;
        inner
            .handler_rootfs(cid, root, bundle_path, rootfs_mounts, annotations)
            .await
    }

    /// Record the host artifacts creating a container from `spec` will make,
    /// before any of them exists. Returns the record generation a save would
    /// cover, or 0 when nothing waits for one.
    pub async fn register_container_intents(&self, spec: &oci::Spec) -> Result<u64> {
        let inner = self.inner.read().await;
        inner.register_container_intents(spec).await
    }

    /// Confirm that a save wrote the state out covering `generation`.
    pub async fn container_intents_saved(&self, generation: u64) {
        let inner = self.inner.read().await;
        inner.container_intents_saved(generation).await
    }

    pub async fn handler_volumes(
        &self,
        cid: &str,
        spec: &oci::Spec,
    ) -> Result<Vec<Arc<dyn Volume>>> {
        let inner = self.inner.read().await;
        inner.handler_volumes(cid, spec).await
    }

    pub async fn handler_devices(
        &self,
        cid: &str,
        linux: &Linux,
        device_ids: &mut Vec<String>,
    ) -> Result<Vec<ContainerDevice>> {
        let inner = self.inner.read().await;
        inner.handler_devices(cid, linux, device_ids).await
    }

    pub async fn dump(&self) {
        let inner = self.inner.read().await;
        inner.dump().await
    }

    pub async fn update_linux_resource(
        &self,
        cid: &str,
        linux_resources: Option<&LinuxResources>,
        op: ResourceUpdateOp,
    ) -> Result<Option<LinuxResources>> {
        let inner = self.inner.read().await;
        inner.update_linux_resource(cid, linux_resources, op).await
    }

    pub async fn cleanup(&self) -> Result<()> {
        let inner = self.inner.read().await;
        inner.cleanup().await
    }

    pub async fn has_passthrough_network_devices(&self) -> bool {
        let inner = self.inner.read().await;
        inner.has_passthrough_network_devices().await
    }

    /// Kills every process left in the sandbox cgroups and waits until
    /// they are empty. Only for a shim that does not run in them.
    pub async fn kill_sandbox_processes(&self, wait: std::time::Duration) -> Result<()> {
        let inner = self.inner.read().await;
        inner.cgroups_resource.kill_all_processes(wait).await
    }
}

#[async_trait]
impl Persist for ResourceManager {
    type State = ResourceState;
    type ConstructorArgs = ManagerArgs;

    /// Save a state of ResourceManager
    async fn save(&self) -> Result<Self::State> {
        let inner = self.inner.read().await;
        inner.save().await
    }

    /// Restore ResourceManager
    async fn restore(
        resource_args: Self::ConstructorArgs,
        resource_state: Self::State,
    ) -> Result<Self> {
        let inner = ResourceManagerInner::restore(resource_args, resource_state).await?;
        Ok(Self {
            inner: Arc::new(RwLock::new(inner)),
        })
    }
}
