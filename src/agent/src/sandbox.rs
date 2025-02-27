// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::fs;
use std::os::fd::FromRawFd;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use std::{thread, time};

use anyhow::{anyhow, Context, Result};
use kata_types::cpu::CpuSet;
use kata_types::mount::StorageDevice;
use libc::{pid_t, syscall};
use nix::fcntl::{self, OFlag};
use nix::sched::{setns, unshare, CloneFlags};
use nix::sys::stat::Mode;
use oci::{Hook, Hooks};
use oci_spec::runtime as oci;
use protocols::agent::{OnlineCPUMemRequest, SharedMount};
use regex::Regex;
use rustjail::cgroups::{self as rustjail_cgroups, DevicesCgroupInfo};
use rustjail::container::BaseContainer;
use rustjail::container::LinuxContainer;
use rustjail::process::Process;
use slog::Logger;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::oneshot;
use tokio::sync::Mutex;
use tracing::instrument;

use crate::linux_abi::*;
use crate::mount::{get_mount_fs_type, TYPE_ROOTFS};
use crate::namespace::Namespace;
use crate::netlink::Handle;
use crate::network::Network;
use crate::pci;
use crate::storage::StorageDeviceGeneric;
use crate::uevent::{Uevent, UeventMatcher};
use crate::watcher::BindWatcher;

pub const ERR_INVALID_CONTAINER_ID: &str = "Invalid container id";

type UeventWatcher = (Box<dyn UeventMatcher>, oneshot::Sender<Uevent>);

#[derive(Clone)]
pub struct StorageState {
    count: Arc<AtomicU32>,
    device: Arc<dyn StorageDevice>,
}

impl Debug for StorageState {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StorageState").finish()
    }
}

impl StorageState {
    fn new() -> Self {
        StorageState {
            count: Arc::new(AtomicU32::new(1)),
            device: Arc::new(StorageDeviceGeneric::default()),
        }
    }

    pub fn from_device(device: Arc<dyn StorageDevice>) -> Self {
        Self {
            count: Arc::new(AtomicU32::new(1)),
            device,
        }
    }

    pub fn path(&self) -> Option<&str> {
        self.device.path()
    }

    pub async fn ref_count(&self) -> u32 {
        self.count.load(Ordering::Relaxed)
    }

    async fn inc_ref_count(&self) {
        self.count.fetch_add(1, Ordering::Acquire);
    }

    async fn dec_and_test_ref_count(&self) -> bool {
        self.count.fetch_sub(1, Ordering::AcqRel) == 1
    }
}

#[derive(Debug)]
pub struct Sandbox {
    pub logger: Logger,
    pub id: String,
    pub hostname: String,
    pub containers: HashMap<String, LinuxContainer>,
    pub network: Network,
    pub mounts: Vec<String>,
    pub container_mounts: HashMap<String, Vec<String>>,
    pub uevent_map: HashMap<String, Uevent>,
    pub uevent_watchers: Vec<Option<UeventWatcher>>,
    pub shared_utsns: Namespace,
    pub shared_ipcns: Namespace,
    pub sandbox_pidns: Option<Namespace>,
    pub storages: HashMap<String, StorageState>,
    pub running: bool,
    pub no_pivot_root: bool,
    pub sender: Option<tokio::sync::oneshot::Sender<i32>>,
    pub rtnl: Handle,
    pub hooks: Option<Hooks>,
    pub event_rx: Arc<Mutex<Receiver<String>>>,
    pub event_tx: Option<Sender<String>>,
    pub bind_watcher: BindWatcher,
    pub pcimap: HashMap<pci::Address, pci::Address>,
    pub devcg_info: Arc<RwLock<DevicesCgroupInfo>>,
}

impl Sandbox {
    #[instrument]
    pub fn new(logger: &Logger) -> Result<Self> {
        let fs_type = get_mount_fs_type("/")?;
        let logger = logger.new(o!("subsystem" => "sandbox"));
        let (tx, rx) = channel::<String>(100);
        let event_rx = Arc::new(Mutex::new(rx));

        Ok(Sandbox {
            logger: logger.clone(),
            id: String::new(),
            hostname: String::new(),
            network: Network::new(),
            containers: HashMap::new(),
            mounts: Vec::new(),
            container_mounts: HashMap::new(),
            uevent_map: HashMap::new(),
            uevent_watchers: Vec::new(),
            shared_utsns: Namespace::new(&logger),
            shared_ipcns: Namespace::new(&logger),
            sandbox_pidns: None,
            storages: HashMap::new(),
            running: false,
            no_pivot_root: fs_type.eq(TYPE_ROOTFS),
            sender: None,
            rtnl: Handle::new()?,
            hooks: None,
            event_rx,
            event_tx: Some(tx),
            bind_watcher: BindWatcher::new(),
            pcimap: HashMap::new(),
            devcg_info: Arc::new(RwLock::new(DevicesCgroupInfo::default())),
        })
    }

    /// Add a new storage object or increase reference count of existing one.
    /// The caller may detect new storage object by checking `StorageState.refcount == 1`.
    #[instrument]
    pub async fn add_sandbox_storage(&mut self, path: &str) -> StorageState {
        match self.storages.entry(path.to_string()) {
            Entry::Occupied(e) => {
                let state = e.get().clone();
                state.inc_ref_count().await;
                state
            }
            Entry::Vacant(e) => {
                let state = StorageState::new();
                e.insert(state.clone());
                state
            }
        }
    }

