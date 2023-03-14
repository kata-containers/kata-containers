// Copyright 2019 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::error::{self, Result};
use log::debug;
use serde::Serialize;
use snafu::ResultExt;
use std::fs::{self, File};
use std::io::{ErrorKind, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};
use tempfile::TempDir;

#[derive(Debug, Clone)]
pub(crate) struct Datastore(Arc<RwLock<DatastorePath>>);

impl Datastore {
    pub(crate) fn new(path: Option<PathBuf>) -> Result<Self> {
        // using pattern matching instead of mapping because TempDir::new() can error
        Ok(Self(Arc::new(RwLock::new(match path {
            None => DatastorePath::TempDir(TempDir::new().context(error::DatastoreInitSnafu)?),
            Some(p) => DatastorePath::Path(p),
        }))))
    }

    // Because we are not actually changing the underlying data in the lock, we can ignore when a
    // lock is poisoned.

    fn read(&self) -> RwLockReadGuard<'_, DatastorePath> {
        self.0.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn write(&self) -> RwLockWriteGuard<'_, DatastorePath> {
        self.0.write().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn reader(&self, file: &str) -> Result<Option<impl Read>> {
        let path = self.read().path().join(file);
        match File::open(&path) {
            Ok(file) => Ok(Some(file)),
            Err(err) => match err.kind() {
                ErrorKind::NotFound => Ok(None),
                _ => Err(err).context(error::DatastoreOpenSnafu { path: &path }),
            },
        }
    }

    pub(crate) fn create<T: Serialize>(&self, file: &str, value: &T) -> Result<()> {
        let path = self.write().path().join(file);
        serde_json::to_writer_pretty(
            File::create(&path).context(error::DatastoreCreateSnafu { path: &path })?,
            value,
        )
        .context(error::DatastoreSerializeSnafu {
            what: format!("{} in datastore", file),
            path,
        })
    }

    pub(crate) fn remove(&self, file: &str) -> Result<()> {
        let path = self.write().path().join(file);
        debug!("removing '{}'", path.display());
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(err) => match err.kind() {
                ErrorKind::NotFound => Ok(()),
                _ => Err(err).context(error::DatastoreRemoveSnafu { path: &path }),
            },
        }
    }
}

/// Because `TempDir` is an RAII object, we need to hold on to it. This private enum allows us to
/// hold either a `TempDir` or a `PathBuf` depending on whether or not the user wants to manage the
/// directory.
#[derive(Debug)]
enum DatastorePath {
    /// Path to a user-managed directory.
    Path(PathBuf),
    /// A `TempDir` that we created on the user's behalf.
    TempDir(TempDir),
}

impl DatastorePath {
    /// Provides convenient access to the underlying filepath.
    fn path(&self) -> &Path {
        match self {
            DatastorePath::Path(p) => p,
            DatastorePath::TempDir(t) => t.path(),
        }
    }
}
