// Copyright (c) 2025 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use agent::Agent;
use anyhow::{anyhow, Context, Error, Result};
use hypervisor::{
    device::{
        device_manager::{do_handle_device, get_block_driver, DeviceManager},
        DeviceConfig, DeviceType,
    },
    BlockConfig,
};
use nix::sched::sched_yield;
use nix::sys::statvfs::statvfs;
use std::fmt;
use std::fs::Permissions;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::fs;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tokio::task;
use tokio::task::spawn_blocking;
use tokio::time::sleep;

use crate::cpu_mem::mem::MemResource;

const CHUNK_SIZE: usize = 1024 * 1024;
const ERROR_WAIT_SECS: u64 = 120;
const ONE_MB: usize = 1024 * 1024;
const ERROR_RETRY_TIMES_MAX: usize = 2;

async fn check_disk_size(path: &Path, mut size: usize) -> Result<()> {
    let task_path = path.to_path_buf();

    let stat = spawn_blocking(move || statvfs(&task_path))
        .await
        .context("spawn_blocking")?
        .context("statvfs")?;

    let available_space = stat.blocks_available() * stat.block_size();

    size += ONE_MB * 1024;

    if available_space < size as u64 {
        Err(anyhow::anyhow!(
            "Not enough space on disk to create swap file {:?}",
            path.to_path_buf()
        ))
    } else {
        Ok(())
    }
}

async fn check_mkswap() -> Result<()> {
    Command::new("mkswap").arg("--help").output().await?;

    Ok(())
}

#[derive(Debug, Clone)]
struct Core {
    current_swap_size: usize,
    next_swap_id: usize,
    stopped: bool,
}

impl Core {
    fn new() -> Self {
        Self {
            current_swap_size: 0,
            next_swap_id: 0,
            stopped: false,
        }
    }

    fn update_next_swap_id(&mut self) {
        self.next_swap_id += 1;
    }

    fn plus_swap_size(&mut self, size: usize) {
        self.current_swap_size += size;
    }

    fn stop(&mut self) {
        self.stopped = true;
    }
}

#[derive(Clone)]
pub struct SwapTask {
    path: PathBuf,
    size_percent: usize,
    create_threshold_secs: u64,
    core: Arc<Mutex<Core>>,
    wake_rx: Arc<Mutex<mpsc::Receiver<()>>>,
    mem: MemResource,
    agent: Arc<dyn Agent>,
    device_manager: Arc<RwLock<DeviceManager>>,
}

#[derive(Debug, PartialEq)]
struct StStop {
    need_remove: bool,
}
impl StStop {
    fn get_error(need_remove: bool) -> Error {
        Self { need_remove }.into()
    }
}
impl fmt::Display for StStop {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "swap_task stop.  Need remove {}", self.need_remove)
    }
}
impl std::error::Error for StStop {}

impl SwapTask {
    // Return true if need remove runtime_path.join(format!("swap{}", core.lock().unwrap().next_swap_id))
    async fn run(&mut self) -> Result<()> {
        sleep(Duration::from_secs(self.create_threshold_secs)).await;

        if self.should_stop(true).await {
            return Err(StStop::get_error(false));
        }

        let current_size = self.mem.get_current_mb().await.context("get_current_mb")? as usize
            * ONE_MB
            * self.size_percent
            / 100;
        let current_swap_size = { self.core.lock().await.current_swap_size };

        if current_size <= current_swap_size {
            debug!(
                sl!(),
                "swap_task: current memory {} current swap {}, stop",
                current_size,
                current_swap_size
            );
            return Err(StStop::get_error(false));
        }

        let swap_size = current_size - current_swap_size;
        let swap_path = self.get_swap_path().await;

        self.create_disk(swap_size, &swap_path).await?;

        let swap_path = swap_path.to_string_lossy().to_string();

        // Add swap file to sandbox
        let block_driver = get_block_driver(&self.device_manager).await;
        let dev_info = DeviceConfig::BlockCfg(BlockConfig {
            path_on_host: swap_path.clone(),
            driver_option: block_driver,
            is_direct: Some(true),
            ..Default::default()
        });

        if self.should_stop(false).await {
            return Err(StStop::get_error(true));
        }

        let device_info = do_handle_device(&self.device_manager.clone(), &dev_info)
            .await
            .context("do_handle_device")?;
        let device_id = match device_info {
            DeviceType::Block(ref bdev) => bdev.device_id.clone(),
            _ => return Err(anyhow!("swap_task: device_info {} is not Block", swap_path)),
        };

        sleep(Duration::from_secs(1)).await;

        if self.should_stop(false).await {
            if let Err(e1) = self
                .device_manager
                .write()
                .await
                .try_remove_device(&device_id)
                .await
            {
                error!(
                    sl!(),
                    "swap_task: try_remove_device {} fail: {:?}", swap_path, e1
                );
            }

            return Err(StStop::get_error(true));
        }

        if let DeviceType::Block(device) = device_info {
            let ret = if let Some(pci_path) = device.config.pci_path.clone() {
                self.agent.add_swap(agent::types::AddSwapRequest {
                    pci_path: pci_path.slots.iter().map(|slot| slot.0 as u32).collect(),
                })
            } else if !device.config.virt_path.is_empty() {
                self.agent.add_swap_path(agent::types::AddSwapPathRequest {
                    path: device.config.virt_path.clone(),
                })
            } else {
                return Err(anyhow!(
                    "swap_task: device_info {} pci_path is None",
                    swap_path
                ));
            };

            if let Err(e) = ret.await {
                if let Err(e1) = self
                    .device_manager
                    .write()
                    .await
                    .try_remove_device(&device_id)
                    .await
                {
                    error!(
                        sl!(),
                        "swap_task: try_remove_device {} fail: {:?}", swap_path, e1
                    );
                }

                return Err(anyhow!("swap_task: agent.swap_add failed: {:?}", e));
            }
        } else {
            return Err(anyhow!("swap_task: device_info {} is not Block", swap_path));
        }

        let mut core_lock = self.core.lock().await;
        core_lock.update_next_swap_id();
        core_lock.plus_swap_size(swap_size);

        info!(
            sl!(),
            "swap_task: swap file {:?} {} inserted", swap_path, swap_size
        );

        Ok(())
    }

