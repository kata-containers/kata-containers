// Copyright 2020 Ant Group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

//! Factory to create blob cache objects for blobs.
//!
//! The factory module provides methods to create
//! [blob cache objects](../cache/trait.BlobCache.html) for blobs. Internally it caches a group
//! of [BlobCacheMgr](../cache/trait.BlobCacheMgr.html) objects according to their
//! [FactoryConfig](../../api/http/struct.FactoryConfig.html). Those cached blob managers may be garbage-collected
//! by [BlobFactory::gc()](struct.BlobFactory.html#method.gc).
//! if not used anymore.
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::io::Result as IOResult;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lazy_static::lazy_static;
use nydus_api::http::{BackendConfig, FactoryConfig};
use tokio::{
    runtime::{Builder, Runtime},
    time,
};

#[cfg(feature = "backend-localfs")]
use crate::backend::localfs;
#[cfg(feature = "backend-oss")]
use crate::backend::oss;
#[cfg(feature = "backend-registry")]
use crate::backend::registry;
use crate::backend::BlobBackend;
use crate::cache::{BlobCache, BlobCacheMgr, DummyCacheMgr, FileCacheMgr, FsCacheMgr};
use crate::device::BlobInfo;

lazy_static! {
    pub static ref ASYNC_RUNTIME: Arc<Runtime> = {
        let runtime = Builder::new_multi_thread()
                .worker_threads(1) // Limit the number of worker thread to 1 since this runtime is generally used to do blocking IO.
                .thread_keep_alive(Duration::from_secs(10))
                .max_blocking_threads(8)
                .thread_name("cache-flusher")
                .enable_all()
                .build();
        match runtime {
            Ok(v) => Arc::new(v),
            Err(e) => panic!("failed to create tokio async runtime, {}", e),
        }
    };
}

#[derive(Eq, PartialEq)]
struct BlobCacheMgrKey {
    config: Arc<FactoryConfig>,
}

#[allow(clippy::derive_hash_xor_eq)]
impl Hash for BlobCacheMgrKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.config.id.hash(state);
        self.config.backend.backend_type.hash(state);
        self.config.cache.cache_type.hash(state);
        self.config.cache.prefetch_config.hash(state);
    }
}

lazy_static::lazy_static! {
    /// Default blob factory.
    pub static ref BLOB_FACTORY: BlobFactory = BlobFactory::new();
}

/// Factory to create blob cache for blob objects.
pub struct BlobFactory {
    mgrs: Mutex<HashMap<BlobCacheMgrKey, Arc<dyn BlobCacheMgr>>>,
    mgr_checker_active: AtomicBool,
}

impl BlobFactory {
    /// Create a new instance of blob factory object.
    pub fn new() -> Self {
        BlobFactory {
            mgrs: Mutex::new(HashMap::new()),
            mgr_checker_active: AtomicBool::new(false),
        }
    }

    pub fn start_mgr_checker(&self) {
        if self
            .mgr_checker_active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_err()
        {
            return;
        }
        ASYNC_RUNTIME.spawn(async {
            let mut interval = time::interval(Duration::from_secs(5));
            loop {
                interval.tick().await;
                BLOB_FACTORY.check_cache_stat();
            }
        });
    }

    /// Create a blob cache object for a blob with specified configuration.
    pub fn new_blob_cache(
        &self,
        config: &Arc<FactoryConfig>,
        blob_info: &Arc<BlobInfo>,
        blobs_need: usize,
    ) -> IOResult<Arc<dyn BlobCache>> {
        let key = BlobCacheMgrKey {
            config: config.clone(),
        };
        let mut guard = self.mgrs.lock().unwrap();
        // Use the existing blob cache manager if there's one with the same configuration.
        if let Some(mgr) = guard.get(&key) {
            return mgr.get_blob_cache(blob_info);
        }
        let backend = Self::new_backend(key.config.backend.clone(), blob_info.blob_id())?;
        let mgr = match key.config.cache.cache_type.as_str() {
            "blobcache" => {
                let mgr = FileCacheMgr::new(
                    config.cache.clone(),
                    backend,
                    ASYNC_RUNTIME.clone(),
                    &config.id,
                    blobs_need,
                )?;
                mgr.init()?;
                Arc::new(mgr) as Arc<dyn BlobCacheMgr>
            }
            "fscache" => {
                let mgr = FsCacheMgr::new(
                    config.cache.clone(),
                    backend,
                    ASYNC_RUNTIME.clone(),
                    &config.id,
                    blobs_need,
                )?;
                mgr.init()?;
                Arc::new(mgr) as Arc<dyn BlobCacheMgr>
            }
            _ => {
                let mgr = DummyCacheMgr::new(config.cache.clone(), backend, false)?;
                mgr.init()?;
                Arc::new(mgr) as Arc<dyn BlobCacheMgr>
            }
        };

        let mgr = guard.entry(key).or_insert_with(|| mgr);

        mgr.get_blob_cache(blob_info)
    }

    /// Garbage-collect unused blob cache managers and blob caches.
    pub fn gc(&self, victim: Option<(&Arc<FactoryConfig>, &str)>) {
        let mut mgrs = Vec::new();

        if let Some((config, id)) = victim {
            let key = BlobCacheMgrKey {
                config: config.clone(),
            };
            let mgr = self.mgrs.lock().unwrap().get(&key).cloned();
            if let Some(mgr) = mgr {
                if mgr.gc(Some(id)) {
                    mgrs.push((key, mgr.clone()));
                }
            }
        } else {
            for (key, mgr) in self.mgrs.lock().unwrap().iter() {
                if mgr.gc(None) {
                    mgrs.push((
                        BlobCacheMgrKey {
                            config: key.config.clone(),
                        },
                        mgr.clone(),
                    ));
                }
            }
        }

        for (key, mgr) in mgrs {
            let mut guard = self.mgrs.lock().unwrap();
            if mgr.gc(None) {
                guard.remove(&key);
            }
        }
    }

    /// Create a storage backend for the blob with id `blob_id`.
    #[allow(unused_variables)]
    pub fn new_backend(
        config: BackendConfig,
        blob_id: &str,
    ) -> IOResult<Arc<dyn BlobBackend + Send + Sync>> {
        match config.backend_type.as_str() {
            #[cfg(feature = "backend-oss")]
            "oss" => Ok(Arc::new(oss::Oss::new(
                config.backend_config,
                Some(blob_id),
            )?)),
            #[cfg(feature = "backend-registry")]
            "registry" => Ok(Arc::new(registry::Registry::new(
                config.backend_config,
                Some(blob_id),
            )?)),
            #[cfg(feature = "backend-localfs")]
            "localfs" => Ok(Arc::new(localfs::LocalFs::new(
                config.backend_config,
                Some(blob_id),
            )?)),
            _ => Err(einval!(format!(
                "unsupported backend type '{}'",
                config.backend_type
            ))),
        }
    }

    fn check_cache_stat(&self) {
        let mgrs = self.mgrs.lock().unwrap();
        for (_key, mgr) in mgrs.iter() {
            mgr.check_stat();
        }
    }
}

impl Default for BlobFactory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_config() {
        let config = BackendConfig {
            backend_type: "localfs".to_string(),
            backend_config: Default::default(),
        };
        let str_val = serde_json::to_string(&config).unwrap();
        let config2 = serde_json::from_str(&str_val).unwrap();

        assert_eq!(config, config2);
    }
}
