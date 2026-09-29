// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::time::Duration;
use std::{collections::HashMap, path::Path, process::Stdio, sync::Arc};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    sync::{
        mpsc::{channel, Receiver, Sender},
        watch, Mutex, RwLock,
    },
    task::JoinHandle,
    time::timeout,
};

use agent::Storage;
use hypervisor::{device::device_manager::DeviceManager, utils::chown_to_parent, Hypervisor};
use kata_types::{config::hypervisor::SharedFsInfo, rootless::is_rootless};

use super::{
    share_virtio_fs::generate_sock_path, utils::ensure_dir_exist, utils::get_host_ro_shared_path,
    virtio_fs_share_mount::VirtiofsShareMount, MountedInfo, ShareFs, ShareFsMount,
};
use crate::share_fs::{
    kata_guest_share_dir,
    share_virtio_fs::{
        prepare_virtiofs, FS_TYPE_VIRTIO_FS, KATA_VIRTIO_FS_DEV_TYPE, MOUNT_GUEST_TAG,
    },
    VIRTIO_FS,
};

const VIRTIOFSD_STOP_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct ShareVirtioFsStandaloneConfig {
    id: String,

    // virtio_fs_daemon is the virtio-fs vhost-user daemon path
    pub virtio_fs_daemon: String,
    // virtio_fs_cache cache mode for fs version cache
    pub virtio_fs_cache: String,
    // virtio_fs_extra_args passes options to virtiofsd daemon
    pub virtio_fs_extra_args: Vec<String>,
}

#[derive(Default, Debug)]
struct ShareVirtioFsStandaloneInner {
    stop_tx: Option<watch::Sender<bool>>,
    watcher: Option<JoinHandle<Result<()>>>,
}

pub(crate) struct ShareVirtioFsStandalone {
    inner: Arc<RwLock<ShareVirtioFsStandaloneInner>>,
    config: ShareVirtioFsStandaloneConfig,
    share_fs_mount: Arc<dyn ShareFsMount>,
    mounted_info_set: Arc<Mutex<HashMap<String, MountedInfo>>>,
}

