// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{process::Stdio, sync::Arc};

use agent::Storage;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hypervisor::Hypervisor;
use kata_types::config::hypervisor::SharedFsInfo;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    sync::{
        mpsc::{channel, Receiver, Sender},
        RwLock,
    },
};

use super::{
    share_virtio_fs::generate_sock_path, utils::get_host_ro_shared_path,
    virtio_fs_share_mount::VirtiofsShareMount, ShareFs, ShareFsMount,
};

#[derive(Debug, Clone)]
pub struct ShareVirtioFsStandaloneConfig {
    id: String,
    jail_root: String,

    // virtio_fs_daemon is the virtio-fs vhost-user daemon path
    pub virtio_fs_daemon: String,
    // virtio_fs_cache cache mode for fs version cache or "none"
    pub virtio_fs_cache: String,
    // virtio_fs_extra_args passes options to virtiofsd daemon
    pub virtio_fs_extra_args: Vec<String>,
}

#[derive(Default)]
struct ShareVirtioFsStandaloneInner {
    pid: Option<u32>,
}
pub(crate) struct ShareVirtioFsStandalone {
    inner: Arc<RwLock<ShareVirtioFsStandaloneInner>>,
    config: ShareVirtioFsStandaloneConfig,
    share_fs_mount: Arc<dyn ShareFsMount>,
}

impl ShareVirtioFsStandalone {
    pub(crate) fn new(id: &str, _config: &SharedFsInfo) -> Result<Self> {
        Ok(Self {
            inner: Arc::new(RwLock::new(ShareVirtioFsStandaloneInner::default())),
            // TODO: update with config
            config: ShareVirtioFsStandaloneConfig {
                id: id.to_string(),
                jail_root: "".to_string(),
                virtio_fs_daemon: "".to_string(),
                virtio_fs_cache: "".to_string(),
                virtio_fs_extra_args: vec![],
            },
            share_fs_mount: Arc::new(VirtiofsShareMount::new(id)),
        })
    }

    fn virtiofsd_args(&self, sock_path: &str) -> Result<Vec<String>> {
        let source_path = get_host_ro_shared_path(&self.config.id);
        if !source_path.exists() {
            return Err(anyhow!("The virtiofs shared path didn't exist"));
        }

        let mut args: Vec<String> = vec![
            String::from("-f"),
            String::from("-o"),
            format!("vhost_user_socket={}", sock_path),
            String::from("-o"),
            format!("source={}", source_path.to_str().unwrap()),
            String::from("-o"),
            format!("cache={}", self.config.virtio_fs_cache),
        ];

        if !self.config.virtio_fs_extra_args.is_empty() {
            let mut extra_args: Vec<String> = self.config.virtio_fs_extra_args.clone();
            args.append(&mut extra_args);
        }

        Ok(args)
    }

    async fn setup_virtiofsd(&self) -> Result<()> {
        let sock_path = generate_sock_path(&self.config.jail_root);
        let args = self.virtiofsd_args(&sock_path).context("virtiofsd args")?;

        let mut cmd = Command::new(&self.config.virtio_fs_daemon);
        let child_cmd = cmd.args(&args).stderr(Stdio::piped());
        let child = child_cmd.spawn().context("spawn virtiofsd")?;

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

    async fn setup_device_before_start_vm(&self, _h: &dyn Hypervisor) -> Result<()> {
        self.setup_virtiofsd().await.context("setup virtiofsd")?;
        Ok(())
    }

    async fn setup_device_after_start_vm(&self, _h: &dyn Hypervisor) -> Result<()> {
        Ok(())
    }

    async fn get_storages(&self) -> Result<Vec<Storage>> {
        Ok(vec![])
    }
}
