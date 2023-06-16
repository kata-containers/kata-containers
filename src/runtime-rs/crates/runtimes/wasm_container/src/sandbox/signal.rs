// Copyright (c) 2019-2020 Ant Financial
// Copyright (c) 2020 Intel Corporation
// Copyright (c) 2022-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::container_manager::container::container::Container;
use common::{error::Error, types::ContainerProcess};

use anyhow::{anyhow, Context, Result};
use capctl::prctl::set_subreaper;
use nix::{
    libc::pid_t,
    sys::wait::{self, WaitPidFlag, WaitStatus},
    unistd::Pid,
};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufRead, BufReader},
    sync::Arc,
};
use tokio::{
    select,
    signal::unix::{signal, SignalKind},
    sync::{watch::Receiver, RwLock},
};

async fn find_process_by_pid(
    containers: Arc<RwLock<HashMap<String, Container>>>,
    pid: pid_t,
) -> Option<ContainerProcess> {
    let containers = containers.read().await;
    for (_, c) in containers.iter() {
        if let Some(process) = c.find_process_by_pid(pid).await {
            return Some(process);
        }
    }

    None
}

async fn update_exited_process(
    containers: Arc<RwLock<HashMap<String, Container>>>,
    process_id: &ContainerProcess,
    exit_code: i32,
) -> Result<()> {
    let containers = containers.read().await;
    let container_id = &process_id.container_id.container_id;
    let c = containers
        .get(container_id)
        .ok_or_else(|| Error::ContainerNotFound(container_id.clone()))?;

    c.update_exited_process(process_id, exit_code).await
}

async fn handle_sigchild(
    wait_status: &WaitStatus,
    containers: Arc<RwLock<HashMap<String, Container>>>,
) -> Result<()> {
    let pid = wait_status
        .pid()
        .ok_or("no pid found")
        .map_err(|e| anyhow!(e))?
        .as_raw();

    let process = find_process_by_pid(containers.clone(), pid)
        .await
        .ok_or("no process found")
        .map_err(|e| anyhow!(e))?;

    let exit_code: i32 = match wait_status {
        WaitStatus::Exited(_, c) => c.clone(),
        WaitStatus::Signaled(_, sig, _) => sig.clone() as i32,
        _ => {
            return Err(anyhow!("wrong status for process {:?}", wait_status));
        }
    };

    update_exited_process(containers.clone(), &process, exit_code).await?;

    Ok(())
}

pub async fn signal_handler(
    containers: Arc<RwLock<HashMap<String, Container>>>,
    mut shutdown_rx: Receiver<bool>,
) -> Result<()> {
    let logger = sl!().new(o!("subsystem" => "signals"));

    set_subreaper(true)
        .map_err(|err| anyhow!(err).context("failed to setup manager as a child subreaper"))
        .unwrap();

    let mut signal_stream = signal(SignalKind::child())
        .context("failed to setup signal stream")
        .unwrap();

    loop {
        select! {
            _ = shutdown_rx.changed() => {
                info!(logger, "got shutdown request");
                break;
            }

            _ = signal_stream.recv() => {
                let wait_result = wait::waitpid(
                    Some(Pid::from_raw(-1)),
                    Some(WaitPidFlag::WNOHANG | WaitPidFlag::__WALL),
                );

                if let Ok(wait_status) = wait_result {
                    if wait_status != WaitStatus::StillAlive {
                        info!(logger, "wasm handling sigchild"; "wait_status" => format!("{:?}", wait_status));
                        let _ = handle_sigchild(&wait_status, containers.clone())
                            .await
                            .map_err(|e| {
                                error!(logger, "failed to handle sigchild {:?}", e);
                            });
                    }
                };
            }
        }
    }

    Ok(())
}

// Check if the container process installed the
// handler for specific signal.
pub fn is_signal_handled(proc_status_file: &str, signum: u32) -> bool {
    let shift_count: u64 = if signum == 0 {
        // signum 0 is used to check for process liveness.
        // Since that signal is not part of the mask in the file, we only need
        // to know if the file (and therefore) process exists to handle
        // that signal.
        return fs::metadata(proc_status_file).is_ok();
    } else if signum > 64 {
        // Ensure invalid signum won't break bit shift logic
        warn!(sl!(), "received invalid signum {}", signum);
        return false;
    } else {
        (signum - 1).into()
    };

    // Open the file in read-only mode (ignoring errors).
    let file = match File::open(proc_status_file) {
        Ok(f) => f,
        Err(_) => {
            warn!(sl!(), "failed to open file {}", proc_status_file);
            return false;
        }
    };

    let sig_mask: u64 = 1 << shift_count;
    let reader = BufReader::new(file);

    // read lines start with SigBlk/SigIgn/SigCgt and check any match the signal mask
    reader
        .lines()
        .flatten()
        .filter(|line| {
            line.starts_with("SigBlk:")
                || line.starts_with("SigIgn:")
                || line.starts_with("SigCgt:")
        })
        .any(|line| {
            let mask_vec: Vec<&str> = line.split(':').collect();
            if mask_vec.len() == 2 {
                let sig_str = mask_vec[1].trim();
                if let Ok(sig) = u64::from_str_radix(sig_str, 16) {
                    return sig & sig_mask == sig_mask;
                }
            }
            false
        })
}