impl ShareVirtioFsStandalone {
    pub(crate) fn new(id: &str, config: &SharedFsInfo) -> Result<Self> {
        Ok(Self {
            inner: Arc::new(RwLock::new(ShareVirtioFsStandaloneInner::default())),
            config: ShareVirtioFsStandaloneConfig {
                id: id.to_string(),
                virtio_fs_daemon: config.virtio_fs_daemon.clone(),
                virtio_fs_cache: config.virtio_fs_cache.clone(),
                virtio_fs_extra_args: config.virtio_fs_extra_args.clone(),
            },
            share_fs_mount: Arc::new(VirtiofsShareMount::new(id)),
            mounted_info_set: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    fn virtiofsd_args(&self, sock_path: &str, disable_guest_selinux: bool) -> Result<Vec<String>> {
        let source_path = get_host_ro_shared_path(&self.config.id);
        ensure_dir_exist(&source_path)?;
        let shared_dir = source_path
            .to_str()
            .ok_or_else(|| anyhow!("convert source path {:?} to str failed", source_path))?;

        let mut args: Vec<String> = vec![
            String::from("--socket-path"),
            String::from(sock_path),
            String::from("--shared-dir"),
            String::from(shared_dir),
            String::from("--cache"),
            self.config.virtio_fs_cache.clone(),
        ];

        if !self.config.virtio_fs_extra_args.is_empty() {
            let mut extra_args: Vec<String> = self.config.virtio_fs_extra_args.clone();
            args.append(&mut extra_args);
        }

        if !disable_guest_selinux {
            args.push(String::from("--xattr"));
        }

        Ok(args)
    }

    async fn setup_virtiofsd(&self, h: &dyn Hypervisor) -> Result<()> {
        let sock_path = generate_sock_path(&h.get_jailer_root().await?);
        let disable_guest_selinux = h.hypervisor_config().await.disable_guest_selinux;

        let socket_path = if is_rootless() {
            // In rootless mode, we use relative socket paths instead of absolute paths
            // because the absolute path with rootless prefix can exceed the unix socket path length limit (typically 108 bytes)
            // By using a relative path and changing the working directory, we can keep the socket path short
            let sock_file = Path::new(sock_path.as_str())
                .file_name()
                .ok_or_else(|| anyhow!("failed to get file name of {:?}", sock_path))?;
            sock_file.to_string_lossy().to_string()
        } else {
            sock_path.clone()
        };

        let args = self
            .virtiofsd_args(&socket_path, disable_guest_selinux)
            .context("virtiofsd args")?;

        let mut cmd = Command::new(&self.config.virtio_fs_daemon);
        let child_cmd = cmd.args(&args).stderr(Stdio::piped());

        if is_rootless() {
            // Change working directory to socket's parent directory
            // This allows virtiofsd to create the socket file using the short relative path
            // avoiding the unix socket path length limitation
            let work_dir = Path::new(&sock_path)
                .parent()
                .ok_or_else(|| anyhow!("failed to get parent dir of {:?}", sock_path))?;
            child_cmd.current_dir(work_dir);
        }

        let mut inner = self.inner.write().await;
        if inner.watcher.is_some() {
            return Err(anyhow!("virtiofsd already initialized"));
        }
        let child = child_cmd.spawn().context("spawn virtiofsd")?;

        let (tx, mut rx): (Sender<Result<()>>, Receiver<Result<()>>) = channel(100);
        let (stop_tx, stop_rx) = watch::channel(false);
        let watcher = tokio::spawn(run_virtiofsd(child, tx, stop_rx));
        inner.stop_tx = Some(stop_tx);
        inner.watcher = Some(watcher);
        drop(inner);

        if is_rootless() {
            // wait for the socket to be created
            for _ in 0..10 {
                if Path::new(&sock_path).exists() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            if let Err(err) = chown_to_parent(&sock_path) {
                if let Err(stop_err) = self.shutdown_virtiofsd().await {
                    warn!(
                        sl!(),
                        "failed to stop virtiofsd after socket error: {stop_err:#}"
                    );
                }
                return Err(err).context("chown virtiofsd socket");
            }
        }

        // TODO: support timeout
        match rx.recv().await {
            Some(Ok(_)) => {
                info!(sl!(), "start virtiofsd successfully");
                Ok(())
            }
            result => {
                error!(sl!(), "failed to start virtiofsd: {:?}", result);
                self.shutdown_virtiofsd()
                    .await
                    .context("shutdown_virtiofsd")?;
                Err(anyhow!("failed to start virtiofsd"))
            }
        }
    }

    async fn shutdown_virtiofsd(&self) -> Result<()> {
        let mut inner = self.inner.write().await;

        if let Some(stop_tx) = inner.stop_tx.as_ref() {
            let _ = stop_tx.send(true);
        }
        if let Some(watcher) = inner.watcher.as_mut() {
            timeout(VIRTIOFSD_STOP_TIMEOUT, watcher)
                .await
                .context("timed out waiting for virtiofsd to exit")?
                .context("virtiofsd watcher failed")??;
        }
        inner.stop_tx = None;
        inner.watcher = None;

        Ok(())
    }
}

async fn run_virtiofsd(
    mut child: Child,
    tx: Sender<Result<()>>,
    mut stop_rx: watch::Receiver<bool>,
) -> Result<()> {
    let stderr = child.stderr.take().context("capture virtiofsd stderr")?;
    let reader = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Some(buffer) = lines.next_line().await.context("read next line")? {
            let trim_buffer = buffer.trim_end();
            if !trim_buffer.is_empty() {
                info!(sl!(), "source: virtiofsd {}", trim_buffer);
            }
            if buffer.contains("Waiting for vhost-user socket connection") {
                let _ = tx.send(Ok(())).await;
            }
        }
        Ok::<(), anyhow::Error>(())
    });

    let status = tokio::select! {
        status = child.wait() => status.context("wait for virtiofsd").map(|_| ()),
        _ = async {
            while !*stop_rx.borrow_and_update() {
                if stop_rx.changed().await.is_err() {
                    break;
                }
            }
        } => child.kill().await.context("kill virtiofsd"),
    };
    reader.abort();
    status
}

#[async_trait]
impl ShareFs for ShareVirtioFsStandalone {
    fn get_share_fs_mount(&self) -> Arc<dyn ShareFsMount> {
        self.share_fs_mount.clone()
    }

    async fn setup_device_before_start_vm(
        &self,
        h: &dyn Hypervisor,
        d: &RwLock<DeviceManager>,
    ) -> Result<()> {
        prepare_virtiofs(d, VIRTIO_FS, &self.config.id, &h.get_jailer_root().await?)
            .await
            .context("prepare virtiofs")?;
        self.setup_virtiofsd(h).await.context("setup virtiofsd")?;

        Ok(())
    }

    async fn setup_device_after_start_vm(
        &self,
        _h: &dyn Hypervisor,
        _d: &RwLock<DeviceManager>,
    ) -> Result<()> {
        Ok(())
    }

    async fn get_storages(&self) -> Result<Vec<Storage>> {
        let mut storages: Vec<Storage> = Vec::new();

        let shared_volume: Storage = Storage {
            driver: String::from(KATA_VIRTIO_FS_DEV_TYPE),
            driver_options: Vec::new(),
            source: String::from(MOUNT_GUEST_TAG),
            fs_type: String::from(FS_TYPE_VIRTIO_FS),
            fs_group: None,
            options: vec![String::from("nodev")],
            mount_point: kata_guest_share_dir(),
            shared: false,
        };

        storages.push(shared_volume);
        Ok(storages)
    }

    fn mounted_info_set(&self) -> Arc<Mutex<HashMap<String, MountedInfo>>> {
        self.mounted_info_set.clone()
    }

    async fn stop(&self) -> Result<()> {
        self.shutdown_virtiofsd()
            .await
            .context("failed to stop virtiofsd daemon")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn shutdown_waits_for_virtiofsd_and_can_repeat() {
        let share = ShareVirtioFsStandalone::new("test", &SharedFsInfo::default()).unwrap();
        let child = Command::new("sleep")
            .arg("30")
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let (tx, _rx) = channel(1);
        let (stop_tx, stop_rx) = watch::channel(false);
        let watcher = tokio::spawn(run_virtiofsd(child, tx, stop_rx));
        {
            let mut inner = share.inner.write().await;
            inner.stop_tx = Some(stop_tx);
            inner.watcher = Some(watcher);
        }

        share.shutdown_virtiofsd().await.unwrap();
        let inner = share.inner.read().await;
        assert!(inner.stop_tx.is_none());
        assert!(inner.watcher.is_none());
        drop(inner);
        share.shutdown_virtiofsd().await.unwrap();
    }

    #[tokio::test]
    async fn shutdown_keeps_watcher_after_cancellation() {
        let share = ShareVirtioFsStandalone::new("test", &SharedFsInfo::default()).unwrap();
        let (tx, rx) = oneshot::channel::<()>();
        share.inner.write().await.watcher = Some(tokio::spawn(async move {
            rx.await.context("wait for test release")?;
            Ok(())
        }));

        assert!(
            timeout(Duration::from_millis(10), share.shutdown_virtiofsd())
                .await
                .is_err()
        );
        assert!(share.inner.read().await.watcher.is_some());
        tx.send(()).unwrap();
        share.shutdown_virtiofsd().await.unwrap();
        assert!(share.inner.read().await.watcher.is_none());
    }
}
