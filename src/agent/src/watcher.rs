// Copyright (c) 2021 Apple Inc.
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use tokio::fs;
use tokio::sync::Mutex;
use tokio::task;
use tokio::time::{self, Duration};

use anyhow::{ensure, Context, Result};
use async_recursion::async_recursion;
use nix::mount::{umount, MsFlags};
use slog::{debug, error, Logger};

use crate::mount::BareMount;
use crate::protocols::agent as protos;

/// The maximum number of file system entries agent will watch for each mount.
const MAX_ENTRIES_PER_STORAGE: usize = 8;

/// The maximum size of a watchable mount in bytes.
const MAX_SIZE_PER_WATCHABLE_MOUNT: u64 = 1024 * 1024;

/// How often to check for modified files.
const WATCH_INTERVAL_SECS: u64 = 2;

/// Destination path for tmpfs
const WATCH_MOUNT_POINT_PATH: &str = "/run/kata-containers/shared/containers/watchable/";

/// Represents a single watched storage entry which may have multiple files to watch.
#[derive(Default, Debug, Clone)]
struct Storage {
    /// A mount point without inotify capabilities.
    source_mount_point: PathBuf,

    /// The target mount point, where the watched files will be copied/mirrored
    /// when being changed, added or removed. This will be subdirectory of a tmpfs
    target_mount_point: PathBuf,

    /// Flag to indicate that the Storage should be watched. Storage will be watched until
    /// the source becomes too large, either in number of files (>8) or total size (>1MB).
    watch: bool,

    /// The list of files to watch from the source mount point and updated in the target one.
    watched_files: HashMap<PathBuf, SystemTime>,
}

impl Drop for Storage {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.target_mount_point);
    }
}

impl Storage {
    async fn new(storage: protos::Storage) -> Result<Storage> {
        let entry = Storage {
            source_mount_point: PathBuf::from(&storage.source),
            target_mount_point: PathBuf::from(&storage.mount_point),
            watch: true,
            watched_files: HashMap::new(),
        };

        Ok(entry)
    }

    async fn update_target(&self, logger: &Logger, source_path: impl AsRef<Path>) -> Result<()> {
        let source_file_path = source_path.as_ref();

        let dest_file_path = if self.source_mount_point.is_file() {
            // Simple file to file copy
            // Assume target mount is a file path
            self.target_mount_point.clone()
        } else {
            let dest_file_path = self.make_target_path(&source_file_path)?;

            if let Some(path) = dest_file_path.parent() {
                debug!(logger, "Creating destination directory: {}", path.display());
                fs::create_dir_all(path)
                    .await
                    .with_context(|| format!("Unable to mkdir all for {}", path.display()))?;
            }

            dest_file_path
        };

        debug!(
            logger,
            "Copy from {} to {}",
            source_file_path.display(),
            dest_file_path.display()
        );
        fs::copy(&source_file_path, &dest_file_path)
            .await
            .with_context(|| {
                format!(
                    "Copy from {} to {} failed",
                    source_file_path.display(),
                    dest_file_path.display()
                )
            })?;

        Ok(())
    }

    async fn scan(&mut self, logger: &Logger) -> Result<usize> {
        debug!(logger, "Scanning for changes");

        let mut remove_list = Vec::new();
        let mut updated_files: Vec<PathBuf> = Vec::new();

        // Remove deleted files for tracking list
        self.watched_files.retain(|st, _| {
            if st.exists() {
                true
            } else {
                remove_list.push(st.to_path_buf());
                false
            }
        });

        // Delete from target
        for path in remove_list {
            // File has been deleted, remove it from target mount
            let target = self.make_target_path(path)?;
            debug!(logger, "Removing file from mount: {}", target.display());
            let _ = fs::remove_file(target).await;
        }

        // Scan new & changed files
        self.scan_path(
            logger,
            self.source_mount_point.clone().as_path(),
            &mut updated_files,
        )
        .await
        .with_context(|| "Scan path failed")?;

        // Update identified files:
        for path in &updated_files {
            self.update_target(logger, path.as_path()).await?;
        }

        Ok(updated_files.len())
    }

