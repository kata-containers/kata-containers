// Copyright (c) 2025 Edgeless Systems GmbH
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::registry::ImageLayer;

use fs2::FileExt;
use log::{debug, warn};
use serde::de::DeserializeOwned;
use std::fs::{File, OpenOptions};
use std::io::{Error, ErrorKind};
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
        Self::try_new_with_schema(layers_cache_file_path)
    }

    fn try_new_with_schema<T>(layers_cache_file_path: &Option<String>) -> std::io::Result<Vec<T>>
    where
        T: DeserializeOwned,
    {
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

                let initial_state: Vec<T> = match serde_json::from_reader(&file) {
                    Ok(data) => data,
                    Err(e) if e.is_eof() => Vec::new(), // empty file
                    Err(e) => {
                        return Self::delete_incompatible_cache(filename, file, e.to_string());
                    }
                };

                FileExt::unlock(&file)?;
                Ok(initial_state)
            }
            None => Ok(Vec::new()),
        }
    }

    fn delete_incompatible_cache<T>(
        filename: &str,
        file: File,
        reason: String,
    ) -> std::io::Result<Vec<T>> {
        FileExt::unlock(&file)?;
        drop(file);
        std::fs::remove_file(filename)?;
        Err(Error::new(
            ErrorKind::InvalidData,
            format!("deleted incompatible image layers cache: {reason}"),
        ))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    const DIFF_ID: &str = "sha256:0123456789abcdef";

    #[test]
    fn persists_and_loads_current_schema() {
        let temp_dir = tempdir().unwrap();
        let cache_path = temp_dir.path().join("layers-cache.json");
        let filename = Some(cache_path.to_string_lossy().into_owned());
        let cache = ImageLayersCache::new(&filename);
        cache.insert_layer(&ImageLayer {
            diff_id: DIFF_ID.to_string(),
            passwd: "root:x:0:0:root:/root:/bin/sh".to_string(),
            group: "root:x:0:".to_string(),
        });

        cache.persist();

        let json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&cache_path).unwrap()).unwrap();
        assert_eq!(json[0].as_object().unwrap().len(), 3);
        assert!(json[0].get("unknown").is_none());

        let reloaded = ImageLayersCache::new(&filename);
        let layer = reloaded.get_layer(DIFF_ID).unwrap();
        assert_eq!(layer.diff_id, DIFF_ID);
    }

    #[derive(Debug, serde::Deserialize)]
    #[allow(dead_code)]
    struct ImageLayerWithUnknownField {
        diff_id: String,
        passwd: String,
        group: String,
        unknown: String,
    }

    #[test]
    fn deletes_current_cache_when_future_field_is_required() {
        let temp_dir = tempdir().unwrap();
        let cache_path = temp_dir.path().join("layers-cache.json");
        let current_cache = serde_json::json!([{
            "diff_id": DIFF_ID,
            "passwd": "root:x:0:0:root:/root:/bin/sh",
            "group": "root:x:0:"
        }]);
        fs::write(
            &cache_path,
            serde_json::to_vec_pretty(&current_cache).unwrap(),
        )
        .unwrap();

        let filename = Some(cache_path.to_string_lossy().into_owned());
        let result = ImageLayersCache::try_new_with_schema::<ImageLayerWithUnknownField>(&filename);

        assert_eq!(result.unwrap_err().kind(), ErrorKind::InvalidData);
        assert!(!cache_path.exists());
    }

    #[test]
    fn deletes_future_cache_with_unexpected_field() {
        let temp_dir = tempdir().unwrap();
        let cache_path = temp_dir.path().join("layers-cache.json");
        let future_cache = serde_json::json!([{
            "diff_id": DIFF_ID,
            "passwd": "root:x:0:0:root:/root:/bin/sh",
            "group": "root:x:0:",
            "unknown": "future-value"
        }]);
        fs::write(
            &cache_path,
            serde_json::to_vec_pretty(&future_cache).unwrap(),
        )
        .unwrap();

        let filename = Some(cache_path.to_string_lossy().into_owned());
        let cache = ImageLayersCache::new(&filename);

        assert!(!cache_path.exists());
        assert!(cache.get_layer(DIFF_ID).is_none());
    }

    #[test]
    fn deletes_cache_when_expected_field_is_renamed() {
        let temp_dir = tempdir().unwrap();
        let cache_path = temp_dir.path().join("layers-cache.json");
        let changed_cache = serde_json::json!([{
            "diff_id": DIFF_ID,
            "passwd": "root:x:0:0:root:/root:/bin/sh",
            "unknown": "root:x:0:"
        }]);
        fs::write(
            &cache_path,
            serde_json::to_vec_pretty(&changed_cache).unwrap(),
        )
        .unwrap();

        let filename = Some(cache_path.to_string_lossy().into_owned());
        let cache = ImageLayersCache::new(&filename);

        assert!(!cache_path.exists());
        assert!(cache.get_layer(DIFF_ID).is_none());
    }
}
