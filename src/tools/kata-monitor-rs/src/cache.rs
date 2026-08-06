// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashSet;
use std::sync::Arc;

use tokio::sync::RwLock;

pub struct SandboxCache {
    inner: RwLock<HashSet<String>>,
}

impl SandboxCache {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: RwLock::new(HashSet::new()),
        })
    }

    pub async fn insert(&self, id: String) -> bool {
        self.inner.write().await.insert(id)
    }

    pub async fn remove(&self, id: &str) -> bool {
        self.inner.write().await.remove(id)
    }

    pub async fn get_all(&self) -> Vec<String> {
        self.inner.read().await.iter().cloned().collect()
    }

    pub async fn count(&self) -> usize {
        self.inner.read().await.len()
    }
}

#[cfg(test)]
mod tests {}