    async fn create_disk(&mut self, swap_size: usize, swap_path: &PathBuf) -> Result<()> {
        debug!(
            sl!(),
            "swap_task: create_disk {:?} {} start", swap_path, swap_size
        );

        check_disk_size(&self.path, swap_size)
            .await
            .context("check_disk_size")?;

        debug!(
            sl!(),
            "swap_task: create swap file {:?} {}", swap_path, swap_size
        );
        let mut file = File::create(swap_path)
            .await
            .context(format!("swap: File::create {:?}", swap_path))?;
        fs::set_permissions(swap_path, Permissions::from_mode(0o700))
            .await
            .context(format!("swap: File::set_permissions {:?}", swap_path))?;

        let buffer = vec![0; CHUNK_SIZE];
        let mut total_written = 0;
        while total_written < swap_size {
            spawn_blocking(sched_yield)
                .await
                .context("swap_task: task::spawn_blocking")?
                .context("swap_task: sched_yield")?;

            if self.should_stop(false).await {
                return Err(StStop::get_error(true));
            }

            let remaining = swap_size - total_written;
            let write_size = std::cmp::min(remaining, CHUNK_SIZE);
            file.write_all(&buffer[..write_size])
                .await
                .context("file.write_all")?;
            total_written += write_size;
        }

        if self.should_stop(false).await {
            return Err(StStop::get_error(true));
        }

        file.flush().await.context("file.flush")?;
        drop(file);

        if self.should_stop(false).await {
            return Err(StStop::get_error(true));
        }

        let output = Command::new("mkswap")
            .arg(swap_path)
            .output()
            .await
            .context("mkswap command")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "mkswap command fail: {}",
                String::from_utf8(output.stdout).unwrap_or_default()
            ));
        }

        debug!(
            sl!(),
            "swap_task: created swap file {:?} {}", swap_path, swap_size
        );

        Ok(())
    }

    async fn should_stop(&self, clear_wake: bool) -> bool {
        if clear_wake {
            if let Err(e) = self.wake_rx.lock().await.try_recv() {
                error!(
                    sl!(),
                    "swap_task: should_keep_run wake_rx.try_recv() {:?}", e
                );
            }
        }
        self.core.lock().await.stopped
    }

    // Return true if thread should stop
    async fn wait_wake(&self) -> bool {
        {
            if self.core.lock().await.stopped {
                return true;
            }
        }

        {
            if self.wake_rx.lock().await.recv().await.is_none() {
                return true;
            }
        }

        self.core.lock().await.stopped
    }

    async fn get_swap_path(&self) -> PathBuf {
        let id = self.core.lock().await.next_swap_id;
        self.path.join(format!("swap{}", id))
    }
}

#[derive(Debug, Clone)]
struct SwapResourceInner {
    wake_tx: mpsc::Sender<()>,
    core: Arc<Mutex<Core>>,
    swap_task_handle: Arc<Mutex<task::JoinHandle<()>>>,
}

#[derive(Debug, Clone)]
pub struct SwapResource {
    runtime_path: PathBuf,
    inner: Option<SwapResourceInner>,
}