    /// Update the storage device associated with a path.
    pub fn update_sandbox_storage(
        &mut self,
        path: &str,
        device: Arc<dyn StorageDevice>,
    ) -> std::result::Result<Arc<dyn StorageDevice>, Arc<dyn StorageDevice>> {
        if !self.storages.contains_key(path) {
            return Err(device);
        }

        let state = StorageState::from_device(device);
        // Safe to unwrap() because we have just ensured existence of entry.
        let state = self.storages.insert(path.to_string(), state).unwrap();
        Ok(state.device)
    }

    /// Decrease reference count and destroy the storage object if reference count reaches zero.
    /// Returns `Ok(true)` if the reference count has reached zero and the storage object has been
    /// removed.
    #[instrument]
    pub async fn remove_sandbox_storage(&mut self, path: &str) -> Result<bool> {
        match self.storages.get(path) {
            None => Err(anyhow!("Sandbox storage with path {} not found", path)),
            Some(state) => {
                if state.dec_and_test_ref_count().await {
                    if let Some(storage) = self.storages.remove(path) {
                        storage.device.cleanup()?;
                    }
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
        }
    }

    #[instrument]
    pub async fn setup_shared_namespaces(&mut self) -> Result<bool> {
        // Set up shared IPC namespace
        self.shared_ipcns = Namespace::new(&self.logger)
            .get_ipc()
            .setup()
            .await
            .context("setup persistent IPC namespace")?;

        // // Set up shared UTS namespace
        self.shared_utsns = Namespace::new(&self.logger)
            .get_uts(self.hostname.as_str())
            .setup()
            .await
            .context("setup persistent UTS namespace")?;

        Ok(true)
    }

    #[instrument]
    pub fn update_shared_pidns(&mut self, c: &LinuxContainer) -> Result<()> {
        // Populate the shared pid path only if this is an infra container and
        // sandbox_pidns has not been passed in the create_sandbox request.
        // This means a separate pause process has not been created. We treat the
        // first container created as the infra container in that case
        // and use its pid namespace in case pid namespace needs to be shared.
        if self.sandbox_pidns.is_none() && self.containers.is_empty() {
            let init_pid = c.init_process_pid;
            if init_pid == -1 {
                return Err(anyhow!(
                    "Failed to setup pid namespace: init container pid is -1"
                ));
            }

            let mut pid_ns = Namespace::new(&self.logger).get_pid();
            pid_ns.path = format!("/proc/{}/ns/pid", init_pid);

            self.sandbox_pidns = Some(pid_ns);
        }

        Ok(())
    }

    pub fn add_container(&mut self, c: LinuxContainer) {
        self.containers.insert(c.id.clone(), c);
    }

    pub fn get_container(&mut self, id: &str) -> Option<&mut LinuxContainer> {
        self.containers.get_mut(id)
    }

    pub fn find_container_by_name(&self, name: &str) -> Option<&LinuxContainer> {
        self.containers
            .values()
            .find(|&c| c.config.container_name == name)
    }

    pub fn find_process(&mut self, pid: pid_t) -> Option<&mut Process> {
        for (_, c) in self.containers.iter_mut() {
            if let Some(p) = c.processes.get_mut(&pid) {
                return Some(p);
            }
        }

        None
    }

    pub fn find_container_process(&mut self, cid: &str, eid: &str) -> Result<&mut Process> {
        let ctr = self
            .get_container(cid)
            .ok_or_else(|| anyhow!(ERR_INVALID_CONTAINER_ID))?;

        if eid.is_empty() {
            return ctr
                .processes
                .get_mut(&ctr.init_process_pid)
                .ok_or_else(|| anyhow!("cannot find init process!"));
        }

        ctr.get_process(eid).map_err(|_| anyhow!("Invalid exec id"))
    }

    #[instrument]
    pub async fn destroy(&mut self) -> Result<()> {
        for ctr in self.containers.values_mut() {
            ctr.destroy().await?;
        }
        Ok(())
    }

    #[instrument]
    pub fn online_cpu_memory(&self, req: &OnlineCPUMemRequest) -> Result<()> {
        if req.nb_cpus > 0 {
            // online cpus
            online_cpus(&self.logger, req.nb_cpus as i32).context("online cpus")?;
        }

        if !req.cpu_only {
            // online memory
            online_memory(&self.logger).context("online memory")?;
        }

        if req.nb_cpus == 0 {
            return Ok(());
        }

        let guest_cpuset = rustjail_cgroups::fs::get_guest_cpuset()?;

        for (_, ctr) in self.containers.iter() {
            match ctr
                .config
                .spec
                .as_ref()
                .and_then(|spec| spec.linux().as_ref())
                .and_then(|linux| linux.resources().as_ref())
                .and_then(|resources| resources.cpu().as_ref())
                .and_then(|cpus| cpus.cpus().as_ref())
            {
                Some(cpu_set) => {
                    info!(self.logger, "updating {}", ctr.id.as_str());
                    ctr.cgroup_manager
                        .update_cpuset_path(guest_cpuset.as_str(), cpu_set)?;
                }
                None => continue,
            }
        }

        Ok(())
    }

    #[instrument]
    pub fn add_hooks(&mut self, dir: &str) -> Result<()> {
        let mut hooks = Hooks::default();
        if let Ok(hook) = self.find_hooks(dir, "prestart") {
            hooks.set_prestart(Some(hook));
        }
        if let Ok(hook) = self.find_hooks(dir, "poststart") {
            hooks.set_poststart(Some(hook));
        }
        if let Ok(hook) = self.find_hooks(dir, "poststop") {
            hooks.set_poststop(Some(hook));
        }
        self.hooks = Some(hooks);

        Ok(())
    }

    #[instrument]
    fn find_hooks(&self, hook_path: &str, hook_type: &str) -> Result<Vec<Hook>> {
        let mut hooks = Vec::new();
        for entry in fs::read_dir(Path::new(hook_path).join(hook_type))? {
            let entry = entry?;
            // Reject non-file, symlinks and non-executable files
            if !entry.file_type()?.is_file()
                || entry.file_type()?.is_symlink()
                || entry.metadata()?.permissions().mode() & 0o111 == 0
            {
                continue;
            }

            let name = entry.file_name();
            let mut hook = oci::Hook::default();
            hook.set_path(PathBuf::from(hook_path).join(hook_type).join(&name));
            hook.set_args(Some(vec![
                name.to_str().unwrap().to_owned(),
                hook_type.to_owned(),
            ]));

            info!(
                self.logger,
                "found {} hook {:?} mode {:o}",
                hook_type,
                hook,
                entry.metadata()?.permissions().mode()
            );

            hooks.push(hook);
        }

        Ok(hooks)
    }

    #[instrument]
    pub async fn run_oom_event_monitor(&self, mut rx: Receiver<String>, container_id: String) {
        let logger = self.logger.clone();
        let tx = match self.event_tx.as_ref() {
            Some(v) => v.clone(),
            None => {
                error!(
                    logger,
                    "sandbox.event_tx not found in run_oom_event_monitor"
                );
                return;
            }
        };

        tokio::spawn(async move {
            loop {
                let event = rx.recv().await;
                // None means the container has exited, and sender in OOM notifier is dropped.
                if event.is_none() {
                    return;
                }
                info!(logger, "got an OOM event {:?}", event);
                if let Err(e) = tx.send(container_id.clone()).await {
                    error!(logger, "failed to send message: {:?}", e);
                }
            }
        });
    }

    #[instrument]
    pub fn setup_shared_mounts(&self, c: &LinuxContainer, mounts: &Vec<SharedMount>) -> Result<()> {
        let mut src_ctrs: HashMap<String, i32> = HashMap::new();
        for shared_mount in mounts {
            match src_ctrs.get(&shared_mount.src_ctr) {
                None => {
                    if let Some(c) = self.find_container_by_name(&shared_mount.src_ctr) {
                        src_ctrs.insert(shared_mount.src_ctr.clone(), c.init_process_pid);
                    }
                }
                Some(_) => {}
            }
        }

        // If there are no shared mounts to be set up, return directly.
        if src_ctrs.is_empty() {
            return Ok(());
        }

        let mounts = mounts.clone();
        let init_mntns = fcntl::open(
            "/proc/self/ns/mnt",
            OFlag::O_RDONLY | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|e| anyhow!("failed to open /proc/self/ns/mnt: {}", e))?;
        // safe because the fd are opened by fcntl::open and used directly.
        let _init_mntns_f = unsafe { fs::File::from_raw_fd(init_mntns) };
        let dst_mntns_path = format!("/proc/{}/ns/mnt", c.init_process_pid);
        let dst_mntns = fcntl::open(
            dst_mntns_path.as_str(),
            OFlag::O_RDONLY | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .map_err(|e| anyhow!("failed to open {}: {}", dst_mntns_path.as_str(), e))?;
        // safe because the fd are opened by fcntl::open and used directly.
        let _dst_mntns_f = unsafe { fs::File::from_raw_fd(dst_mntns) };
        let new_thread = std::thread::spawn(move || {
            || -> Result<()> {
                // A process can't join a new mount namespace if it is sharing
                // filesystem-related attributes (using CLONE_FS flag) with another process.
                // Ref: https://man7.org/linux/man-pages/man2/setns.2.html
                //
                // The implementation of the Rust standard library's std::thread relies on
                // the CLONE_FS parameter at the low level.
                // Therefore, it is not possible to switch directly to the mount namespace using setns.
                // Instead, it is necessary to first switch to a new mount namespace using unshare.
                unshare(CloneFlags::CLONE_NEWNS)
                    .map_err(|e| anyhow!("failed to create new mount namespace: {}", e))?;
                for m in mounts {
                    if let Some(src_init_pid) = src_ctrs.get(m.src_ctr()) {
                        // Shared mount points are created by application process within the source container,
                        // so we need to ensure they are already prepared.
                        setns(init_mntns, CloneFlags::CLONE_NEWNS).map_err(|e| {
                            anyhow!("switch to initial mount namespace failed: {}", e)
                        })?;
                        let mut is_ready = false;
                        let start_time = Instant::now();
                        let time_out = Duration::from_millis(10_000);
                        loop {
                            let proc_mounts_path = format!("/proc/{}/mounts", *src_init_pid);
                            let proc_mounts = fs::read_to_string(proc_mounts_path.as_str())?;
                            let lines: Vec<&str> = proc_mounts.split('\n').collect();
                            for line in lines {
                                let parts: Vec<&str> = line.split_whitespace().collect();
                                if parts.len() >= 2 && parts[1] == m.src_path() {
                                    is_ready = true;
                                    break;
                                }
                            }

                            if is_ready {
                                break;
                            }

                            if start_time.elapsed() >= time_out {
                                break;
                            }

                            thread::sleep(Duration::from_millis(100));
                        }
                        if !is_ready {
                            continue;
                        }

                        // Switch to the src container to obtain shared mount points.
                        let src_mntns_path = format!("/proc/{}/ns/mnt", *src_init_pid);
                        let src_mntns = fcntl::open(
                            src_mntns_path.as_str(),
                            OFlag::O_RDONLY | OFlag::O_CLOEXEC,
                            Mode::empty(),
                        )
                        .map_err(|e| {
                            anyhow!("failed to open {}: {}", src_mntns_path.as_str(), e)
                        })?;
                        // safe because the fd are opened by fcntl::open and used directly.
                        let _src_mntns_f = unsafe { fs::File::from_raw_fd(src_mntns) };
                        setns(src_mntns, CloneFlags::CLONE_NEWNS).map_err(|e| {
                            anyhow!("switch to source mount namespace failed: {}", e)
                        })?;
                        let src = std::ffi::CString::new(m.src_path())?;
                        let mount_fd = unsafe {
                            syscall(
                                libc::SYS_open_tree,
                                libc::AT_FDCWD,
                                src.as_ptr(),
                                0x1 | 0x8000 | libc::O_CLOEXEC, // OPEN_TREE_CLONE | AT_RECURSIVE | OPEN_TREE_CLOEXEC
                            ) as i32
                        };
                        if mount_fd < 0 {
                            return Err(anyhow!(
                                "failed to clone mounted subtree on {}",
                                m.src_path()
                            ));
                        }
                        // safe because we have checked whether mount_fd is valid
                        let _mount_f = unsafe { fs::File::from_raw_fd(mount_fd) };

                        // Switch to the dst container and mount them.
                        setns(dst_mntns, CloneFlags::CLONE_NEWNS).map_err(|e| {
                            anyhow!("switch to destination mount namespace failed: {}", e)
                        })?;
                        fs::create_dir_all(m.dst_path())?;
                        let dst = std::ffi::CString::new(m.dst_path())?;
                        let empty = std::ffi::CString::new("")?;
                        unsafe {
                            syscall(
                                libc::SYS_move_mount,
                                mount_fd,
                                empty.as_ptr(),
                                libc::AT_FDCWD,
                                dst.as_ptr(),
                                4, // MOVE_MOUNT_F_EMPTY_PATH
                            )
                        };
                    }
                }

                Ok(())
            }()
        });

        new_thread
            .join()
            .map_err(|e| anyhow!("Failed to join thread {:?}!", e))??;

        Ok(())
    }
}

#[instrument]
fn online_resources(logger: &Logger, path: &str, pattern: &str, num: i32) -> Result<i32> {
    let mut count = 0;
    let re = Regex::new(pattern)?;

    for e in fs::read_dir(path)? {
        let entry = e?;
        // Skip direntry which doesn't match the pattern.
        match entry.file_name().to_str() {
            None => continue,
            Some(v) => {
                if !re.is_match(v) {
                    continue;
                }
            }
        };

        let p = entry.path().join(SYSFS_ONLINE_FILE);
        if let Ok(c) = fs::read_to_string(&p) {
            // Try to online the object in offline state.
            if c.trim().contains('0') && fs::write(&p, "1").is_ok() && num > 0 {
                count += 1;
                if count == num {
                    break;
                }
            }
        }
    }

    Ok(count)
}

#[instrument]
fn online_memory(logger: &Logger) -> Result<()> {
    online_resources(logger, SYSFS_MEMORY_ONLINE_PATH, r"memory[0-9]+", -1)
        .context("online memory resource")?;
    Ok(())
}

// max wait for all CPUs to online will use 50 * 100 = 5 seconds.
const ONLINE_CPUMEM_WAIT_MILLIS: u64 = 50;
const ONLINE_CPUMEM_MAX_RETRIES: i32 = 100;

#[instrument]
fn online_cpus(logger: &Logger, num: i32) -> Result<i32> {
    let mut onlined_cpu_count = onlined_cpus().context("onlined cpu count")?;
    // for some vmms, like dragonball, they will online cpus for us
    // so check first whether agent need to do the online operation
    if onlined_cpu_count >= num {
        return Ok(num);
    }

    for i in 0..ONLINE_CPUMEM_MAX_RETRIES {
        // online num resources
        online_resources(
            logger,
            SYSFS_CPU_PATH,
            r"cpu[0-9]+",
            num - onlined_cpu_count,
        )
        .context("online cpu resource")?;

        onlined_cpu_count = onlined_cpus().context("onlined cpu count")?;
        if onlined_cpu_count >= num {
            info!(
                logger,
                "Currently {} onlined CPU(s) after {} retries", onlined_cpu_count, i
            );
            return Ok(num);
        }
        thread::sleep(time::Duration::from_millis(ONLINE_CPUMEM_WAIT_MILLIS));
    }

    Err(anyhow!(
        "failed to online {} CPU(s) after {} retries",
        num,
        ONLINE_CPUMEM_MAX_RETRIES
    ))
}

fn onlined_cpus() -> Result<i32> {
    let content =
        fs::read_to_string(SYSFS_CPU_ONLINE_PATH).context("read sysfs cpu online file")?;
    let online_cpu_set = CpuSet::from_str(content.trim())?;
    Ok(online_cpu_set.len() as i32)
}

#[cfg(test)]
#[allow(dead_code)]
#[allow(unused_imports)]
mod tests {
    use super::*;
    use crate::mount::baremount;
    use anyhow::{anyhow, Error};
    use nix::mount::MsFlags;
    use oci::{Linux, LinuxBuilder, LinuxDeviceCgroup, LinuxResources, Root, Spec, SpecBuilder};
    use oci_spec::runtime as oci;
    use rustjail::container::LinuxContainer;
    use rustjail::process::Process;
    use rustjail::specconv::CreateOpts;
    use slog::Logger;
    use std::fs::{self, File};
    use std::io::prelude::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tempfile::{tempdir, Builder, TempDir};
    use test_utils::skip_if_not_root;

    const CGROUP_PARENT: &str = "kata.agent.test.k8s.io";

    fn bind_mount(src: &str, dst: &str, logger: &Logger) -> Result<(), Error> {
        let src_path = Path::new(src);
        let dst_path = Path::new(dst);

        baremount(src_path, dst_path, "bind", MsFlags::MS_BIND, "", logger)
    }

    use serial_test::serial;

    #[tokio::test]
    #[serial]
    async fn set_sandbox_storage() {
        let logger = slog::Logger::root(slog::Discard, o!());
        let mut s = Sandbox::new(&logger).unwrap();

        let tmpdir = Builder::new().tempdir().unwrap();
        let tmpdir_path = tmpdir.path().to_str().unwrap();

        // Add a new sandbox storage
        let new_storage = s.add_sandbox_storage(tmpdir_path).await;

        // Check the reference counter
        let ref_count = new_storage.ref_count().await;
        assert_eq!(
            ref_count, 1,
            "Invalid refcount, got {} expected 1.",
            ref_count
        );

        // Use the existing sandbox storage
        let new_storage = s.add_sandbox_storage(tmpdir_path).await;

        // Since we are using existing storage, the reference counter
        // should be 2 by now.
        let ref_count = new_storage.ref_count().await;
        assert_eq!(
            ref_count, 2,
            "Invalid refcount, got {} expected 2.",
            ref_count
        );
    }

    #[tokio::test]
    #[serial]
    async fn unset_and_remove_sandbox_storage() {
        skip_if_not_root!();

        let logger = slog::Logger::root(slog::Discard, o!());
        let mut s = Sandbox::new(&logger).unwrap();

        assert!(
            s.remove_sandbox_storage("/tmp/testEphePath").await.is_err(),
            "Should fail because sandbox storage doesn't exist"
        );

        let tmpdir = Builder::new().tempdir().unwrap();
        let tmpdir_path = tmpdir.path().to_str().unwrap();

        let srcdir = Builder::new()
            .prefix("src")
            .tempdir_in(tmpdir_path)
            .unwrap();
        let srcdir_path = srcdir.path().to_str().unwrap();

        let destdir = Builder::new()
            .prefix("dest")
            .tempdir_in(tmpdir_path)
            .unwrap();
        let destdir_path = destdir.path().to_str().unwrap();

        assert!(bind_mount(srcdir_path, destdir_path, &logger).is_ok());

        s.add_sandbox_storage(destdir_path).await;
        let storage = StorageDeviceGeneric::new(destdir_path.to_string());
        assert!(s
            .update_sandbox_storage(destdir_path, Arc::new(storage))
            .is_ok());
        assert!(s.remove_sandbox_storage(destdir_path).await.is_ok());

        let other_dir_str;
        {
            // Create another folder in a separate scope to ensure that is
            // deleted
            let other_dir = Builder::new()
                .prefix("dir")
                .tempdir_in(tmpdir_path)
                .unwrap();
            let other_dir_path = other_dir.path().to_str().unwrap();
            other_dir_str = other_dir_path.to_string();

            s.add_sandbox_storage(other_dir_path).await;
            let storage = StorageDeviceGeneric::new(other_dir_path.to_string());
            assert!(s
                .update_sandbox_storage(other_dir_path, Arc::new(storage))
                .is_ok());
        }

        assert!(s.remove_sandbox_storage(&other_dir_str).await.is_ok());
    }

    #[tokio::test]
    #[serial]
    async fn unset_sandbox_storage() {
        let logger = slog::Logger::root(slog::Discard, o!());
        let mut s = Sandbox::new(&logger).unwrap();

        let storage_path = "/tmp/testEphe";

        // Add a new sandbox storage
        s.add_sandbox_storage(storage_path).await;
        // Use the existing sandbox storage
        let state = s.add_sandbox_storage(storage_path).await;
        assert!(
            state.ref_count().await > 1,
            "Expects false as the storage is not new."
        );

        assert!(
            !s.remove_sandbox_storage(storage_path).await.unwrap(),
            "Expects false as there is still a storage."
        );

        // Reference counter should decrement to 1.
        let storage = &s.storages[storage_path];
        let refcount = storage.ref_count().await;
        assert_eq!(
            refcount, 1,
            "Invalid refcount, got {} expected 1.",
            refcount
        );

        assert!(
            s.remove_sandbox_storage(storage_path).await.unwrap(),
            "Expects true as there is still a storage."
        );

        // Since no container is using this sandbox storage anymore
        // there should not be any reference in sandbox struct
        // for the given storage
        assert!(
            !s.storages.contains_key(storage_path),
            "The storages map should not contain the key {}",
            storage_path
        );

        // If no container is using the sandbox storage, the reference
        // counter for it should not exist.
        assert!(
            s.remove_sandbox_storage(storage_path).await.is_err(),
            "Expects false as the reference counter should no exist."
        );
    }

    fn create_dummy_opts() -> CreateOpts {
        let start = SystemTime::now();
        let since_the_epoch = start
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");

        let mut root = Root::default();
        root.set_path(PathBuf::from("/"));

        let mut cgroup = LinuxDeviceCgroup::default();
        cgroup.set_allow(true);
        cgroup.set_access(Some(String::from("rwm")));

        let mut linux_resources = LinuxResources::default();
        linux_resources.set_devices(Some(vec![cgroup]));

        let cgroups_path = format!(
            "/{}/dummycontainer{}",
            CGROUP_PARENT,
            since_the_epoch.as_millis()
        );

        let spec = SpecBuilder::default()
            .linux(
                LinuxBuilder::default()
                    .cgroups_path(cgroups_path)
                    .resources(linux_resources)
                    .build()
                    .unwrap(),
            )
            .root(root)
            .build()
            .unwrap();

        CreateOpts {
            cgroup_name: "".to_string(),
            use_systemd_cgroup: false,
            no_pivot_root: false,
            no_new_keyring: false,
            spec: Some(spec),
            rootless_euid: false,
            rootless_cgroup: false,
            container_name: "".to_string(),
        }
    }

    fn create_linuxcontainer() -> (LinuxContainer, TempDir) {
        // Create a temporal directory
        let dir = tempdir()
            .map_err(|e| anyhow!(e).context("tempdir failed"))
            .unwrap();

        let container = LinuxContainer::new(
            "some_id",
            dir.path().join("rootfs").to_str().unwrap(),
            None,
            create_dummy_opts(),
            &slog_scope::logger(),
        )
        .unwrap();

        // Create a new container
        (container, dir)
    }

    #[tokio::test]
    #[serial]
    async fn get_container_entry_exist() {
        skip_if_not_root!();

        let logger = slog::Logger::root(slog::Discard, o!());
        let mut s = Sandbox::new(&logger).unwrap();
        let (linux_container, _root) = create_linuxcontainer();

        s.containers
            .insert("testContainerID".to_string(), linux_container);
        let cnt = s.get_container("testContainerID");
        assert!(cnt.is_some());
    }

    #[tokio::test]
    #[serial]
    async fn get_container_no_entry() {
        let logger = slog::Logger::root(slog::Discard, o!());
        let mut s = Sandbox::new(&logger).unwrap();

        let cnt = s.get_container("testContainerID");
        assert!(cnt.is_none());
    }

    #[tokio::test]
    #[serial]
    #[cfg(not(target_arch = "powerpc64"))]
    async fn add_and_get_container() {
        skip_if_not_root!();

        let logger = slog::Logger::root(slog::Discard, o!());
        let mut s = Sandbox::new(&logger).unwrap();
        let (linux_container, _root) = create_linuxcontainer();

        s.add_container(linux_container);
        assert!(s.get_container("some_id").is_some());
    }

    #[tokio::test]
    #[serial]
    async fn update_shared_pidns() {
        skip_if_not_root!();

        let logger = slog::Logger::root(slog::Discard, o!());
        let mut s = Sandbox::new(&logger).unwrap();
        let test_pid = 9999;

        let (mut linux_container, _root) = create_linuxcontainer();
        linux_container.init_process_pid = test_pid;

        s.update_shared_pidns(&linux_container).unwrap();

        assert!(s.sandbox_pidns.is_some());

        let ns_path = format!("/proc/{}/ns/pid", test_pid);
        assert_eq!(s.sandbox_pidns.unwrap().path, ns_path);
    }

    #[tokio::test]
    #[serial]
    async fn add_guest_hooks() {
        let logger = slog::Logger::root(slog::Discard, o!());
        let mut s = Sandbox::new(&logger).unwrap();
        let tmpdir = Builder::new().tempdir().unwrap();
        let tmpdir_path = tmpdir.path().to_str().unwrap();

        assert!(fs::create_dir_all(tmpdir.path().join("prestart")).is_ok());
        assert!(fs::create_dir_all(tmpdir.path().join("poststop")).is_ok());

        let file = File::create(tmpdir.path().join("prestart").join("prestart.sh")).unwrap();
        let mut perm = file.metadata().unwrap().permissions();
        perm.set_mode(0o777);
        assert!(file.set_permissions(perm).is_ok());
        assert!(File::create(tmpdir.path().join("poststop").join("poststop.sh")).is_ok());

        assert!(s.add_hooks(tmpdir_path).is_ok());
        assert!(s.hooks.is_some());
        assert!(s.hooks.as_ref().unwrap().prestart().clone().unwrap().len() == 1);
        // As we don't create poststart/xxx, the poststart will be none
        assert!(s.hooks.as_ref().unwrap().poststart().clone().is_none());
        // poststop path is created but as the problem of file perm is rejected.
        assert!(s
            .hooks
            .as_ref()
            .unwrap()
            .poststop()
            .clone()
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    #[serial]
    async fn test_sandbox_set_destroy() {
        let logger = slog::Logger::root(slog::Discard, o!());
        let mut s = Sandbox::new(&logger).unwrap();
        let ret = s.destroy().await;
        assert!(ret.is_ok());
    }

    #[tokio::test]
    #[cfg(not(target_arch = "powerpc64"))]
    async fn test_find_container_process() {
        skip_if_not_root!();

        let logger = slog::Logger::root(slog::Discard, o!());
        let mut s = Sandbox::new(&logger).unwrap();
        let cid = "container-123";

        let (mut linux_container, _root) = create_linuxcontainer();
        linux_container.init_process_pid = 1;
        linux_container.id = cid.to_string();
        // add init process
        linux_container.processes.insert(
            1,
            Process::new(&logger, &oci::Process::default(), "1", true, 1, None).unwrap(),
        );
        // add exec process
        linux_container.processes.insert(
            123,
            Process::new(
                &logger,
                &oci::Process::default(),
                "exec-123",
                false,
                1,
                None,
            )
            .unwrap(),
        );

        s.add_container(linux_container);

        // empty exec-id will return init process
        let p = s.find_container_process(cid, "");
        assert!(p.is_ok(), "Expecting Ok, Got {:?}", p);
        let p = p.unwrap();
        assert_eq!("1", p.exec_id, "exec_id should be 1");
        assert!(p.init, "init flag should be true");

        // get exist exec-id will return the exec process
        let p = s.find_container_process(cid, "exec-123");
        assert!(p.is_ok(), "Expecting Ok, Got {:?}", p);
        let p = p.unwrap();
        assert_eq!("exec-123", p.exec_id, "exec_id should be exec-123");
        assert!(!p.init, "init flag should be false");

        // get not exist exec-id will return error
        let p = s.find_container_process(cid, "exec-456");
        assert!(p.is_err(), "Expecting Error, Got {:?}", p);

        // container does not exist
        let p = s.find_container_process("not-exist-cid", "");
        assert!(p.is_err(), "Expecting Error, Got {:?}", p);
    }

    #[tokio::test]
    #[cfg(not(target_arch = "powerpc64"))]
    async fn test_find_process() {
        skip_if_not_root!();

        let logger = slog::Logger::root(slog::Discard, o!());

        let test_pids = [i32::MIN, -1, 0, 1, i32::MAX];

        for test_pid in test_pids {
            let mut s = Sandbox::new(&logger).unwrap();
            let (mut linux_container, _root) = create_linuxcontainer();

            let mut test_process = Process::new(
                &logger,
                &oci::Process::default(),
                "this_is_a_test_process",
                true,
                1,
                None,
            )
            .unwrap();
            // processes interally only have pids when manually set
            test_process.pid = test_pid;

            linux_container.processes.insert(test_pid, test_process);

            s.add_container(linux_container);

            let find_result = s.find_process(test_pid);

            // test first if it finds anything
            assert!(find_result.is_some(), "Should be able to find a process");

            let found_process = find_result.unwrap();

            // then test if it founds the correct process
            assert_eq!(
                found_process.pid, test_pid,
                "Should be able to find correct process"
            );
        }

        // to test for nonexistent pids, any pid that isn't the one set
        // above should work, as linuxcontainer starts with no processes
        let mut s = Sandbox::new(&logger).unwrap();

        let nonexistent_test_pid = 1234;

        let find_result = s.find_process(nonexistent_test_pid);

        assert!(
            find_result.is_none(),
            "Shouldn't find a process for non existent pid"
        );
    }

    #[tokio::test]
    async fn test_online_resources() {
        #[derive(Debug, Default)]
        struct TestFile {
            name: String,
            content: String,
        }

        #[derive(Debug, Default)]
        struct TestDirectory<'a> {
            name: String,
            files: &'a [TestFile],
        }

        #[derive(Debug)]
        struct TestData<'a> {
            directory_autogen_name: String,
            number_autogen_directories: u32,

            extra_directories: &'a [TestDirectory<'a>],
            pattern: String,
            to_enable: i32,

            result: Result<i32>,
        }

        impl Default for TestData<'_> {
            fn default() -> Self {
                TestData {
                    directory_autogen_name: Default::default(),
                    number_autogen_directories: Default::default(),
                    extra_directories: Default::default(),
                    pattern: Default::default(),
                    to_enable: Default::default(),
                    result: Ok(Default::default()),
                }
            }
        }

        let tests = &[
            // 4 well formed directories, request enabled 4,
            // correct result 4 enabled, should pass
            TestData {
                directory_autogen_name: String::from("cpu"),
                number_autogen_directories: 4,
                pattern: String::from(r"cpu[0-9]+"),
                to_enable: 4,
                result: Ok(4),
                ..Default::default()
            },
            // 0 well formed directories, request enabled 4,
            // correct result 0 enabled, should pass
            TestData {
                number_autogen_directories: 0,
                to_enable: 4,
                result: Ok(0),
                ..Default::default()
            },
            // 10 well formed directories, request enabled 4,
            // correct result 4 enabled, should pass
            TestData {
                directory_autogen_name: String::from("cpu"),
                number_autogen_directories: 10,
                pattern: String::from(r"cpu[0-9]+"),
                to_enable: 4,
                result: Ok(4),
                ..Default::default()
            },
            // 0 well formed directories, request enabled 0,
            // correct result 0 enabled, should pass
            TestData {
                number_autogen_directories: 0,
                pattern: String::from(r"cpu[0-9]+"),
                to_enable: 0,
                result: Ok(0),
                ..Default::default()
            },
            // 4 well formed directories, 1 malformed (no online file),
            // request enable 5, correct result 4
            TestData {
                directory_autogen_name: String::from("cpu"),
                number_autogen_directories: 4,
                pattern: String::from(r"cpu[0-9]+"),
                extra_directories: &[TestDirectory {
                    name: String::from("cpu4"),
                    files: &[],
                }],
                to_enable: 5,
                result: Ok(4),
            },
            // 3 malformed directories (no online files),
            // request enable 3, correct result 0
            TestData {
                pattern: String::from(r"cpu[0-9]+"),
                extra_directories: &[
                    TestDirectory {
                        name: String::from("cpu0"),
                        files: &[],
                    },
                    TestDirectory {
                        name: String::from("cpu1"),
                        files: &[],
                    },
                    TestDirectory {
                        name: String::from("cpu2"),
                        files: &[],
                    },
                ],
                to_enable: 3,
                result: Ok(0),
                ..Default::default()
            },
            // 1 malformed directories (online file with content "1"),
            // request enable 1, correct result 0
            TestData {
                pattern: String::from(r"cpu[0-9]+"),
                extra_directories: &[TestDirectory {
                    name: String::from("cpu0"),
                    files: &[TestFile {
                        name: SYSFS_ONLINE_FILE.to_string(),
                        content: String::from("1"),
                    }],
                }],
                to_enable: 1,
                result: Ok(0),
                ..Default::default()
            },
            // 2 well formed directories, 1 malformed (online file with content "1"),
            // request enable 3, correct result 2
            TestData {
                directory_autogen_name: String::from("cpu"),
                number_autogen_directories: 2,
                pattern: String::from(r"cpu[0-9]+"),
                extra_directories: &[TestDirectory {
                    name: String::from("cpu2"),
                    files: &[TestFile {
                        name: SYSFS_ONLINE_FILE.to_string(),
                        content: String::from("1"),
                    }],
                }],
                to_enable: 3,
                result: Ok(2),
            },
        ];

        let logger = slog::Logger::root(slog::Discard, o!());
        let tmpdir = Builder::new().tempdir().unwrap();
        let tmpdir_path = tmpdir.path().to_str().unwrap();

        for (i, d) in tests.iter().enumerate() {
            let current_test_dir_path = format!("{}/test_{}", tmpdir_path, i);
            fs::create_dir(&current_test_dir_path).unwrap();

            // create numbered directories and fill using root name
            for j in 0..d.number_autogen_directories {
                let subdir_path = format!(
                    "{}/{}{}",
                    current_test_dir_path, d.directory_autogen_name, j
                );
                let subfile_path = format!("{}/{}", subdir_path, SYSFS_ONLINE_FILE);
                fs::create_dir(&subdir_path).unwrap();
                let mut subfile = File::create(subfile_path).unwrap();
                subfile.write_all(b"0").unwrap();
            }
            // create extra directories and fill to specification
            for j in d.extra_directories {
                let subdir_path = format!("{}/{}", current_test_dir_path, j.name);
                fs::create_dir(&subdir_path).unwrap();
                for file in j.files {
                    let subfile_path = format!("{}/{}", subdir_path, file.name);
                    let mut subfile = File::create(subfile_path).unwrap();
                    subfile.write_all(file.content.as_bytes()).unwrap();
                }
            }

            // run created directory structure against online_resources
            let result = online_resources(&logger, &current_test_dir_path, &d.pattern, d.to_enable);

            let mut msg = format!(
                "test[{}]: {:?}, expected {}, actual {}",
                i,
                d,
                d.result.is_ok(),
                result.is_ok()
            );

            assert_eq!(result.is_ok(), d.result.is_ok(), "{}", msg);

            if d.result.is_ok() {
                let test_result_val = *d.result.as_ref().ok().unwrap();
                let result_val = result.ok().unwrap();

                msg = format!(
                    "test[{}]: {:?}, expected {}, actual {}",
                    i, d, test_result_val, result_val
                );

                assert_eq!(test_result_val, result_val, "{}", msg);
            }
        }
    }
}