    #[async_recursion]
    async fn scan_path(
        &mut self,
        logger: &Logger,
        path: &Path,
        update_list: &mut Vec<PathBuf>,
    ) -> Result<u64> {
        let mut size: u64 = 0;
        debug!(logger, "Scanning path: {}", path.display());

        if path.is_file() {
            let metadata = path
                .metadata()
                .with_context(|| format!("Failed to query metadata for: {}", path.display()))?;

            let modified = metadata
                .modified()
                .with_context(|| format!("Failed to get modified date for: {}", path.display()))?;

            size += metadata.len();

            ensure!(
                self.watched_files.len() <= MAX_ENTRIES_PER_STORAGE,
                "Too many file system entries to watch (must be < {})",
                MAX_ENTRIES_PER_STORAGE
            );

            // Insert will return old entry if any
            if let Some(old_st) = self.watched_files.insert(path.to_path_buf(), modified) {
                if modified > old_st {
                    update_list.push(PathBuf::from(&path))
                }
            } else {
                // Storage just added, copy to target
                debug!(logger, "New entry: {}", path.display());
                update_list.push(PathBuf::from(&path))
            }
        } else {
            // Scan dir recursively
            let mut entries = fs::read_dir(path)
                .await
                .with_context(|| format!("Failed to read dir: {}", path.display()))?;

            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                let res_size = self
                    .scan_path(logger, path.as_path(), update_list)
                    .await
                    .with_context(|| format!("Unable to scan inner path: {}", path.display()))?;
                size += res_size;
            }
        }
        ensure!(
            size <= MAX_SIZE_PER_WATCHABLE_MOUNT,
            "Too many file system entries to watch (must be < {})",
            MAX_SIZE_PER_WATCHABLE_MOUNT,
        );

        Ok(size)
    }

    fn make_target_path(&self, source_file_path: impl AsRef<Path>) -> Result<PathBuf> {
        let relative_path = source_file_path
            .as_ref()
            .strip_prefix(&self.source_mount_point)
            .with_context(|| {
                format!(
                    "Failed to strip prefix: {} - {}",
                    source_file_path.as_ref().display().to_string(),
                    &self.source_mount_point.display()
                )
            })?;

        let dest_file_path = Path::new(&self.target_mount_point).join(relative_path);
        Ok(dest_file_path)
    }
}

#[derive(Default, Debug)]
struct SandboxStorages(Vec<Storage>);

impl SandboxStorages {
    async fn add(
        &mut self,
        list: impl IntoIterator<Item = protos::Storage>,

        logger: &Logger,
    ) -> Result<()> {
        for storage in list.into_iter() {
            let entry = Storage::new(storage)
                .await
                .with_context(|| "Failed to add storage")?;
            self.0.push(entry);
        }

        // Perform initial copy
        self.check(logger)
            .await
            .with_context(|| "Failed to perform initial check")?;

        Ok(())
    }

    async fn check(&mut self, logger: &Logger) -> Result<()> {
        for entry in self.0.iter_mut().filter(|e| e.watch) {
            if let Err(e) = entry.scan(logger).await {
                // If an error was observed, we will stop treating this Storage as being watchable, and
                // instead clean up the target-mount files on the tmpfs and bind mount the source_mount_point
                // to target_mount_point.
                error!(logger, "error observed when watching: {:?}", e);
                entry.watch = false;

                // Remove destination contents, but not the directory itself, since this is
                // assumed to be bind-mounted into a container. If source/mount is a file, no need to cleanup
                if entry.target_mount_point.as_path().is_dir() {
                    for dir_entry in std::fs::read_dir(entry.target_mount_point.as_path())? {
                        let dir_entry = dir_entry?;
                        let path = dir_entry.path();
                        if dir_entry.file_type()?.is_dir() {
                            tokio::fs::remove_dir_all(path).await?;
                        } else {
                            tokio::fs::remove_file(path).await?;
                        }
                    }
                }

                //  - Create bind mount from source to destination
                BareMount::new(
                    entry.source_mount_point.to_str().unwrap(),
                    entry.target_mount_point.to_str().unwrap(),
                    "bind",
                    MsFlags::MS_BIND,
                    "bind",
                    logger,
                )
                .mount()?;
            }
        }
        Ok(())
    }
}

