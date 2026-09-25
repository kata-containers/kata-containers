// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

#[derive(Debug, Clone)]
pub enum SandboxEvent {
    Created(String),
    Removed(String),
}

pub struct SandboxWatcher {
    sandbox_path: PathBuf,
    tx: mpsc::Sender<SandboxEvent>,
}

impl SandboxWatcher {
    pub fn new(sandbox_path: &str, tx: mpsc::Sender<SandboxEvent>) -> Self {
        Self {
            sandbox_path: PathBuf::from(sandbox_path),
            tx,
        }
    }

    pub async fn start(&self) -> Result<()> {
        loop {
            if Path::new(&self.sandbox_path).exists() {
                break;
            }
            warn!(
                path = %self.sandbox_path.display(),
                "sandbox path not found, retry in 60s"
            );
            tokio::time::sleep(Duration::from_secs(60)).await;
        }

        info!(path = %self.sandbox_path.display(), "started fs monitoring");

        let mut backoff = Duration::from_secs(1);
        const MAX_BACKOFF: Duration = Duration::from_secs(60);

        loop {
            match self.run_watch_loop().await {
                Ok(()) => {
                    error!(
                        path = %self.sandbox_path.display(),
                        "watcher event channel closed unexpectedly, restarting"
                    );
                }
                Err(e) => {
                    error!(
                        path = %self.sandbox_path.display(),
                        error = %e,
                        "watcher failed, restarting in {:?}", backoff
                    );
                }
            }

            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(MAX_BACKOFF);
        }
    }

    async fn run_watch_loop(&self) -> Result<()> {
        let (notify_tx, mut notify_rx) = mpsc::channel::<Event>(256);
        let _watcher = self.create_watcher(notify_tx)?;

        // Initial sync: read existing sandbox directories
        let mut entries = tokio::fs::read_dir(&self.sandbox_path).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                let id = entry.file_name().to_string_lossy().to_string();
                if Self::is_valid_sandbox_id(&id) {
                    let _ = self.tx.send(SandboxEvent::Created(id)).await;
                }
            }
        }

        while let Some(event) = notify_rx.recv().await {
            match event.kind {
                EventKind::Create(_) => {
                    for path in &event.paths {
                        if let Some(id) = Self::extract_sandbox_id(path) {
                            let _ = self.tx.send(SandboxEvent::Created(id)).await;
                        }
                    }
                }
                EventKind::Remove(_) => {
                    for path in &event.paths {
                        if let Some(id) = Self::extract_sandbox_id(path) {
                            let _ = self.tx.send(SandboxEvent::Removed(id)).await;
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn create_watcher(&self, notify_tx: mpsc::Sender<Event>) -> Result<RecommendedWatcher> {
        let mut watcher = RecommendedWatcher::new(
            move |res: notify::Result<Event>| {
                if let Ok(event) = res {
                    let _ = notify_tx.blocking_send(event);
                }
            },
            notify::Config::default(),
        )?;
        watcher.watch(&self.sandbox_path, RecursiveMode::NonRecursive)?;
        Ok(watcher)
    }

    fn extract_sandbox_id(path: &Path) -> Option<String> {
        let name = path.file_name()?.to_string_lossy().to_string();
        if Self::is_valid_sandbox_id(&name) {
            Some(name)
        } else {
            None
        }
    }

    fn is_valid_sandbox_id(name: &str) -> bool {
        name.len() == 64 && name.chars().all(|c| c.is_ascii_hexdigit())
    }
}
