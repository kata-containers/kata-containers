// Copyright (c) 2025 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//
// Description: Helper to setup virtio-fs shared path between host & guest

use anyhow::{anyhow, Context, Result};
use hypervisor::Hypervisor;
use hypervisor::{
    device::{
        device_manager::{do_handle_device, DeviceManager},
        DeviceConfig,
    },
    ShareFsConfig,
};
use kata_types::config::hypervisor::SharedFsInfo;
use slog::debug;
use std::{path::Path, process::Stdio, sync::Arc};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    sync::{
        mpsc::{channel, Receiver, Sender},
        RwLock,
    },
};

pub const VIRTIO_FS: &str = "virtio-fs";
pub const MOUNT_GUEST_TAG: &str = "kataShared";
const VIRTIO_FS_SOCKET: &str = "virtiofsd.sock";

// Source: a rw root path created in /tmp and appended with the vm name
pub const VIRTIO_FS_ROOT_PATH: &str = "/tmp";

#[derive(Clone, Default)]
pub struct SharedFs {
    pub pid: u32,
    pub shared_path: String,
}

// Setup up virtio-fs file share between host & guest.
// a. Create the shared root path
// b. Plugin the device in the guest VM
// c. Start the virtiofs daemon
pub(crate) async fn setup_virtio_fs(
    hypervisor: Arc<dyn Hypervisor>,
    dev_mgr: Arc<RwLock<DeviceManager>>,
    root_path: &str,
) -> Result<SharedFs> {
    // If hypervisor config does not support fs sharing, return
    if !hypervisor.capabilities().await?.is_fs_sharing_supported() {
        return Ok(SharedFs::default());
    }

    let shared_fs_info = hypervisor.hypervisor_config().await.shared_fs;

    let shared_fs = shared_fs_info.shared_fs.clone().unwrap_or_default();

    if !shared_fs.as_str().eq(VIRTIO_FS) {
        return Err(anyhow!("Unsupported virtio-fs type: {:?}", &shared_fs));
    }

    // Create the rootfs dir
    let host_path = [VIRTIO_FS_ROOT_PATH, root_path].join("/");
    std::fs::create_dir_all(&host_path).context("virtio-fs:: failed to create root path")?;

    // plugin the device
    let share_fs_config = ShareFsConfig {
        host_shared_path: host_path.clone(),
        sock_path: generate_sock_path(&host_path),
        mount_tag: String::from(MOUNT_GUEST_TAG),
        fs_type: VIRTIO_FS.to_string(),
        queue_size: 0,
        queue_num: 0,
        options: vec![],
        mount_config: None,
    };

    do_handle_device(&dev_mgr, &DeviceConfig::ShareFsCfg(share_fs_config))
        .await
        .context("virtio-fs:: add virtio-fs failed")?;

    // start the virtio fs daemon
    let virtiofsd_pid = start_virtiofsd(shared_fs_info.clone(), &host_path)
        .await
        .context("virtio-fs:: starting daemon")?;

    Ok(SharedFs {
        pid: virtiofsd_pid,
        shared_path: host_path,
    })
}

fn generate_sock_path(root: &str) -> String {
    let socket_path = Path::new(root).join(VIRTIO_FS_SOCKET);
    socket_path.to_str().unwrap().to_string()
}

fn virtiofsd_args(cfg: SharedFsInfo, shared_dir: &str, sock_path: &str) -> Result<Vec<String>> {
    let mut args: Vec<String> = vec![
        String::from("--socket-path"),
        String::from(sock_path),
        String::from("--shared-dir"),
        String::from(shared_dir),
        String::from("--cache"),
        cfg.virtio_fs_cache.clone(),
        String::from("--sandbox"),
        String::from("none"),
    ];

    if !cfg.virtio_fs_extra_args.is_empty() {
        let mut extra_args: Vec<String> = cfg.virtio_fs_extra_args.clone();
        args.append(&mut extra_args);
    }

    Ok(args)
}

async fn start_virtiofsd(share_fs_info: SharedFsInfo, root_path: &str) -> Result<u32> {
    let sock_path = generate_sock_path(root_path);
    let args =
        virtiofsd_args(share_fs_info.clone(), root_path, &sock_path).context("virtiofsd args")?;

    let mut cmd = Command::new(&share_fs_info.virtio_fs_daemon);
    let child_cmd = cmd.args(&args).stderr(Stdio::piped());
    let child = child_cmd.spawn().context("spawn virtiofsd")?;

    let child_pid = child.id().unwrap_or_default();

    let (tx, mut rx): (Sender<Result<()>>, Receiver<Result<()>>) = channel(100);
    tokio::spawn(run_virtiofsd(child, tx));

    match rx.recv().await.unwrap() {
        Ok(_) => {
            debug!(sl!(), "started virtiofsd successfully");
        }
        Err(e) => {
            debug!(sl!(), "failed to start virtiofsd {}", e);
            shutdown_virtiofsd(SharedFs {
                pid: child_pid,
                shared_path: root_path.to_string(),
            })
            .await
            .context("shutdown_virtiofsd")?;
            return Err(anyhow!("failed to start virtiofsd"));
        }
    }

    Ok(child_pid)
}

pub(crate) async fn shutdown_virtiofsd(info: SharedFs) -> Result<()> {
    debug!(sl!(), "virtio-fs:: shutdown virtiofsd pid {}", info.pid);

    if info.pid == 0 {
        debug!(sl!(), "virtio-fs: not running");
        return Ok(());
    }

    let pid = ::nix::unistd::Pid::from_raw(info.pid as i32);

    if let Err(err) = ::nix::sys::signal::kill(pid, nix::sys::signal::SIGKILL) {
        if err != ::nix::Error::ESRCH {
            return Err(anyhow!("failed to kill virtiofsd pid {} {}", pid, err));
        }
    }

    std::fs::remove_dir_all(&info.shared_path)
        .context("virtio-fs: Failed to delete shared path")?;

    Ok(())
}

async fn run_virtiofsd(mut child: Child, tx: Sender<Result<()>>) -> Result<()> {
    let stderr = child.stderr.as_mut().unwrap();
    let stderr_reader = BufReader::new(stderr);
    let mut lines = stderr_reader.lines();

    while let Some(buffer) = lines.next_line().await.context("read next line")? {
        let trim_buffer = buffer.trim_end();
        if !trim_buffer.is_empty() {
            debug!(sl!(), "source: virtiofsd {}", trim_buffer);
        }
        if buffer.contains("Waiting for vhost-user socket connection") {
            tx.send(Ok(())).await.unwrap();
        }
    }

    debug!(sl!(), "wait virtiofsd {:?}", child.wait().await);
    Ok(())
}