/// Handles watchable mounts. The watcher will manage one or more mounts for one or more containers. For each
/// mount that is added, the watcher will maintain a list of files to monitor, and periodically checks for new,
/// removed or changed (modified date) files. When a change is identified, the watcher will either copy the new
/// or updated file to a target mount point, or remove the removed file from the target mount point.  All WatchableStorage
/// target mount points are expected to reside within a single tmpfs, whose root is created by the BindWatcher.
///
/// This is a temporary workaround to handle config map updates until we get inotify on 9p/virtio-fs.
/// More context on this:
/// - https://github.com/kata-containers/runtime/issues/1505
/// - https://github.com/kata-containers/kata-containers/issues/1879
#[derive(Debug, Default)]
pub struct BindWatcher {
    /// Container ID -> Vec of watched entries
    sandbox_storages: Arc<Mutex<HashMap<String, SandboxStorages>>>,
    watch_thread: Option<task::JoinHandle<()>>,
}

impl Drop for BindWatcher {
    fn drop(&mut self) {
        self.cleanup();
    }
}

impl BindWatcher {
    pub fn new() -> BindWatcher {
        Default::default()
    }

    pub async fn add_container(
        &mut self,
        id: String,
        mounts: impl IntoIterator<Item = protos::Storage>,
        logger: &Logger,
    ) -> Result<()> {
        if self.watch_thread.is_none() {
            // Virtio-fs shared path is RO by default, so we back the target-mounts by tmpfs.
            self.mount(logger).await?;

            // Spawn background thread to monitor changes
            self.watch_thread = Some(Self::spawn_watcher(
                logger.clone(),
                Arc::clone(&self.sandbox_storages),
                WATCH_INTERVAL_SECS,
            ));
        }

        self.sandbox_storages
            .lock()
            .await
            .entry(id)
            .or_insert_with(SandboxStorages::default)
            .add(mounts, logger)
            .await
            .with_context(|| "Failed to add container")?;

        Ok(())
    }

    pub async fn remove_container(&self, id: &str) {
        self.sandbox_storages.lock().await.remove(id);
    }

    fn spawn_watcher(
        logger: Logger,
        sandbox_storages: Arc<Mutex<HashMap<String, SandboxStorages>>>,
        interval_secs: u64,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(interval_secs));

