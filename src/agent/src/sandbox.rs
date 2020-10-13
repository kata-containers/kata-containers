// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

//use crate::container::Container;
use crate::linux_abi::*;
use crate::mount::{get_mount_fs_type, remove_mounts, TYPEROOTFS};
use crate::namespace::Namespace;
use crate::network::Network;
use anyhow::{anyhow, Context, Result};
use libc::pid_t;
use netlink::{RtnlHandle, NETLINK_ROUTE};
use oci::{Hook, Hooks};
use protocols::agent::OnlineCPUMemRequest;
use regex::Regex;
use rustjail::cgroups as rustjail_cgroups;
use rustjail::container::BaseContainer;
use rustjail::container::LinuxContainer;
use rustjail::process::Process;
use slog::Logger;
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::{thread, time};

#[derive(Debug)]
pub struct Sandbox {
    pub logger: Logger,
    pub id: String,
    pub hostname: String,
    pub containers: HashMap<String, LinuxContainer>,
    pub network: Network,
    pub mounts: Vec<String>,
    pub container_mounts: HashMap<String, Vec<String>>,
    pub pci_device_map: HashMap<String, String>,
    pub shared_utsns: Namespace,
    pub shared_ipcns: Namespace,
    pub sandbox_pidns: Option<Namespace>,
    pub storages: HashMap<String, u32>,
    pub running: bool,
    pub no_pivot_root: bool,
    pub sender: Option<Sender<i32>>,
    pub rtnl: Option<RtnlHandle>,
    pub hooks: Option<Hooks>,
    pub event_rx: Arc<Mutex<Receiver<String>>>,
    pub event_tx: Sender<String>,
}

impl Sandbox {
    pub fn new(logger: &Logger) -> Result<Self> {
        let fs_type = get_mount_fs_type("/")?;
        let logger = logger.new(o!("subsystem" => "sandbox"));
        let (tx, rx) = mpsc::channel::<String>();
        let event_rx = Arc::new(Mutex::new(rx));

        Ok(Sandbox {
            logger: logger.clone(),
            id: String::new(),
            hostname: String::new(),
            network: Network::new(),
            containers: HashMap::new(),
            mounts: Vec::new(),
            container_mounts: HashMap::new(),
            pci_device_map: HashMap::new(),
            shared_utsns: Namespace::new(&logger),
            shared_ipcns: Namespace::new(&logger),
            sandbox_pidns: None,
            storages: HashMap::new(),
            running: false,
            no_pivot_root: fs_type.eq(TYPEROOTFS),
            sender: None,
            rtnl: Some(RtnlHandle::new(NETLINK_ROUTE, 0).unwrap()),
            hooks: None,
            event_rx: event_rx,
            event_tx: tx,
        })
    }

    // set_sandbox_storage sets the sandbox level reference
    // counter for the sandbox storage.
    // This method also returns a boolean to let
    // callers know if the storage already existed or not.
    // It will return true if storage is new.
    //
    // It's assumed that caller is calling this method after
    // acquiring a lock on sandbox.
    pub fn set_sandbox_storage(&mut self, path: &str) -> bool {
        match self.storages.get_mut(path) {
            None => {
                self.storages.insert(path.to_string(), 1);
                true
            }
            Some(count) => {
                *count += 1;
                false
            }
        }
    }

    // unset_sandbox_storage will decrement the sandbox storage
    // reference counter. If there aren't any containers using
    // that sandbox storage, this method will remove the
    // storage reference from the sandbox and return 'true' to
    // let the caller know that they can clean up the storage
    // related directories by calling remove_sandbox_storage
    //
    // It's assumed that caller is calling this method after
    // acquiring a lock on sandbox.
    pub fn unset_sandbox_storage(&mut self, path: &str) -> Result<bool> {
        match self.storages.get_mut(path) {
            None => return Err(anyhow!("Sandbox storage with path {} not found", path)),
            Some(count) => {
                *count -= 1;
                if *count < 1 {
                    self.storages.remove(path);
                    return Ok(true);
                }
                return Ok(false);
            }
        }
    }

