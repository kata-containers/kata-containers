// Copyright (c) 2021 Alibaba Cloud
// Copyright (c) 2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

use crate::spec_info::ContainerType;

#[derive(Debug, Clone, Default)]
pub struct Mount {
    // source specifies the BlockDevice path
    pub source: String,
    // target specify where the rootfs is mounted if it has been mounted
    pub destination: String,
    // type specifies the type of filesystem to mount.
    pub fs_type: String,
    // options specifies zero or more fstab style mount options.
    pub options: Vec<String>,
    // device_id
    pub device_id: String,
}

#[derive(Default, Debug, Clone)]
pub struct Rootfs {
    pub path: String,
    pub mounted: bool,
    pub mounts: Vec<Mount>,
    pub guest_path: String,
}

#[derive(Default, Debug, Clone)]
pub struct Config {
    // container id
    pub id: String,

    // container rootfs
    pub rootfs: Option<Rootfs>,

    // mounts describe container mountinfo
    pub mounts: Vec<Mount>,

    // bundle_path
    pub bundle_path: String,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum State {
    Ready,
    Running,
    Stopped,
    Paused,
}

#[derive(Debug)]
pub struct CommonProcess {
    pub id: String,
    pub state: State,
    pub status: Arc<RwLock<ExitStatus>>,
    pub stdin: Option<String>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub terminal: bool,
}

#[derive(Debug)]
pub struct Exec {
    pub common_process: CommonProcess,
    pub state: State,
}

#[derive(Debug)]
pub struct ExitStatus {
    pub exit_code: i32,
    pub exit_time: SystemTime,
}

// TODO: just a placeholder here. Details will be released later.
#[derive(Debug)]
pub struct Container {
    pub common_process: CommonProcess,
    pub config: Config,
    pub processes: HashMap<String, Exec>,
    container_type: ContainerType,
}

// TODO: just a placeholder here. Details will be released later.
impl Container {
    pub fn container_type(&self) -> ContainerType {
        self.container_type
    }
}
