// Copyright (c) 2025 Edgeless Systems GmbH
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::registry::ImageLayer;

use fs2::FileExt;
use log::{debug, warn};
use std::fs::OpenOptions;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct ImageLayersCache {
    inner: Arc<Mutex<Vec<ImageLayer>>>,
    filename: Option<String>,
}

impl ImageLayersCache {
    pub fn new(layers_cache_file_path: &Option<String>) -> Self {
        let layers = match ImageLayersCache::try_new(layers_cache_file_path) {
            Ok(layers) => layers,
            Err(e) => {
                warn!("Could not read image layers cache: {e}");
                Vec::new()
            }
        };
        Self {
            inner: Arc::new(Mutex::new(layers)),
            filename: layers_cache_file_path.clone(),
        }
    }

    fn try_new(layers_cache_file_path: &Option<String>) -> std::io::Result<Vec<ImageLayer>> {
        match &layers_cache_file_path {
            Some(filename) => {
                let file = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .truncate(false)
                    .open(filename)?;
                // Using try_lock_shared allows this genpolicy instance to make progress even if another concurrent instance holds a lock.
                // In this case, the cache will simply not be used for this instance.
                FileExt::try_lock_shared(&file)?;

                let initial_state: Vec<ImageLayer> = match serde_json::from_reader(&file) {
                    Ok(data) => data,
                    Err(e) if e.is_eof() => Vec::new(), // empty file
                    Err(e) => {
                        FileExt::unlock(&file)?;
                        return Err(e.into());
                    }
                };
                FileExt::unlock(&file)?;
                Ok(initial_state)
            }
            None => Ok(Vec::new()),
        }
    }

    pub fn get_layer(&self, diff_id: &str) -> Option<ImageLayer> {
        let layers = self.inner.lock().unwrap();
        layers
            .iter()
            .find(|layer| layer.diff_id == diff_id)
            .cloned()
    }

    pub fn insert_layer(&self, layer: &ImageLayer) {
        let mut layers = self.inner.lock().unwrap();
        layers.push(layer.clone());
    }

    pub fn persist(&self) {
        if let Err(e) = self.try_persist() {
            warn!("Could not persist image layers cache: {e}");
        }
    }

    fn try_persist(&self) -> std::io::Result<()> {
        let Some(ref filename) = self.filename else {
            return Ok(());
        };
        debug!("Persisting image layers cache...");
        let layers = self.inner.lock().unwrap();
        let file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(filename)?;
        FileExt::try_lock_exclusive(&file)?;
        serde_json::to_writer_pretty(&file, &*layers)?;
        FileExt::unlock(&file)?;
        Ok(())
    }
}