    // remove_sandbox_storage removes the sandbox storage if no
    // containers are using that storage.
    //
    // It's assumed that caller is calling this method after
    // acquiring a lock on sandbox.
    pub fn remove_sandbox_storage(&self, path: &str) -> Result<()> {
        let mounts = vec![path.to_string()];
        remove_mounts(&mounts)?;
        fs::remove_dir_all(path).context(format!("failed to remove dir {:?}", path))?;
        Ok(())
    }

    // unset_and_remove_sandbox_storage unsets the storage from sandbox
    // and if there are no containers using this storage it will
    // remove it from the sandbox.
    //
    // It's assumed that caller is calling this method after
    // acquiring a lock on sandbox.
    pub fn unset_and_remove_sandbox_storage(&mut self, path: &str) -> Result<()> {
        if self.unset_sandbox_storage(path)? {
            return self.remove_sandbox_storage(path);
        }

        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn set_hostname(&mut self, hostname: String) {
        self.hostname = hostname;
    }

    pub fn setup_shared_namespaces(&mut self) -> Result<bool> {
        // Set up shared IPC namespace
        self.shared_ipcns = Namespace::new(&self.logger)
            .as_ipc()
            .setup()
            .context("Failed to setup persistent IPC namespace")?;

        // // Set up shared UTS namespace
        self.shared_utsns = Namespace::new(&self.logger)
            .as_uts(self.hostname.as_str())
            .setup()
            .context("Failed to setup persistent UTS namespace")?;

        Ok(true)
    }

    pub fn add_container(&mut self, c: LinuxContainer) {
        self.containers.insert(c.id.clone(), c);
    }

    pub fn update_shared_pidns(&mut self, c: &LinuxContainer) -> Result<()> {
        // Populate the shared pid path only if this is an infra container and
        // sandbox_pidns has not been passed in the create_sandbox request.
        // This means a separate pause process has not been created. We treat the
        // first container created as the infra container in that case
        // and use its pid namespace in case pid namespace needs to be shared.
        if self.sandbox_pidns.is_none() && self.containers.len() == 0 {
            let init_pid = c.init_process_pid;
            if init_pid == -1 {
                return Err(anyhow!(
                    "Failed to setup pid namespace: init container pid is -1"
                ));
            }

            let mut pid_ns = Namespace::new(&self.logger).as_pid();
            pid_ns.path = format!("/proc/{}/ns/pid", init_pid);

            self.sandbox_pidns = Some(pid_ns);
        }

        Ok(())
    }

    pub fn get_container(&mut self, id: &str) -> Option<&mut LinuxContainer> {
        self.containers.get_mut(id)
    }

    pub fn find_process(&mut self, pid: pid_t) -> Option<&mut Process> {
        for (_, c) in self.containers.iter_mut() {
            if c.processes.get(&pid).is_some() {
                return c.processes.get_mut(&pid);
            }
        }

        None
    }

    pub fn destroy(&mut self) -> Result<()> {
        for (_, ctr) in &mut self.containers {
            ctr.destroy()?;
        }
        Ok(())
    }

    pub fn online_cpu_memory(&self, req: &OnlineCPUMemRequest) -> Result<()> {
        if req.nb_cpus > 0 {
            // online cpus
            online_cpus(&self.logger, req.nb_cpus as i32)?;
        }

        if !req.cpu_only {
            // online memory
            online_memory(&self.logger)?;
        }

        let cpuset = rustjail_cgroups::fs::get_guest_cpuset()?;

        for (_, ctr) in self.containers.iter() {
            info!(self.logger, "updating {}", ctr.id.as_str());
            ctr.cgroup_manager
                .as_ref()
                .unwrap()
                .update_cpuset_path(cpuset.as_str())?;
        }

        Ok(())
    }

    pub fn add_hooks(&mut self, dir: &str) -> Result<()> {
        let mut hooks = Hooks::default();
        if let Ok(hook) = self.find_hooks(dir, "prestart") {
            hooks.prestart = hook;
        }
        if let Ok(hook) = self.find_hooks(dir, "poststart") {
            hooks.poststart = hook;
        }
        if let Ok(hook) = self.find_hooks(dir, "poststop") {
            hooks.poststop = hook;
        }
        self.hooks = Some(hooks);
        Ok(())
    }

    fn find_hooks(&self, hook_path: &str, hook_type: &str) -> Result<Vec<Hook>> {
        let mut hooks = Vec::new();
        for entry in fs::read_dir(Path::new(hook_path).join(hook_type))? {
            let entry = entry?;
            // Reject non-file, symlinks and non-executable files
            if !entry.file_type()?.is_file()
                || entry.file_type()?.is_symlink()
                || entry.metadata()?.permissions().mode() & 0o777 & 0o111 == 0
            {
                continue;
            }

            let name = entry.file_name();
            let hook = Hook {
                path: Path::new(hook_path)
                    .join(hook_type)
                    .join(&name)
                    .to_str()
                    .unwrap()
                    .to_owned(),
                args: vec![name.to_str().unwrap().to_owned(), hook_type.to_owned()],
                ..Default::default()
            };
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

    pub fn run_oom_event_monitor(&self, rx: Receiver<String>, container_id: String) {
        let tx = self.event_tx.clone();
        let logger = self.logger.clone();

        thread::spawn(move || {
            for event in rx {
                info!(logger, "got an OOM event {:?}", event);
                let _ = tx
                    .send(container_id.clone())
                    .map_err(|e| error!(logger, "failed to send message: {:?}", e));
            }
        });
    }
}

fn online_resources(logger: &Logger, path: &str, pattern: &str, num: i32) -> Result<i32> {
    let mut count = 0;
    let re = Regex::new(pattern)?;

    for e in fs::read_dir(path)? {
        let entry = e?;
        let tmpname = entry.file_name();
        let name = tmpname.to_str().unwrap();
        let p = entry.path();

        if re.is_match(name) {
            let file = format!("{}/{}", p.to_str().unwrap(), SYSFS_ONLINE_FILE);
            info!(logger, "{}", file.as_str());

            let c = fs::read_to_string(file.as_str());
            if c.is_err() {
                continue;
            }
            let c = c.unwrap();

            if c.trim().contains("0") {
                let r = fs::write(file.as_str(), "1");
                if r.is_err() {
                    continue;
                }
                count += 1;

                if num > 0 && count == num {
                    break;
                }
            }
        }
    }

    if num > 0 {
        return Ok(count);
    }

    Ok(0)
}

// max wait for all CPUs to online will use 50 * 100 = 5 seconds.
const ONLINE_CPUMEM_WATI_MILLIS: u64 = 50;
const ONLINE_CPUMEM_MAX_RETRIES: u32 = 100;

fn online_cpus(logger: &Logger, num: i32) -> Result<i32> {
    let mut onlined_count: i32 = 0;

    for i in 0..ONLINE_CPUMEM_MAX_RETRIES {
        let r = online_resources(
            logger,
            SYSFS_CPU_ONLINE_PATH,
            r"cpu[0-9]+",
            (num - onlined_count),
        );
        if r.is_err() {
            return r;
        }

        onlined_count += r.unwrap();
        if onlined_count == num {
            info!(logger, "online {} CPU(s) after {} retries", num, i);
            return Ok(num);
        }
        thread::sleep(time::Duration::from_millis(ONLINE_CPUMEM_WATI_MILLIS));
    }

    Err(anyhow!(
        "failed to online {} CPU(s) after {} retries",
        num,
        ONLINE_CPUMEM_MAX_RETRIES
    ))
}

fn online_memory(logger: &Logger) -> Result<()> {
    online_resources(logger, SYSFS_MEMORY_ONLINE_PATH, r"memory[0-9]+", -1)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    //use rustjail::Error;
    use super::Sandbox;
    use crate::{mount::BareMount, skip_if_not_root};
    use anyhow::Error;
    use nix::mount::MsFlags;
    use oci::{Linux, Root, Spec};
    use rustjail::container::LinuxContainer;
    use rustjail::specconv::CreateOpts;
    use slog::Logger;
    use std::fs::{self, File};
    use std::os::unix::fs::PermissionsExt;
    use tempfile::Builder;

    fn bind_mount(src: &str, dst: &str, logger: &Logger) -> Result<(), Error> {
        let baremount = BareMount::new(src, dst, "bind", MsFlags::MS_BIND, "", &logger);
        baremount.mount()
    }

    #[test]
    fn set_sandbox_storage() {
        let logger = slog::Logger::root(slog::Discard, o!());
        let mut s = Sandbox::new(&logger).unwrap();

        let tmpdir = Builder::new().tempdir().unwrap();
        let tmpdir_path = tmpdir.path().to_str().unwrap();

        // Add a new sandbox storage
        let new_storage = s.set_sandbox_storage(&tmpdir_path);

        // Check the reference counter
        let ref_count = s.storages[tmpdir_path];
        assert_eq!(
            ref_count, 1,
            "Invalid refcount, got {} expected 1.",
            ref_count
        );
        assert_eq!(new_storage, true);

        // Use the existing sandbox storage
        let new_storage = s.set_sandbox_storage(&tmpdir_path);
        assert_eq!(new_storage, false, "Should be false as already exists.");

        // Since we are using existing storage, the reference counter
        // should be 2 by now.
        let ref_count = s.storages[tmpdir_path];
        assert_eq!(
            ref_count, 2,
            "Invalid refcount, got {} expected 2.",
            ref_count
        );
    }

    #[test]
    fn remove_sandbox_storage() {
        skip_if_not_root!();

        let logger = slog::Logger::root(slog::Discard, o!());
        let s = Sandbox::new(&logger).unwrap();

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

        let emptydir = Builder::new()
            .prefix("empty")
            .tempdir_in(tmpdir_path)
            .unwrap();

        assert!(
            s.remove_sandbox_storage(&srcdir_path).is_err(),
            "Expect Err as the directory i not a mountpoint"
        );

        assert!(s.remove_sandbox_storage("").is_err());

        let invalid_dir = emptydir.path().join("invalid");

        assert!(s
            .remove_sandbox_storage(invalid_dir.to_str().unwrap())
            .is_err());

        // Now, create a double mount as this guarantees the directory cannot
        // be deleted after the first umount.
        for _i in 0..2 {
            assert!(bind_mount(srcdir_path, destdir_path, &logger).is_ok());
        }

        assert!(
            s.remove_sandbox_storage(destdir_path).is_err(),
            "Expect fail as deletion cannot happen due to the second mount."
        );

        // This time it should work as the previous two calls have undone the double
        // mount.
        assert!(s.remove_sandbox_storage(destdir_path).is_ok());
    }

    #[test]
    #[allow(unused_assignments)]
    fn unset_and_remove_sandbox_storage() {
        skip_if_not_root!();

        let logger = slog::Logger::root(slog::Discard, o!());
        let mut s = Sandbox::new(&logger).unwrap();

        // FIX: This test fails, not sure why yet.
        assert!(
            s.unset_and_remove_sandbox_storage("/tmp/testEphePath")
                .is_err(),
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

        assert_eq!(s.set_sandbox_storage(&destdir_path), true);
        assert!(s.unset_and_remove_sandbox_storage(&destdir_path).is_ok());

        let mut other_dir_str = String::new();
        {
            // Create another folder in a separate scope to ensure that is
            // deleted
            let other_dir = Builder::new()
                .prefix("dir")
                .tempdir_in(tmpdir_path)
                .unwrap();
            let other_dir_path = other_dir.path().to_str().unwrap();
            other_dir_str = other_dir_path.to_string();

            assert_eq!(s.set_sandbox_storage(&other_dir_path), true);
        }

        assert!(s.unset_and_remove_sandbox_storage(&other_dir_str).is_err());
    }

    #[test]
    fn unset_sandbox_storage() {
        let logger = slog::Logger::root(slog::Discard, o!());
        let mut s = Sandbox::new(&logger).unwrap();

        let storage_path = "/tmp/testEphe";

        // Add a new sandbox storage
        assert_eq!(s.set_sandbox_storage(&storage_path), true);
        // Use the existing sandbox storage
        assert_eq!(
            s.set_sandbox_storage(&storage_path),
            false,
            "Expects false as the storage is not new."
        );

        assert_eq!(
            s.unset_sandbox_storage(&storage_path).unwrap(),
            false,
            "Expects false as there is still a storage."
        );

        // Reference counter should decrement to 1.
        let ref_count = s.storages[storage_path];
        assert_eq!(
            ref_count, 1,
            "Invalid refcount, got {} expected 1.",
            ref_count
        );

        assert_eq!(
            s.unset_sandbox_storage(&storage_path).unwrap(),
            true,
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
            s.unset_sandbox_storage(&storage_path).is_err(),
            "Expects false as the reference counter should no exist."
        );
    }

    fn create_dummy_opts() -> CreateOpts {
        let mut root = Root::default();
        root.path = String::from("/");

        let linux = Linux::default();
        let mut spec = Spec::default();
        spec.root = Some(root).into();
        spec.linux = Some(linux).into();

        CreateOpts {
            cgroup_name: "".to_string(),
            use_systemd_cgroup: false,
            no_pivot_root: false,
            no_new_keyring: false,
            spec: Some(spec),
            rootless_euid: false,
            rootless_cgroup: false,
        }
    }

    fn create_linuxcontainer() -> LinuxContainer {
        LinuxContainer::new(
            "some_id",
            "/run/agent",
            create_dummy_opts(),
            &slog_scope::logger(),
        )
        .unwrap()
    }

    #[test]
    fn get_container_entry_exist() {
        skip_if_not_root!();
        let logger = slog::Logger::root(slog::Discard, o!());
        let mut s = Sandbox::new(&logger).unwrap();
        let linux_container = create_linuxcontainer();

        s.containers
            .insert("testContainerID".to_string(), linux_container);
        let cnt = s.get_container("testContainerID");
        assert!(cnt.is_some());
    }

    #[test]
    fn get_container_no_entry() {
        let logger = slog::Logger::root(slog::Discard, o!());
        let mut s = Sandbox::new(&logger).unwrap();

        let cnt = s.get_container("testContainerID");
        assert!(cnt.is_none());
    }

    #[test]
    fn add_and_get_container() {
        skip_if_not_root!();
        let logger = slog::Logger::root(slog::Discard, o!());
        let mut s = Sandbox::new(&logger).unwrap();
        let linux_container = create_linuxcontainer();

        s.add_container(linux_container);
        assert!(s.get_container("some_id").is_some());
    }
    #[test]
    fn update_shared_pidns() {
        skip_if_not_root!();
        let logger = slog::Logger::root(slog::Discard, o!());
        let mut s = Sandbox::new(&logger).unwrap();
        let test_pid = 9999;

        let mut linux_container = create_linuxcontainer();
        linux_container.init_process_pid = test_pid;

        s.update_shared_pidns(&linux_container).unwrap();

        assert!(s.sandbox_pidns.is_some());

        let ns_path = format!("/proc/{}/ns/pid", test_pid);
        assert_eq!(s.sandbox_pidns.unwrap().path, ns_path);
    }
    #[test]
    fn add_guest_hooks() {
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
        assert!(s.hooks.as_ref().unwrap().prestart.len() == 1);
        assert!(s.hooks.as_ref().unwrap().poststart.is_empty());
        assert!(s.hooks.as_ref().unwrap().poststop.is_empty());
    }
}
