// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs::File,
    io::Read,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use agent::Agent;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hypervisor::device::device_manager::DeviceManager;
use inotify::{EventMask, Inotify, WatchMask};
use kata_sys_util::mount::{get_mount_options, get_mount_path, get_mount_type};
use nix::sys::stat::SFlag;
use rand::rng;
use rand::Rng;
use tokio::{
    io::AsyncReadExt,
    sync::{Mutex, RwLock},
    task::JoinHandle,
    time::Instant,
};
use walkdir::WalkDir;

use super::Volume;
use crate::share_fs::{MountedInfo, ShareFs, ShareFsVolumeConfig, DEFAULT_KATA_GUEST_SHARE_DIR};
use kata_types::{
    k8s::{is_configmap, is_downward_api, is_projected, is_secret},
    mount,
};
use oci_spec::runtime as oci;

const SYS_MOUNT_PREFIX: [&str; 2] = ["/proc", "/sys"];
const MONITOR_INTERVAL: Duration = Duration::from_millis(100);
const DEBOUNCE_TIME: Duration = Duration::from_millis(500);

// Corresponds to os.FileMode(0750) | os.ModeDir in Go
// So, it's (permission bits 0o750) ORed with (file type bit S_IFDIR).
// We use u32 here because `file_mode` in CopyFileRequest is u32
const DIR_MODE_PERMS: u32 = SFlag::S_IFDIR.bits() | 0o750;

// copy file to container's rootfs if filesystem sharing is not supported, otherwise
// bind mount it in the shared directory.
// Ignore /dev, directories and all other device files. We handle
// only regular files in /dev. It does not make sense to pass the host
// device nodes to the guest.
// skip the volumes whose source had already set to guest share dir.
pub(crate) struct ShareFsVolume {
    share_fs: Option<Arc<dyn ShareFs>>,
    mounts: Vec<oci::Mount>,
    storages: Vec<agent::Storage>,

    // Add volume manager reference
    volume_manager: Option<Arc<VolumeManager>>,
    // Record the source path for cleanup
    source_path: Option<String>,
    // Record the container ID
    container_id: String,
    // Retain release state so retries do not consume another container's reference.
    released: Mutex<HashMap<String, Release>>,
}

enum Release {
    Done,
    Umount(String),
}

/// Directory Monitor Config
/// path: the to be watched target directory
/// recursive: recursively monitor sub-dirs or not,
/// follow_symlinks: track symlinks or not,
/// exclude_hidden: exclude hidden files or not,
/// watch_events: Watcher Event types with CREATE/DELETE/MODIFY/MOVED_FROM/MOVED_TO
#[derive(Clone, Debug)]
struct MonitorConfig {
    path: PathBuf,
    recursive: bool,
    follow_symlinks: bool,
    exclude_hidden: bool,
    watch_events: WatchMask,
}

impl MonitorConfig {
    fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            recursive: true,
            follow_symlinks: false,
            exclude_hidden: true,
            watch_events: WatchMask::CREATE
                | WatchMask::DELETE
                | WatchMask::MODIFY
                | WatchMask::MOVED_FROM
                | WatchMask::MOVED_TO
                | WatchMask::CLOSE_WRITE,
        }
    }
}

#[derive(Clone)]
struct FsWatcher {
    config: MonitorConfig,
    inotify: Arc<Mutex<Inotify>>,
    watch_dirs: Arc<Mutex<HashSet<PathBuf>>>,
    pending_events: Arc<Mutex<HashSet<PathBuf>>>,
    need_sync: Arc<Mutex<bool>>,
}

impl FsWatcher {
    async fn new(source_path: &Path) -> Result<Self> {
        let inotify = Inotify::init()?;
        let mon_cfg = MonitorConfig::new(source_path);
        let mut watcher = Self {
            config: mon_cfg,
            inotify: Arc::new(Mutex::new(inotify)),
            pending_events: Arc::new(Mutex::new(HashSet::new())),
            watch_dirs: Arc::new(Mutex::new(HashSet::new())),
            need_sync: Arc::new(Mutex::new(false)),
        };

        watcher.add_watchers().await?;

        Ok(watcher)
    }

    /// add watched directory recursively
    async fn add_watchers(&mut self) -> Result<()> {
        let mut watched_dirs = self.watch_dirs.lock().await;
        let config: &MonitorConfig = &self.config;
        let walker = WalkDir::new(&config.path)
            .follow_links(config.follow_symlinks)
            .min_depth(0)
            .max_depth(if config.recursive { usize::MAX } else { 1 })
            .into_iter()
            .filter_entry(|e| {
                !(config.exclude_hidden
                    && e.file_name()
                        .to_str()
                        .map(|s| s.starts_with('.'))
                        .unwrap_or(false))
            });

        for entry in walker.filter_map(|e| e.ok()) {
            if entry.file_type().is_dir() {
                let path = entry.path();
                if watched_dirs.insert(path.to_path_buf()) {
                    self.inotify
                        .lock()
                        .await
                        .watches()
                        .add(path, config.watch_events)?; // we don't use WatchMask::ALL_EVENTS
                }
            }
        }

        Ok(())
    }