impl SwapResource {
    async fn new_inner(
        core: Arc<Mutex<Core>>,
        runtime_path: PathBuf,
        size_percent: u64,
        create_threshold_secs: u64,
        mem: MemResource,
        agent: Arc<dyn Agent>,
        device_manager: Arc<RwLock<DeviceManager>>,
    ) -> Result<Self> {
        check_mkswap().await.context("check_mkswap")?;

        fs::create_dir_all(&runtime_path)
            .await
            .context(format!("fs::create_dir_all {:?}", &runtime_path))?;
        match fs::set_permissions(&runtime_path, Permissions::from_mode(0o700)).await {
            Ok(_) => Ok(()),
            Err(e) => {
                if let Err(e2) = fs::remove_dir_all(&runtime_path).await {
                    error!(
                        sl!(),
                        "swap: fs::remove_dir_all {:?} fail: {:?}", &runtime_path, e2
                    );
                }
                Err(anyhow!(
                    "swap: set_permissions {:?} failed: {:?}",
                    &runtime_path,
                    e
                ))
            }
        }?;

        let (wake_tx, wake_rx) = mpsc::channel(1);

        let mut st = SwapTask {
            path: runtime_path.clone(),
            size_percent: size_percent as usize,
            create_threshold_secs,
            core: core.clone(),
            wake_rx: Arc::new(Mutex::new(wake_rx)),
            mem,
            agent,
            device_manager,
        };
        let swap_task_handle = task::spawn(async move {
            info!(sl!(), "swap_task {:?} start", st.path);

            loop {
                if st.wait_wake().await {
                    break;
                }

                let mut error_retry_times = 0;
                let mut keep_run = true;

                while keep_run {
                    keep_run = false;

                    let need_remove = match st.run().await {
                        Ok(_) => false,
                        Err(e) => {
                            error!(sl!(), "swap_task in {:?} fail: {:?}", st.path, e);
                            if let Some(custom_error) = e.downcast_ref::<StStop>() {
                                custom_error.need_remove
                            } else {
                                keep_run = true;
                                true
                            }
                        }
                    };
                    debug!(sl!(), "swap_task: run stop");

                    if need_remove {
                        let swap_path = st.get_swap_path().await;
                        if swap_path.exists() {
                            if let Err(e) = fs::remove_file(&swap_path).await {
                                error!(
                                    sl!(),
                                    "swap_task error handler remove_file {:?} fail: {:?}",
                                    swap_path,
                                    e
                                );
                                st.core.lock().await.update_next_swap_id();
                            }
                        }
                    }

                    if keep_run {
                        error_retry_times += 1;
                        if error_retry_times > ERROR_RETRY_TIMES_MAX {
                            error!(sl!(), "swap_task {:?} error retry times exceed", st.path);
                            keep_run = false;
                        } else {
                            sleep(Duration::from_secs(ERROR_WAIT_SECS)).await;
                        }
                    }
                }
            }

            info!(sl!(), "swap_task {:?} stop", st.path);
        });

        Ok(Self {
            runtime_path,
            inner: Some(SwapResourceInner {
                wake_tx,
                core,
                swap_task_handle: Arc::new(Mutex::new(swap_task_handle)),
            }),
        })
    }

    pub(crate) async fn new(
        runtime_path: PathBuf,
        size_percent: u64,
        create_threshold_secs: u64,
        mem: MemResource,
        agent: Arc<dyn Agent>,
        device_manager: Arc<RwLock<DeviceManager>>,
    ) -> Result<Self> {
        let core = Arc::new(Mutex::new(Core::new()));
        Self::new_inner(
            core,
            runtime_path,
            size_percent,
            create_threshold_secs,
            mem,
            agent,
            device_manager,
        )
        .await
    }

    pub(crate) async fn restore(runtime_path: PathBuf) -> Self {
        Self {
            runtime_path,
            inner: None,
        }
    }

    fn wakeup_thread(&self) {
        if let Some(inner) = &self.inner {
            if let Err(e) = inner.wake_tx.try_send(()) {
                error!(sl!(), "swap wakeup_thread wake_tx try_send fail: {:?}", e);
            }
        } else {
            error!(sl!(), "swap wakeup_thread no inner");
        }
    }

    pub async fn update(&self) {
        self.wakeup_thread();
    }

    async fn stop(&self) {
        if let Some(inner) = &self.inner {
            inner.core.lock().await.stop();
        }

        self.wakeup_thread();

        if let Some(inner) = &self.inner {
            let mut handle = inner.swap_task_handle.lock().await;
            let join_handle = std::mem::replace(&mut *handle, task::spawn(async {}));
            join_handle.await.unwrap();
        }
    }

    pub async fn clean(&self) {
        self.stop().await;

        if let Err(e) = fs::remove_dir_all(&self.runtime_path).await {
            error!(
                sl!(),
                "swap fs::remove_dir_all {:?} fail: {:?}", self.runtime_path, e
            );
        }
    }
}
