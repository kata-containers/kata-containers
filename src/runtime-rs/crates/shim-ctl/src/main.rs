// Copyright (c) 2022 Red Hat
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{Context, Result};
use common::{
    message::Message,
    types::{ContainerConfig, TaskRequest},
};
use runtimes::RuntimeHandlerManager;
use tokio::sync::mpsc::channel;

const MESSAGE_BUFFER_SIZE: usize = 8;
const WORKER_THREADS: usize = 2;

async fn real_main() {
    let (sender, _receiver) = channel::<Message>(MESSAGE_BUFFER_SIZE);
    let manager = RuntimeHandlerManager::new("xxx", sender).unwrap();

    let req = TaskRequest::CreateContainer(ContainerConfig {
        container_id: "xxx".to_owned(),
        bundle: ".".to_owned(),
        rootfs_mounts: Vec::new(),
        terminal: false,
        options: None,
        stdin: None,
        stdout: None,
        stderr: None,
    });

    manager.handler_task_message(req).await.ok();
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(WORKER_THREADS)
        .enable_all()
        .build()
        .context("prepare tokio runtime")?;

    runtime.block_on(real_main());

    Ok(())
}
