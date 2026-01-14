// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{collections::HashMap, path::Path, process::Stdio, sync::Arc};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    sync::{
        mpsc::{channel, Receiver, Sender},
        Mutex, RwLock,
    },
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
    pid: Option<u32>,
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
            String::from("--sandbox"),
            String::from("none"),
            String::from("--seccomp"),
            String::from("none"),
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

        let child = child_cmd.spawn().context("spawn virtiofsd")?;

        if is_rootless() {
            // wait for the socket to be created
            for _ in 0..10 {
                if Path::new(&sock_path).exists() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            chown_to_parent(&sock_path)?;
        }

        // update virtiofsd pid{
        {
            let mut inner = self.inner.write().await;
            inner.pid = child.id();
        }

        let (tx, mut rx): (Sender<Result<()>>, Receiver<Result<()>>) = channel(100);
        tokio::spawn(run_virtiofsd(child, tx));

        // TODO: support timeout
        match rx.recv().await.unwrap() {
            Ok(_) => {
                info!(sl!(), "start virtiofsd successfully");
                Ok(())
            }
            Err(e) => {
                error!(sl!(), "failed to start virtiofsd {}", e);
                self.shutdown_virtiofsd()
                    .await
                    .context("shutdown_virtiofsd")?;
                Err(anyhow!("failed to start virtiofsd"))
            }
        }
    }

    async fn shutdown_virtiofsd(&self) -> Result<()> {
        let mut inner = self.inner.write().await;

        if let Some(pid) = inner.pid.take() {
            info!(sl!(), "shutdown virtiofsd pid {}", pid);
            let pid = ::nix::unistd::Pid::from_raw(pid as i32);
            if let Err(err) = ::nix::sys::signal::kill(pid, nix::sys::signal::SIGKILL) {
                if err != ::nix::Error::ESRCH {
                    return Err(anyhow!("failed to kill virtiofsd pid {} {}", pid, err));
                }
            }
        }
        inner.pid = None;

        Ok(())
    }
}

async fn run_virtiofsd(mut child: Child, tx: Sender<Result<()>>) -> Result<()> {
    let stderr = child.stderr.as_mut().unwrap();
    let stderr_reader = BufReader::new(stderr);
    let mut lines = stderr_reader.lines();

    while let Some(buffer) = lines.next_line().await.context("read next line")? {
        let trim_buffer = buffer.trim_end();
        if !trim_buffer.is_empty() {
            info!(sl!(), "source: virtiofsd {}", trim_buffer);
        }
        if buffer.contains("Waiting for vhost-user socket connection") {
            tx.send(Ok(())).await.unwrap();
        }
    }

    info!(sl!(), "wait virtiofsd {:?}", child.wait().await);
    Ok(())
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
        };

        storages.push(shared_volume);
        Ok(storages)
    }

    fn mounted_info_set(&self) -> Arc<Mutex<HashMap<String, MountedInfo>>> {
        self.mounted_info_set.clone()
    }
}