    /// start monitor
    pub async fn start_monitor(
        &self,
        agent: Arc<dyn Agent>,
        src: PathBuf,
        dst: PathBuf,
    ) -> JoinHandle<()> {
        let need_sync = self.need_sync.clone();
        let pending_events = self.pending_events.clone();
        let inotify = self.inotify.clone();
        let monitor_config = self.config.clone();

        // Perform a full sync before starting monitoring to ensure that files which exist before monitoring starts are also synced.
        let agent_sync = agent.clone();
        let src_sync = src.clone();
        let dst_sync = dst.clone();

        tokio::spawn(async move {
            let mut buffer = [0u8; 4096];
            let mut last_event_time = None;

            // Initial sync: ensure existing contents in the directory are synchronized
            {
                info!(
                    sl!(),
                    "Initial sync from {:?} to {:?}", &src_sync, &dst_sync
                );
                if let Err(e) =
                    copy_dir_recursively(&src_sync, &dst_sync.to_string_lossy(), &agent_sync).await
                {
                    error!(sl!(), "Initial sync failed: {:?}", e);
                }
            }

            loop {
                // use cloned inotify instance
                match inotify.lock().await.read_events(&mut buffer) {
                    Ok(events) => {
                        for event in events {
                            if !event.mask.intersects(
                                EventMask::CREATE
                                    | EventMask::MODIFY
                                    | EventMask::DELETE
                                    | EventMask::MOVED_FROM
                                    | EventMask::MOVED_TO
                                    | EventMask::CLOSE_WRITE,
                            ) {
                                continue;
                            }

                            if let Some(file_name) = event.name {
                                let full_path = &monitor_config.path.join(file_name);
                                let event_types: Vec<&str> = event
                                    .mask
                                    .iter()
                                    .map(|m| match m {
                                        EventMask::CREATE => "CREATE",
                                        EventMask::DELETE => "DELETE",
                                        EventMask::MODIFY => "MODIFY",
                                        EventMask::MOVED_FROM => "MOVED_FROM",
                                        EventMask::MOVED_TO => "MOVED_TO",
                                        EventMask::CLOSE_WRITE => "CLOSE_WRITE",
                                        _ => "OTHER",
                                    })
                                    .collect();

                                info!(
                                    sl!(),
                                    "handle events [{}] {:?} -> {:?}",
                                    event_types.join("|"),
                                    event.mask,
                                    full_path
                                );
                                pending_events.lock().await.insert(full_path.clone());
                            }
                        }
                    }
                    Err(e) => eprintln!("inotify error: {e}"),
                }

                // handle events to be synchronized
                let events_paths = {
                    let mut pending = pending_events.lock().await;
                    pending.drain().collect::<Vec<_>>()
                };
                if !events_paths.is_empty() {
                    *need_sync.lock().await = true;
                    last_event_time = Some(Instant::now());
                }

                // Debounce handling
                // It is used to prevent unnecessary repeated copies when file changes are triggered
                // multiple times in a short period; we only execute the last one.
                if let Some(t) = last_event_time {
                    if Instant::now().duration_since(t) > DEBOUNCE_TIME && *need_sync.lock().await {
                        info!(sl!(), "debounce handle copyfile {:?} -> {:?}", &src, &dst);
                        if let Err(e) =
                            copy_dir_recursively(&src, &dst.to_string_lossy(), &agent).await
                        {
                            error!(
                                sl!(),
                                "debounce handle copyfile {:?} -> {:?} failed with error: {:?}",
                                &src,
                                &dst,
                                e
                            );
                            eprintln!("sync host/guest files failed: {e}");
                        }
                        *need_sync.lock().await = false;
                        last_event_time = None;
                    }
                }

                tokio::time::sleep(MONITOR_INTERVAL).await;
            }
        })
    }
}

//==========volume manager==============
/// Sandbox-level volume state manager
/// Tracks which paths have been copied to the guest on the runtime side
#[derive(Clone, Default)]
pub struct VolumeManager {
    // Mapping of source path -> volume state
    volume_states: Arc<RwLock<HashMap<String, VolumeState>>>,
}

#[derive(Clone, Debug, Default)]
struct VolumeState {
    // Source path (on the host)
    source_path: String,
    // Guest path
    guest_path: String,
    // Reference count (how many containers are using it)
    ref_count: usize,
    // List of container IDs using this volume
    containers: HashSet<String>,
    // Monitor task handle (if any)
    monitor_task: Option<Arc<JoinHandle<()>>>,
}