            loop {
                interval.tick().await;

                debug!(&logger, "Looking for changed files");
                for (_, entries) in sandbox_storages.lock().await.iter_mut() {
                    if let Err(err) = entries.check(&logger).await {
                        // We don't fail background loop, but rather log error instead.
                        error!(logger, "Check failed: {}", err);
                    }
                }
            }
        })
    }

    async fn mount(&self, logger: &Logger) -> Result<()> {
        fs::create_dir_all(WATCH_MOUNT_POINT_PATH).await?;

        BareMount::new(
            "tmpfs",
            WATCH_MOUNT_POINT_PATH,
            "tmpfs",
            MsFlags::empty(),
            "",
            logger,
        )
        .mount()?;

        Ok(())
    }

    fn cleanup(&mut self) {
        if let Some(handle) = self.watch_thread.take() {
            // Stop our background thread
            handle.abort();
        }

        let _ = umount(WATCH_MOUNT_POINT_PATH);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mount::is_mounted;
    use crate::skip_if_not_root;
    use std::fs;
    use std::thread;

    #[tokio::test]
    async fn watch_entries() {
        skip_if_not_root!();

        // If there's an error with an entry, let's make sure it is removed, and that the
        // mount-destination behaves like a standard bind-mount.

        // Create an entries vector with three storage objects: storage, storage1, storage2.
        // We'll first verify each are evaluated correctly, then increase the first entry's contents
        // so it fails mount size check (>1MB) (test handling for failure on mount that is a directory).
        // We'll then similarly cause failure with storage2 (test handling for failure on mount that is
        // a single file). We'll then verify that storage1 continues to be watchable.
        let source_dir = tempfile::tempdir().unwrap();
        let dest_dir = tempfile::tempdir().unwrap();

        let storage = protos::Storage {
            source: source_dir.path().display().to_string(),
            mount_point: dest_dir.path().display().to_string(),
            ..Default::default()
        };
        std::fs::File::create(source_dir.path().join("small.txt"))
            .unwrap()
            .set_len(10)
            .unwrap();

        let source_dir1 = tempfile::tempdir().unwrap();
        let dest_dir1 = tempfile::tempdir().unwrap();
        let storage1 = protos::Storage {
            source: source_dir1.path().display().to_string(),
            mount_point: dest_dir1.path().display().to_string(),
            ..Default::default()
        };
        std::fs::File::create(source_dir1.path().join("large.txt"))
            .unwrap()
            .set_len(MAX_SIZE_PER_WATCHABLE_MOUNT)
            .unwrap();

        // And finally, create a single file mount:
        let source_dir2 = tempfile::tempdir().unwrap();
        let dest_dir2 = tempfile::tempdir().unwrap();

        let source_path = source_dir2.path().join("mounted-file");
        let dest_path = dest_dir2.path().join("mounted-file");
        let mounted_file = std::fs::File::create(&source_path).unwrap();
        mounted_file.set_len(MAX_SIZE_PER_WATCHABLE_MOUNT).unwrap();

        let storage2 = protos::Storage {
            source: source_path.display().to_string(),
            mount_point: dest_path.display().to_string(),
            ..Default::default()
        };

        let logger = slog::Logger::root(slog::Discard, o!());

        let mut entries = SandboxStorages {
            ..Default::default()
        };

        entries
            .add(std::iter::once(storage), &logger)
            .await
            .unwrap();

        entries
            .add(std::iter::once(storage1), &logger)
            .await
            .unwrap();

        entries
            .add(std::iter::once(storage2), &logger)
            .await
            .unwrap();

        // Check that there are three entries, and that the
        // destination (mount point) matches what we expect for
        // the first:
        assert!(entries.check(&logger).await.is_ok());
        assert_eq!(entries.0.len(), 3);
        assert_eq!(std::fs::read_dir(dest_dir.path()).unwrap().count(), 1);

        // Add a second file which will trip file size check:
        std::fs::File::create(source_dir.path().join("big.txt"))
            .unwrap()
            .set_len(MAX_SIZE_PER_WATCHABLE_MOUNT)
            .unwrap();

        assert!(entries.check(&logger).await.is_ok());

        // Verify Storage 0 is no longer going to be watched:
        assert!(!entries.0[0].watch);

        // Verify that the directory has two entries:
        assert_eq!(std::fs::read_dir(dest_dir.path()).unwrap().count(), 2);

        // Verify that the directory is a bind mount. Add an entry without calling check,
        // and verify that the destination directory includes these files in the case of
        // mount that is no longer being watched (storage), but not within the still-being
        // watched (storage1):
        fs::write(source_dir.path().join("1.txt"), "updated").unwrap();
        fs::write(source_dir1.path().join("2.txt"), "updated").unwrap();

        assert_eq!(std::fs::read_dir(source_dir.path()).unwrap().count(), 3);
        assert_eq!(std::fs::read_dir(dest_dir.path()).unwrap().count(), 3);
        assert_eq!(std::fs::read_dir(source_dir1.path()).unwrap().count(), 2);
        assert_eq!(std::fs::read_dir(dest_dir1.path()).unwrap().count(), 1);

        // Verify that storage1 is still working. After running check, we expect that the number
        // of entries to increment
        assert!(entries.check(&logger).await.is_ok());
        assert_eq!(std::fs::read_dir(dest_dir1.path()).unwrap().count(), 2);

        // Break storage2 by increasing the file size
        mounted_file
            .set_len(MAX_SIZE_PER_WATCHABLE_MOUNT + 10)
            .unwrap();
        assert!(entries.check(&logger).await.is_ok());
        // Verify Storage 2 is no longer going to be watched:
        assert!(!entries.0[2].watch);

        // Verify bind mount is working -- let's write to the file and observe output:
        fs::write(&source_path, "updated").unwrap();
        assert_eq!(fs::read_to_string(&source_path).unwrap(), "updated");
    }

    #[tokio::test]
    async fn watch_directory_too_large() {
        let source_dir = tempfile::tempdir().unwrap();
        let dest_dir = tempfile::tempdir().unwrap();
        let mut entry = Storage::new(protos::Storage {
            source: source_dir.path().display().to_string(),
            mount_point: dest_dir.path().display().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

        let logger = slog::Logger::root(slog::Discard, o!());

        // Create a file that is too large:
        std::fs::File::create(source_dir.path().join("big.txt"))
            .unwrap()
            .set_len(MAX_SIZE_PER_WATCHABLE_MOUNT + 1)
            .unwrap();
        thread::sleep(Duration::from_secs(1));
        assert!(entry.scan(&logger).await.is_err());
        fs::remove_file(source_dir.path().join("big.txt")).unwrap();

        std::fs::File::create(source_dir.path().join("big.txt"))
            .unwrap()
            .set_len(MAX_SIZE_PER_WATCHABLE_MOUNT - 1)
            .unwrap();
        thread::sleep(Duration::from_secs(1));
        assert!(entry.scan(&logger).await.is_ok());

        std::fs::File::create(source_dir.path().join("too-big.txt"))
            .unwrap()
            .set_len(2)
            .unwrap();
        thread::sleep(Duration::from_secs(1));
        assert!(entry.scan(&logger).await.is_err());

        fs::remove_file(source_dir.path().join("big.txt")).unwrap();
        fs::remove_file(source_dir.path().join("too-big.txt")).unwrap();

        // Up to eight files should be okay:
        fs::write(source_dir.path().join("1.txt"), "updated").unwrap();
        fs::write(source_dir.path().join("2.txt"), "updated").unwrap();
        fs::write(source_dir.path().join("3.txt"), "updated").unwrap();
        fs::write(source_dir.path().join("4.txt"), "updated").unwrap();
        fs::write(source_dir.path().join("5.txt"), "updated").unwrap();
        fs::write(source_dir.path().join("6.txt"), "updated").unwrap();
        fs::write(source_dir.path().join("7.txt"), "updated").unwrap();
        fs::write(source_dir.path().join("8.txt"), "updated").unwrap();
        assert_eq!(entry.scan(&logger).await.unwrap(), 8);

        // Nine files is too many:
        fs::write(source_dir.path().join("9.txt"), "updated").unwrap();
        thread::sleep(Duration::from_secs(1));
        assert!(entry.scan(&logger).await.is_err());
    }

    #[tokio::test]
    async fn watch_directory() {
        // Prepare source directory:
        // ./tmp/1.txt
        // ./tmp/A/B/2.txt
        let source_dir = tempfile::tempdir().unwrap();
        fs::write(source_dir.path().join("1.txt"), "one").unwrap();
        fs::create_dir_all(source_dir.path().join("A/B")).unwrap();
        fs::write(source_dir.path().join("A/B/1.txt"), "two").unwrap();

        let dest_dir = tempfile::tempdir().unwrap();

        let mut entry = Storage::new(protos::Storage {
            source: source_dir.path().display().to_string(),
            mount_point: dest_dir.path().display().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

        let logger = slog::Logger::root(slog::Discard, o!());

        assert_eq!(entry.scan(&logger).await.unwrap(), 2);

        // Should copy no files since nothing is changed since last check
        assert_eq!(entry.scan(&logger).await.unwrap(), 0);

        // Should copy 1 file
        thread::sleep(Duration::from_secs(1));
        fs::write(source_dir.path().join("A/B/1.txt"), "updated").unwrap();
        assert_eq!(entry.scan(&logger).await.unwrap(), 1);
        assert_eq!(
            fs::read_to_string(dest_dir.path().join("A/B/1.txt")).unwrap(),
            "updated"
        );

        // Should copy no new files after copy happened
        assert_eq!(entry.scan(&logger).await.unwrap(), 0);

        // Update another file
        fs::write(source_dir.path().join("1.txt"), "updated").unwrap();
        assert_eq!(entry.scan(&logger).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn watch_file() {
        let source_dir = tempfile::tempdir().unwrap();
        let source_file = source_dir.path().join("1.txt");

        fs::write(&source_file, "one").unwrap();

        let dest_dir = tempfile::tempdir().unwrap();
        let dest_file = dest_dir.path().join("1.txt");

        let mut entry = Storage::new(protos::Storage {
            source: source_file.display().to_string(),
            mount_point: dest_file.display().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

        let logger = slog::Logger::root(slog::Discard, o!());

        assert_eq!(entry.scan(&logger).await.unwrap(), 1);

        thread::sleep(Duration::from_secs(1));
        fs::write(&source_file, "two").unwrap();
        assert_eq!(entry.scan(&logger).await.unwrap(), 1);
        assert_eq!(fs::read_to_string(&dest_file).unwrap(), "two");
        assert_eq!(entry.scan(&logger).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn delete_file() {
        let source_dir = tempfile::tempdir().unwrap();
        let source_file = source_dir.path().join("1.txt");
        fs::write(&source_file, "one").unwrap();

        let dest_dir = tempfile::tempdir().unwrap();
        let target_file = dest_dir.path().join("1.txt");

        let mut entry = Storage::new(protos::Storage {
            source: source_dir.path().display().to_string(),
            mount_point: dest_dir.path().display().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

        let logger = slog::Logger::root(slog::Discard, o!());

        assert_eq!(entry.scan(&logger).await.unwrap(), 1);
        assert_eq!(entry.watched_files.len(), 1);

        assert!(target_file.exists());
        assert!(entry.watched_files.contains_key(&source_file));

        // Remove source file
        fs::remove_file(&source_file).unwrap();

        assert_eq!(entry.scan(&logger).await.unwrap(), 0);

        assert_eq!(entry.watched_files.len(), 0);
        assert!(!target_file.exists());
    }

    #[tokio::test]
    async fn make_target_path() {
        let source_dir = tempfile::tempdir().unwrap();
        let target_dir = tempfile::tempdir().unwrap();

        let source_dir = source_dir.path();
        let target_dir = target_dir.path();

        let entry = Storage::new(protos::Storage {
            source: source_dir.display().to_string(),
            mount_point: target_dir.display().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

        assert_eq!(
            entry.make_target_path(source_dir.join("1.txt")).unwrap(),
            target_dir.join("1.txt")
        );

        assert_eq!(
            entry
                .make_target_path(source_dir.join("a/b/2.txt"))
                .unwrap(),
            target_dir.join("a/b/2.txt")
        );
    }

    #[tokio::test]
    async fn create_tmpfs() {
        skip_if_not_root!();

        let logger = slog::Logger::root(slog::Discard, o!());
        let mut watcher = BindWatcher::default();

        watcher.mount(&logger).await.unwrap();
        assert!(is_mounted(WATCH_MOUNT_POINT_PATH).unwrap());

        watcher.cleanup();
        assert!(!is_mounted(WATCH_MOUNT_POINT_PATH).unwrap());
    }

    #[tokio::test]
    async fn spawn_thread() {
        skip_if_not_root!();

        let source_dir = tempfile::tempdir().unwrap();
        fs::write(source_dir.path().join("1.txt"), "one").unwrap();

        let dest_dir = tempfile::tempdir().unwrap();

        let storage = protos::Storage {
            source: source_dir.path().display().to_string(),
            mount_point: dest_dir.path().display().to_string(),
            ..Default::default()
        };

        let logger = slog::Logger::root(slog::Discard, o!());
        let mut watcher = BindWatcher::default();

        watcher
            .add_container("test".into(), std::iter::once(storage), &logger)
            .await
            .unwrap();

        thread::sleep(Duration::from_secs(WATCH_INTERVAL_SECS));

        let out = fs::read_to_string(dest_dir.path().join("1.txt")).unwrap();
        assert_eq!(out, "one");
    }
}
