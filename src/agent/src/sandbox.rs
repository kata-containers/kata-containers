// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

//use crate::container::Container;
use crate::mount::{get_mount_fs_type, remove_mounts, TYPEROOTFS};
use crate::namespace::{setup_persistent_ns, Namespace, NSTYPEIPC, NSTYPEUTS};
use crate::netlink::{RtnlHandle, NETLINK_ROUTE};
use crate::network::Network;
use libc::pid_t;
use protocols::agent::OnlineCPUMemRequest;
use regex::Regex;
use rustjail::cgroups;
use rustjail::container::BaseContainer;
use rustjail::container::LinuxContainer;
use rustjail::errors::*;
use rustjail::process::Process;
use slog::Logger;
use std::collections::HashMap;
use std::fs;
use std::sync::mpsc::Sender;

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
    pub storages: HashMap<String, u32>,
    pub running: bool,
    pub no_pivot_root: bool,
    enable_grpc_trace: bool,
    pub sandbox_pid_ns: bool,
    pub sender: Option<Sender<i32>>,
    pub rtnl: Option<RtnlHandle>,
}

impl Sandbox {
    pub fn new(logger: &Logger) -> Result<Self> {
        let fs_type = get_mount_fs_type("/")?;
        let logger = logger.new(o!("subsystem" => "sandbox"));

        Ok(Sandbox {
            logger: logger,
            id: "".to_string(),
            hostname: "".to_string(),
            network: Network::new(),
            containers: HashMap::new(),
            mounts: Vec::new(),
            container_mounts: HashMap::new(),
            pci_device_map: HashMap::new(),
            shared_utsns: Namespace {
                path: "".to_string(),
            },
            shared_ipcns: Namespace {
                path: "".to_string(),
            },
            storages: HashMap::new(),
            running: false,
            no_pivot_root: fs_type.eq(TYPEROOTFS),
            enable_grpc_trace: false,
            sandbox_pid_ns: false,
            sender: None,
            rtnl: Some(RtnlHandle::new(NETLINK_ROUTE, 0).unwrap()),
        })
    }

    // unset_sandbox_storage will decrement the sandbox storage
    // reference counter. If there aren't any containers using
    // that sandbox storage, this method will remove the
    // storage reference from the sandbox and return 'true, nil' to
    // let the caller know that they can clean up the storage
    // related directories by calling remove_sandbox_storage
    //
    // It's assumed that caller is calling this method after
    // acquiring a lock on sandbox.
    pub fn unset_sandbox_storage(&mut self, path: &str) -> bool {
        match self.storages.get_mut(path) {
            None => return false,
            Some(count) => {
                *count -= 1;
                if *count < 1 {
                    self.storages.remove(path);
                }
                return true;
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
        fs::remove_dir_all(path)?;
        Ok(())
    }

    // unset_and_remove_sandbox_storage unsets the storage from sandbox
    // and if there are no containers using this storage it will
    // remove it from the sandbox.
    //
    // It's assumed that caller is calling this method after
    // acquiring a lock on sandbox.
    pub fn unset_and_remove_sandbox_storage(&mut self, path: &str) -> Result<()> {
        if self.unset_sandbox_storage(path) {
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
        self.shared_ipcns = match setup_persistent_ns(self.logger.clone(), NSTYPEIPC) {
            Ok(ns) => ns,
            Err(err) => {
                return Err(ErrorKind::ErrorCode(format!(
                    "Failed to setup persisten IPC namespace with error: {}",
                    &err
                ))
                .into())
            }
        };

        // Set up shared UTS namespace
        self.shared_utsns = match setup_persistent_ns(self.logger.clone(), NSTYPEUTS) {
            Ok(ns) => ns,
            Err(err) => {
                return Err(ErrorKind::ErrorCode(format!(
                    "Failed to setup persisten UTS namespace with error: {} ",
                    &err
                ))
                .into())
            }
        };

        Ok(true)
    }

    pub fn add_container(&mut self, c: LinuxContainer) {
        self.containers.insert(c.id.clone(), c);
    }

    pub fn get_container(&mut self, id: &str) -> Option<&mut LinuxContainer> {
        self.containers.get_mut(id)
    }

    pub fn find_process<'a>(&'a mut self, pid: pid_t) -> Option<&'a mut Process> {
        for (_, c) in self.containers.iter_mut() {
            if c.processes.get(&pid).is_some() {
                return c.processes.get_mut(&pid);
            }
        }

        None
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

        let cpuset = cgroups::fs::get_guest_cpuset()?;

        for (_, ctr) in self.containers.iter() {
            info!(self.logger, "updating {}", ctr.id.as_str());
            ctr.cgroup_manager
                .as_ref()
                .unwrap()
                .update_cpuset_path(cpuset.as_str())?;
        }

        Ok(())
    }
}

pub const CPU_ONLINE_PATH: &'static str = "/sys/devices/system/cpu";
pub const MEMORY_ONLINE_PATH: &'static str = "/sys/devices/system/memory";
pub const ONLINE_FILE: &'static str = "online";

fn online_resources(logger: &Logger, path: &str, pattern: &str, num: i32) -> Result<i32> {
    let mut count = 0;
    let re = Regex::new(pattern)?;

    for e in fs::read_dir(path)? {
        let entry = e?;
        let tmpname = entry.file_name();
        let name = tmpname.to_str().unwrap();
        let p = entry.path();

        if re.is_match(name) {
            let file = format!("{}/{}", p.to_str().unwrap(), ONLINE_FILE);
            info!(logger, "{}", file.as_str());
            let c = fs::read_to_string(file.as_str())?;

            if c.trim().contains("0") {
                fs::write(file.as_str(), "1")?;
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

fn online_cpus(logger: &Logger, num: i32) -> Result<i32> {
    online_resources(logger, CPU_ONLINE_PATH, r"cpu[0-9]+", num)
}

fn online_memory(logger: &Logger) -> Result<()> {
    online_resources(logger, MEMORY_ONLINE_PATH, r"memory[0-9]+", -1)?;
    Ok(())
}