#[allow(dead_code)]
impl VolumeManager {
    pub fn new() -> Self {
        Self {
            volume_states: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Gets or creates the volume's guest path
    pub async fn get_or_create_volume(
        &self,
        canonical_source: &str,
        container_id: &str,
        mount_destination: &Path,
    ) -> Result<String> {
        let mut states = self.volume_states.write().await;

        if let Some(state) = states.get_mut(canonical_source) {
            // Existing volume and update reference
            state.ref_count += 1;
            state.containers.insert(container_id.to_string());

            info!(
                sl!(),
                "Existing volume: source={:?}, guest={:?}, ref_count={}",
                canonical_source,
                state.guest_path,
                state.ref_count,
            );
        }

        // Create a new volume state
        let guest_path = generate_copy_file_guest_path(container_id, mount_destination)
            .context("generate path failed")?;

        let mut containers = HashSet::new();
        containers.insert(container_id.to_string());

        let state = VolumeState {
            source_path: canonical_source.to_string(),
            guest_path: guest_path.clone(),
            ref_count: 1,
            containers,
            monitor_task: None,
        };

        states.insert(state.source_path.clone(), state.clone());

        info!(
            sl!(),
            "Created new volume state: source={:?}, guest={:?}",
            state.source_path,
            state.guest_path,
        );

        // Return guest path
        Ok(guest_path)
    }

    /// Register monitor task into the volume manager
    pub async fn register_monitor(
        &self,
        canonical_source: &str,
        monitor_task: Option<JoinHandle<()>>,
    ) -> Result<()> {
        let mut states = self.volume_states.write().await;

        if let Some(state) = states.get_mut(canonical_source) {
            if let Some(handle) = monitor_task {
                state.monitor_task = Some(Arc::new(handle));
            }
        }

        Ok(())
    }

    /// Releases a volume reference
    pub async fn release_volume(&self, source_path: &str, container_id: &str) -> Result<bool> {
        let mut states = self.volume_states.write().await;

        let canonical_source = std::fs::canonicalize(source_path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| source_path.to_string());

        if let Some(state) = states.get_mut(&canonical_source) {
            state.containers.remove(container_id);
            state.ref_count = state.ref_count.saturating_sub(1);

            if state.ref_count == 0 {
                // Abort the monitor task
                if let Some(handle) = &state.monitor_task {
                    handle.abort();
                }

                info!(
                    sl!(),
                    "Volume has no more references, source={:?}, guest={:?}",
                    canonical_source,
                    state.guest_path
                );

                return Ok(true); // Can be cleaned up
            }
        }

        Ok(false)
    }
}

impl ShareFsVolume {
    pub(crate) async fn new(
        share_fs: &Option<Arc<dyn ShareFs>>,
        m: &oci::Mount,
        cid: &str,
        readonly: bool,
        agent: Arc<dyn Agent>,
        volume_manager: Arc<VolumeManager>,
    ) -> Result<Self> {
        // The file_name is in the format of "sandbox-{uuid}-{file_name}"
        let source_path = get_mount_path(m.source());
        let file_name = Path::new(&source_path)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        let file_name = generate_mount_path("sandbox", file_name);

        let mut volume = Self {
            share_fs: share_fs.as_ref().map(Arc::clone),
            mounts: vec![],
            storages: vec![],
            volume_manager: Some(volume_manager.clone()),
            source_path: Some(source_path.clone()),
            container_id: cid.to_string(),
            released: Mutex::new(HashMap::new()),
        };

        match share_fs {
            None => {
                let src = match std::fs::canonicalize(&source_path) {
                    Err(err) => {
                        return Err(anyhow!(format!(
                            "failed to canonicalize file {} {:?}",
                            source_path, err
                        )))
                    }
                    Ok(src) => src,
                };

                // append oci::Mount structure to volume mounts
                let mut oci_mount = oci::Mount::default();
                oci_mount.set_destination(m.destination().clone());
                oci_mount.set_typ(Some("bind".to_string()));
                oci_mount.set_options(m.options().clone());

                // If the mount source is a file, we can copy it to the sandbox
                if src.is_file() {
                    // Generate guest path
                    let guest_path = generate_copy_file_guest_path(cid, m.destination())
                        .context("generate path failed")?;
                    // Copy a single file
                    Self::copy_file_to_guest(&src, &guest_path, &agent)
                        .await
                        .context("copy file to guest")?;

                    oci_mount.set_source(Some(PathBuf::from(&guest_path)));
                    volume.mounts.push(oci_mount);
                } else if src.is_dir() {
                    // We allow directory copying wildly
                    // source path: "/var/lib/kubelet/pods/6dad7281-57ff-49e4-b844-c588ceabec16/volumes/kubernetes.io~projected/kube-api-access-8s2nl"
                    info!(sl!(), "copying directory {:?} to guest", &src);

                    // Get or create the guest path
                    let guest_path = volume_manager
                        .get_or_create_volume(&src.to_string_lossy(), cid, m.destination())
                        .await
                        .context("get or create volume")?;

                    // Create directory
                    Self::copy_directory_to_guest(&src, &guest_path, &agent)
                        .await
                        .context("copy directory to guest")?;

                    oci_mount.set_source(Some(PathBuf::from(&guest_path)));
                    volume.mounts.push(oci_mount);

                    // Start monitoring (only for watchable volumes)
                    let mut monitor_task = None;
                    if is_watchable_volume(&src) {
                        let watcher = FsWatcher::new(&src).await?;
                        let handle = watcher
                            .start_monitor(agent.clone(), src.clone(), PathBuf::from(&guest_path))
                            .await;
                        monitor_task = Some(handle);
                    }

                    // Register monitor into Volume Manager
                    volume_manager
                        .register_monitor(&src.to_string_lossy(), monitor_task)
                        .await?;
                } else {
                    // If not, we can ignore it. Let's issue a warning so that the user knows.
                    warn!(
                        sl!(),
                        "Ignoring non-regular file as FS sharing not supported. mount: {:?}", m
                    );
                }
            }
            Some(share_fs) => {
                let share_fs_mount = share_fs.get_share_fs_mount();
                let mounted_info_set = share_fs.mounted_info_set();
                let mut mounted_info_set = mounted_info_set.lock().await;
                if let Some(mut mounted_info) = mounted_info_set.get(&source_path).cloned() {
                    // Mounted at least once
                    let guest_path = mounted_info
                        .guest_path
                        .clone()
                        .as_os_str()
                        .to_str()
                        .unwrap()
                        .to_owned();
                    if !readonly && mounted_info.readonly() {
                        // The current mount should be upgraded to readwrite permission
                        info!(
                            sl!(),
                            "The mount will be upgraded, mount = {:?}, cid = {}", m, cid
                        );
                        share_fs_mount
                            .upgrade_to_rw(
                                &mounted_info
                                    .file_name()
                                    .context("get name of mounted info")?,
                            )
                            .await
                            .context("upgrade mount")?;
                    }
                    if readonly {
                        mounted_info.ro_ref_count += 1;
                    } else {
                        mounted_info.rw_ref_count += 1;
                    }
                    mounted_info_set.insert(source_path.clone(), mounted_info);

                    let mut oci_mount = oci::Mount::default();
                    oci_mount.set_destination(m.destination().clone());
                    oci_mount.set_typ(Some("bind".to_string()));
                    oci_mount.set_source(Some(PathBuf::from(&guest_path)));
                    oci_mount.set_options(m.options().clone());

                    volume.mounts.push(oci_mount);
                } else {
                    // Not mounted ever
                    let mount_result = share_fs_mount
                        .share_volume(&ShareFsVolumeConfig {
                            // The scope of shared volume is sandbox
                            cid: String::from(""),
                            source: source_path.clone(),
                            target: file_name.clone(),
                            readonly,
                            mount_options: get_mount_options(m.options()).clone(),
                            mount: m.clone(),
                            is_rafs: false,
                        })
                        .await
                        .context("mount shared volume")?;
                    let mounted_info = MountedInfo::new(
                        PathBuf::from_str(&mount_result.guest_path)
                            .context("convert guest path")?,
                        readonly,
                    );
                    mounted_info_set.insert(source_path.clone(), mounted_info);
                    // set storages for the volume
                    volume.storages = mount_result.storages;

                    // set mount for the volume
                    let mut oci_mount = oci::Mount::default();
                    oci_mount.set_destination(m.destination().clone());
                    oci_mount.set_typ(Some("bind".to_string()));
                    oci_mount.set_source(Some(PathBuf::from(&mount_result.guest_path)));
                    oci_mount.set_options(m.options().clone());

                    volume.mounts.push(oci_mount);
                }
            }
        }
        Ok(volume)
    }

    async fn copy_file_to_guest(
        src: &Path,
        guest_path: &str,
        agent: &Arc<dyn Agent>,
    ) -> Result<()> {
        // Read file metadata
        let file_metadata = std::fs::metadata(src)
            .with_context(|| format!("Failed to read metadata from file: {src:?}"))?;

        // Open file
        let mut file = File::open(src).with_context(|| format!("Failed to open file: {src:?}"))?;

        // Open read file contents to buffer
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .with_context(|| format!("Failed to read file: {src:?}"))?;

        // Create gRPC request
        let r = agent::CopyFileRequest {
            path: guest_path.to_owned(),
            file_size: file_metadata.len() as i64,
            uid: file_metadata.uid() as i32,
            gid: file_metadata.gid() as i32,
            file_mode: file_metadata.mode(),
            data: buffer,
            ..Default::default()
        };

        debug!(sl!(), "copy_file: {:?} to sandbox {:?}", &src, guest_path);

        // Issue gRPC request to agent
        agent.copy_file(r).await.with_context(|| {
            format!("copy file request failed: src: {src:?}, dest: {guest_path:?}")
        })?;
        Ok(())
    }

    async fn copy_directory_to_guest(
        src: &Path,
        guest_path: &str,
        agent: &Arc<dyn Agent>,
    ) -> Result<()> {
        // create directory
        let dir_metadata =
            std::fs::metadata(src).context(format!("read metadata from directory: {src:?}"))?;

        // ttRPC request for creating directory
        let dir_request = agent::CopyFileRequest {
            path: guest_path.to_owned(),
            file_size: 0, // useless for dir
            uid: dir_metadata.uid() as i32,
            gid: dir_metadata.gid() as i32,
            dir_mode: DIR_MODE_PERMS,
            file_mode: dir_metadata.mode(),
            data: vec![], // no files
            ..Default::default()
        };

        info!(
            sl!(),
            "creating directory: {:?} in sandbox with file_mode: {:?}",
            guest_path,
            dir_request.file_mode
        );

        // send request for creating directory
        agent
            .copy_file(dir_request)
            .await
            .context(format!("create directory in sandbox: {guest_path:?}"))?;

        // recursively copy files from this directory
        // similar to `scp -r $source_dir $target_dir`
        copy_dir_recursively(src, guest_path, agent)
            .await
            .context(format!("failed to copy directory contents: {src:?}"))?;

        Ok(())
    }
}

#[async_trait]
impl Volume for ShareFsVolume {
    fn get_volume_mount(&self) -> anyhow::Result<Vec<oci::Mount>> {
        Ok(self.mounts.clone())
    }

    fn get_storage(&self) -> Result<Vec<agent::Storage>> {
        Ok(self.storages.clone())
    }

    async fn cleanup(&self, _device_manager: &RwLock<DeviceManager>) -> Result<()> {
        let share_fs = match self.share_fs.as_ref() {
            Some(fs) => fs,
            None => {
                return {
                    // Release volume reference
                    if let (Some(manager), Some(source)) = (&self.volume_manager, &self.source_path)
                    {
                        let should_cleanup =
                            manager.release_volume(source, &self.container_id).await?;

                        if should_cleanup {
                            info!(
                                sl!(),
                                "Volume {:?} has no more references, can be cleaned up", source
                            );
                            // NOTE: We cannot delete files from the guest because there is no corresponding API
                            // Files will be cleaned up automatically when the sandbox is destroyed
                        }
                    }
                    Ok(())
                };
            }
        };

        let share_fs_mount = share_fs.get_share_fs_mount();
        let mounted_info_set = share_fs.mounted_info_set();
        let mut mounted_info_set = mounted_info_set.lock().await;
        let mut released = self.released.lock().await;
        for m in self.mounts.iter() {
            let guest_path = get_mount_path(m.source());
            match released.get(&guest_path) {
                Some(Release::Done) => continue,
                Some(Release::Umount(file_name)) => {
                    share_fs_mount
                        .umount_volume(file_name)
                        .await
                        .context("Umount volume")?;
                    released.insert(guest_path, Release::Done);
                    continue;
                }
                None => {}
            }

            let (host_source, mut mounted_info) = match mounted_info_set
                .iter()
                .find(|entry| entry.1.guest_path.as_os_str().to_str().unwrap() == guest_path)
                .map(|entry| (entry.0.to_owned(), entry.1.clone()))
            {
                Some(entry) => entry,
                None => {
                    warn!(
                        sl!(),
                        "The mounted info for guest path {} not found", &guest_path
                    );
                    continue;
                }
            };

            let old_readonly = mounted_info.readonly();
            if get_mount_options(m.options()).contains(&"ro".to_owned()) {
                mounted_info.ro_ref_count -= 1;
            } else {
                mounted_info.rw_ref_count -= 1;
            }

            debug!(
                sl!(),
                "Ref count for {} was updated to {} due to volume cleanup",
                host_source,
                mounted_info.ref_count()
            );
            let file_name = mounted_info.file_name()?;

            if mounted_info.ref_count() > 0 {
                // Downgrade to readonly if no container needs readwrite permission
                if !old_readonly && mounted_info.readonly() {
                    info!(sl!(), "Downgrade {} to readonly due to no container that needs readwrite permission", host_source);
                    share_fs_mount
                        .downgrade_to_ro(&file_name)
                        .await
                        .context("Downgrade volume")?;
                }
                mounted_info_set.insert(host_source.clone(), mounted_info);
                released.insert(guest_path, Release::Done);
            } else {
                info!(
                    sl!(),
                    "The path will be umounted due to no references, host_source = {}", host_source
                );
                // Remove the entry before awaiting unmount so new users get a fresh share.
                mounted_info_set.remove(&host_source);
                released.insert(guest_path.clone(), Release::Umount(file_name.clone()));
                // Umount the volume
                share_fs_mount
                    .umount_volume(&file_name)
                    .await
                    .context("Umount volume")?;
                released.insert(guest_path, Release::Done);
            }
        }

        Ok(())
    }

    fn get_device_id(&self) -> Result<Option<String>> {
        Ok(None)
    }
}

#[allow(dead_code)]
async fn copy_dir_recursively<P: AsRef<Path>>(
    src_dir: P,
    dest_dir: &str,
    agent: &Arc<dyn Agent>,
) -> Result<()> {
    let mut queue = VecDeque::new();
    queue.push_back((src_dir.as_ref().to_path_buf(), dest_dir.to_string()));

    while let Some((current_src, current_dest)) = queue.pop_front() {
        let mut entries = tokio::fs::read_dir(&current_src)
            .await
            .context(format!("read directory: {current_src:?}"))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .context(format!("read directory entry in {current_src:?}"))?
        {
            let entry_path = entry.path();
            let file_name = entry_path
                .file_name()
                .ok_or_else(|| anyhow!("get file name for {:?}", entry_path))?
                .to_string_lossy()
                .to_string();

            let dest_path = format!("{current_dest}/{file_name}");

            let metadata = entry
                .metadata()
                .await
                .context(format!("read metadata for {entry_path:?}"))?;

            if metadata.is_symlink() {
                // handle symlinks
                let entry_path_err = entry_path.clone();
                let entry_path_clone = entry_path.clone();
                let link_target =
                    tokio::task::spawn_blocking(move || std::fs::read_link(&entry_path_clone))
                        .await
                        .context(format!(
                            "failed to spawn blocking task for symlink: {entry_path_err:?}"
                        ))??;

                let link_target_str = link_target.to_string_lossy().into_owned();
                let symlink_request = agent::CopyFileRequest {
                    path: dest_path.clone(),
                    file_size: link_target_str.len() as i64,
                    uid: metadata.uid() as i32,
                    gid: metadata.gid() as i32,
                    file_mode: SFlag::S_IFLNK.bits(),
                    data: link_target_str.clone().into_bytes(),
                    ..Default::default()
                };
                info!(
                    sl!(),
                    "copying symlink_request {:?} in sandbox with file_mode: {:?}",
                    dest_path.clone(),
                    symlink_request.file_mode
                );

                agent.copy_file(symlink_request).await.context(format!(
                    "failed to create symlink: {dest_path:?} -> {link_target_str:?}"
                ))?;
            } else if metadata.is_dir() {
                // handle directory
                let dir_request = agent::CopyFileRequest {
                    path: dest_path.clone(),
                    file_size: 0,
                    uid: metadata.uid() as i32,
                    gid: metadata.gid() as i32,
                    dir_mode: metadata.mode(),
                    file_mode: metadata.mode(),
                    data: vec![],
                    ..Default::default()
                };
                info!(
                    sl!(),
                    "copying subdirectory {:?} in sandbox with file_mode: {:?}",
                    dir_request.path,
                    dir_request.file_mode
                );
                agent
                    .copy_file(dir_request)
                    .await
                    .context(format!("Failed to create subdirectory: {dest_path:?}"))?;

                // push back the sub-dir into queue to handle it in time
                queue.push_back((entry_path, dest_path));
            } else if metadata.is_file() {
                // async read file
                let mut file = tokio::fs::File::open(&entry_path)
                    .await
                    .context(format!("open file: {entry_path:?}"))?;

                let mut buffer = Vec::new();
                file.read_to_end(&mut buffer)
                    .await
                    .context(format!("read file: {entry_path:?}"))?;

                let file_request = agent::CopyFileRequest {
                    path: dest_path.clone(),
                    file_size: metadata.len() as i64,
                    uid: metadata.uid() as i32,
                    gid: metadata.gid() as i32,
                    file_mode: metadata.mode(),
                    data: buffer,
                    ..Default::default()
                };

                info!(sl!(), "copy file {:?} to guest", dest_path.clone());
                agent
                    .copy_file(file_request)
                    .await
                    .context(format!("copy file: {entry_path:?} -> {dest_path:?}"))?;
            }
        }
    }

    Ok(())
}

pub(crate) fn is_share_fs_volume(m: &oci::Mount) -> bool {
    let mount_type = get_mount_type(m);
    (mount_type == "bind" || mount_type == mount::KATA_EPHEMERAL_VOLUME_TYPE)
        && !is_host_device(&get_mount_path(&Some(m.destination().clone())))
        && !is_system_mount(&get_mount_path(m.source()))
}

fn is_host_device(dest: &str) -> bool {
    if dest == "/dev" {
        return true;
    }

    if dest.starts_with("/dev/") {
        let src = match std::fs::canonicalize(dest) {
            Err(_) => return false,
            Ok(src) => src,
        };

        if src.is_file() {
            return false;
        }

        return true;
    }

    false
}

// Skip mounting certain system paths("/sys/*", "/proc/*")
// from source on the host side into the container as it does not
// make sense to do so.
// Agent will support this kind of bind mount.
fn is_system_mount(src: &str) -> bool {
    for p in SYS_MOUNT_PREFIX {
        let sub_dir_p = format!("{p}/");
        if src == p || src.contains(sub_dir_p.as_str()) {
            return true;
        }
    }
    false
}

// Note, don't generate random name, attaching rafs depends on the predictable name.
pub fn generate_mount_path(id: &str, file_name: &str) -> String {
    let mut nid = String::from(id);
    if nid.len() > 10 {
        nid = nid.chars().take(10).collect();
    }

    let mut uid = uuid::Uuid::new_v4().to_string();
    let uid_vec: Vec<&str> = uid.splitn(2, '-').collect();
    uid = String::from(uid_vec[0]);

    format!("{nid}-{uid}-{file_name}")
}

/// This function is used to check whether a given volume is a watchable volume.
/// More specifically, it determines whether the volume's path is located under
/// a predefined list of allowed copy directories.
pub(crate) fn is_watchable_volume(source_path: &PathBuf) -> bool {
    if !source_path.is_dir() {
        return false;
    }
    // watchable list: { kubernetes.io~projected, kubernetes.io~configmap, kubernetes.io~secret, kubernetes.io~downward-api }
    is_projected(source_path)
        || is_downward_api(source_path)
        || is_secret(source_path)
        || is_configmap(source_path)
}

/// Generates a destination for an agent CopyFile request.
///
/// CopyFile writes into the guest filesystem and confines destinations to the
/// fixed guest shared directory. Unlike filesystem-sharing paths, this guest
/// path must not be adjusted when rootless mode is enabled.
fn generate_copy_file_guest_path(cid: &str, mount_destination: &Path) -> Result<String> {
    let mut data = vec![0u8; 8];
    let mut rng = rng(); // Get a thread-local RNG
    rng.fill_bytes(&mut data);

    let hex_str = hex::encode(data);
    let dest_base = mount_destination
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow!("get mount destination failed"))?;

    Ok(format!(
        "{}{}-{}-{}",
        DEFAULT_KATA_GUEST_SHARE_DIR, cid, hex_str, dest_base
    ))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::share_fs::{ShareFsMount, ShareFsMountResult, ShareFsRootfsConfig};
    use hypervisor::Hypervisor;

    #[test]
    fn test_is_system_mount() {
        let sys_dir = "/sys";
        let proc_dir = "/proc";
        let sys_sub_dir = "/sys/fs/cgroup";
        let proc_sub_dir = "/proc/cgroups";
        let not_sys_dir = "/root";

        assert!(is_system_mount(sys_dir));
        assert!(is_system_mount(proc_dir));
        assert!(is_system_mount(sys_sub_dir));
        assert!(is_system_mount(proc_sub_dir));
        assert!(!is_system_mount(not_sys_dir));
    }

    #[test]
    fn test_is_watchable_volume() {
        // The configmap is /var/lib/kubelet/pods/<uid>/volumes/kubernetes.io~configmap/kube-configmap-0s2no/{..data, key1, key2,...}
        // The secret is /var/lib/kubelet/pods/<uid>/volumes/kubernetes.io~secret/kube-secret-2s2np/{..data, key1, key2,...}
        // The projected is /var/lib/kubelet/pods/<uid>/volumes/kubernetes.io~projected/kube-api-access-8s2nl/{..data, key1, key2,...}
        // The downward-api is /var/lib/kubelet/pods/<uid>/volumes/kubernetes.io~downward-api/downward-api-xxxx/{..data, key1, key2,...}
        let configmap =
            "var/lib/kubelet/pods/1000/volumes/kubernetes.io~configmap/kube-configmap-0s2no";
        let secret = "var/lib/kubelet/pods/1000/volumes/kubernetes.io~secret/kube-secret-2s2np";
        let projected =
            "var/lib/kubelet/1000/<uid>/volumes/kubernetes.io~projected/kube-api-access-8s2nl";
        let downward_api =
            "var/lib/kubelet/1000/<uid>/volumes/kubernetes.io~downward-api/downward-api-xxxx";

        let temp_dir = tempfile::tempdir().unwrap();
        let cm_path = temp_dir.path().join(configmap);
        std::fs::create_dir_all(&cm_path).unwrap();
        let secret_path = temp_dir.path().join(secret);
        std::fs::create_dir_all(&secret_path).unwrap();
        let projected_path = temp_dir.path().join(projected);
        std::fs::create_dir_all(&projected_path).unwrap();
        let downward_api_path = temp_dir.path().join(downward_api);
        std::fs::create_dir_all(&downward_api_path).unwrap();

        assert!(is_watchable_volume(&cm_path));
        assert!(is_watchable_volume(&secret_path));
        assert!(is_watchable_volume(&projected_path));
        assert!(is_watchable_volume(&downward_api_path));
    }

    #[test]
    fn test_generate_copy_file_guest_path() {
        let path =
            generate_copy_file_guest_path("sandbox-id", Path::new("/etc/resolv.conf")).unwrap();

        assert!(path.starts_with(&format!("{DEFAULT_KATA_GUEST_SHARE_DIR}sandbox-id-")));
        assert!(path.ends_with("-resolv.conf"));
    }

    #[derive(Default)]
    struct FakeShareFsMount {
        calls: std::sync::Mutex<Vec<&'static str>>,
        fail_once: std::sync::Mutex<Option<&'static str>>,
    }

    impl FakeShareFsMount {
        fn record(&self, op: &'static str) -> Result<()> {
            self.calls.lock().unwrap().push(op);
            if *self.fail_once.lock().unwrap() == Some(op) {
                self.fail_once.lock().unwrap().take();
                return Err(anyhow!("injected {op} failure"));
            }
            Ok(())
        }

        fn count(&self, op: &str) -> usize {
            self.calls
                .lock()
                .unwrap()
                .iter()
                .filter(|c| **c == op)
                .count()
        }
    }

    #[async_trait]
    impl ShareFsMount for FakeShareFsMount {
        async fn share_rootfs(&self, _: &ShareFsRootfsConfig) -> Result<ShareFsMountResult> {
            unimplemented!()
        }
        async fn share_volume(&self, config: &ShareFsVolumeConfig) -> Result<ShareFsMountResult> {
            self.record("share")?;
            Ok(ShareFsMountResult {
                guest_path: format!("/guest/{}", config.target),
                storages: vec![],
            })
        }
        async fn upgrade_to_rw(&self, _: &str) -> Result<()> {
            self.record("upgrade")
        }
        async fn downgrade_to_ro(&self, _: &str) -> Result<()> {
            self.record("downgrade")
        }
        async fn umount_volume(&self, _: &str) -> Result<()> {
            self.record("umount")
        }
        async fn umount_rootfs(&self, _: &ShareFsRootfsConfig) -> Result<()> {
            unimplemented!()
        }
        async fn cleanup(&self, _: &str) -> Result<()> {
            unimplemented!()
        }
    }

    #[derive(Default)]
    struct FakeShareFs {
        mount: Arc<FakeShareFsMount>,
        mounted_info_set: Arc<Mutex<HashMap<String, MountedInfo>>>,
    }

    #[async_trait]
    impl ShareFs for FakeShareFs {
        fn get_share_fs_mount(&self) -> Arc<dyn ShareFsMount> {
            self.mount.clone()
        }
        async fn setup_device_before_start_vm(
            &self,
            _: &dyn Hypervisor,
            _: &RwLock<DeviceManager>,
        ) -> Result<()> {
            unimplemented!()
        }
        async fn setup_device_after_start_vm(
            &self,
            _: &dyn Hypervisor,
            _: &RwLock<DeviceManager>,
        ) -> Result<()> {
            unimplemented!()
        }
        async fn get_storages(&self) -> Result<Vec<agent::Storage>> {
            unimplemented!()
        }
        fn mounted_info_set(&self) -> Arc<Mutex<HashMap<String, MountedInfo>>> {
            self.mounted_info_set.clone()
        }
    }

    async fn share_volume(
        share_fs: Arc<dyn ShareFs>,
        source: &Path,
        cid: &str,
        readonly: bool,
    ) -> ShareFsVolume {
        let mut m = oci::Mount::default();
        m.set_source(Some(source.to_owned()));
        m.set_destination(PathBuf::from("/data"));
        m.set_typ(Some("bind".to_owned()));
        m.set_options(Some(vec![if readonly { "ro" } else { "rw" }.to_owned()]));
        let agent = Arc::new(agent::kata::KataAgent::new(Default::default()));
        ShareFsVolume::new(
            &Some(share_fs),
            &m,
            cid,
            readonly,
            agent,
            Arc::new(VolumeManager::new()),
        )
        .await
        .unwrap()
    }

    async fn ref_counts(share_fs: &FakeShareFs) -> Option<(usize, usize)> {
        share_fs
            .mounted_info_set
            .lock()
            .await
            .get("/host/volume")
            .map(|info| (info.ro_ref_count, info.rw_ref_count))
    }

    #[actix_rt::test]
    async fn test_cleanup_retries_failed_umount() {
        let share_fs = Arc::new(FakeShareFs::default());
        let device_manager = crate::test_utils::device_manager().await;
        let volume = share_volume(share_fs.clone(), Path::new("/host/volume"), "a", false).await;

        *share_fs.mount.fail_once.lock().unwrap() = Some("umount");
        assert!(volume.cleanup(&device_manager).await.is_err());
        volume.cleanup(&device_manager).await.unwrap();

        assert_eq!(share_fs.mount.count("umount"), 2);
        assert_eq!(ref_counts(&share_fs).await, None);
    }

    #[actix_rt::test]
    async fn test_repeated_cleanup_releases_reference_once() {
        let share_fs = Arc::new(FakeShareFs::default());
        let device_manager = crate::test_utils::device_manager().await;
        let a = share_volume(share_fs.clone(), Path::new("/host/volume"), "a", false).await;
        let b = share_volume(share_fs.clone(), Path::new("/host/volume"), "b", false).await;
        assert_eq!(ref_counts(&share_fs).await, Some((0, 2)));

        a.cleanup(&device_manager).await.unwrap();
        a.cleanup(&device_manager).await.unwrap();
        assert_eq!(ref_counts(&share_fs).await, Some((0, 1)));
        assert_eq!(share_fs.mount.count("umount"), 0);

        b.cleanup(&device_manager).await.unwrap();
        assert_eq!(ref_counts(&share_fs).await, None);
        assert_eq!(share_fs.mount.count("umount"), 1);
    }

    #[actix_rt::test]
    async fn test_cleanup_retries_failed_downgrade() {
        let share_fs = Arc::new(FakeShareFs::default());
        let device_manager = crate::test_utils::device_manager().await;
        let a = share_volume(share_fs.clone(), Path::new("/host/volume"), "a", false).await;
        let b = share_volume(share_fs.clone(), Path::new("/host/volume"), "b", true).await;
        assert_eq!(ref_counts(&share_fs).await, Some((1, 1)));

        *share_fs.mount.fail_once.lock().unwrap() = Some("downgrade");
        assert!(a.cleanup(&device_manager).await.is_err());
        a.cleanup(&device_manager).await.unwrap();
        assert_eq!(ref_counts(&share_fs).await, Some((1, 0)));
        assert_eq!(share_fs.mount.count("downgrade"), 2);

        b.cleanup(&device_manager).await.unwrap();
        assert_eq!(ref_counts(&share_fs).await, None);
        assert_eq!(share_fs.mount.count("umount"), 1);
    }

    #[actix_rt::test]
    #[ignore = "requires root and mount namespace capabilities"]
    async fn test_cleanup_retry_after_partial_failure() {
        use crate::share_fs::do_get_host_path;
        use crate::test_utils::{
            in_private_mount_namespace, inline_share_fs, is_mounted, set_immutable, SharedTmpfs,
        };

        if !in_private_mount_namespace(
            "volume::share_fs_volume::test::test_cleanup_retry_after_partial_failure",
        ) {
            return;
        }
        let tmpfs = SharedTmpfs::new();
        let sid = tmpfs.sandbox();
        let share_fs = inline_share_fs(&sid);
        let device_manager = crate::test_utils::device_manager().await;
        let source = tmpfs.path().join("volume");
        std::fs::create_dir(&source).unwrap();
        std::fs::write(source.join("sentinel"), b"volume data").unwrap();

        let volume = share_volume(share_fs.clone(), &source, "cid", false).await;
        let file_name = share_fs.mounted_info_set().lock().await[source.to_str().unwrap()]
            .file_name()
            .unwrap();
        let host = do_get_host_path(&file_name, &sid, "", true, false);
        let host_ro = do_get_host_path(&file_name, &sid, "", true, true);
        assert!(is_mounted(&host));
        assert!(is_mounted(&host_ro));

        let passthrough = Path::new(&host).parent().unwrap().to_owned();
        set_immutable(&passthrough, true);
        let result = volume.cleanup(&device_manager).await;
        set_immutable(&passthrough, false);
        assert!(result.is_err());
        assert!(!is_mounted(&host));

        volume.cleanup(&device_manager).await.unwrap();
        assert!(!Path::new(&host).exists());
        assert!(!is_mounted(&host_ro));
        assert!(share_fs.mounted_info_set().lock().await.is_empty());
        assert_eq!(
            std::fs::read(source.join("sentinel")).unwrap(),
            b"volume data"
        );
    }
}
