// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use nix::mount::MsFlags;
use nix::sched::{unshare, CloneFlags};
use nix::unistd::{getpid, gettid};
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::thread;

use crate::mount::{BareMount, FLAGS};
use slog::Logger;

//use container::Process;

const PERSISTENT_NS_DIR: &'static str = "/var/run/sandbox-ns";
pub const NSTYPEIPC: &'static str = "ipc";
pub const NSTYPEUTS: &'static str = "uts";
pub const NSTYPEPID: &'static str = "pid";

lazy_static! {
    static ref CLONE_FLAG_TABLE: HashMap<&'static str, CloneFlags> = {
        let mut m = HashMap::new();
        m.insert(NSTYPEIPC, CloneFlags::CLONE_NEWIPC);
        m.insert(NSTYPEUTS, CloneFlags::CLONE_NEWUTS);
        m
    };
}

#[derive(Debug, Default)]
pub struct Namespace {
    pub path: String,
}

pub fn get_current_thread_ns_path(ns_type: &str) -> String {
    format!(
        "/proc/{}/task/{}/ns/{}",
        getpid().to_string(),
        gettid().to_string(),
        ns_type
    )
}

// setup_persistent_ns creates persistent namespace without switchin to it.
// Note, pid namespaces cannot be persisted.
pub fn setup_persistent_ns(logger: Logger, ns_type: &'static str) -> Result<Namespace, String> {
    if let Err(err) = fs::create_dir_all(PERSISTENT_NS_DIR) {
        return Err(err.to_string());
    }

    let ns_path = Path::new(PERSISTENT_NS_DIR);
    let new_ns_path = ns_path.join(ns_type);

    if let Err(err) = File::create(new_ns_path.as_path()) {
        return Err(err.to_string());
    }

    let new_thread = thread::spawn(move || {
        let origin_ns_path = get_current_thread_ns_path(ns_type);
        let _origin_ns_fd = match File::open(Path::new(&origin_ns_path)) {
            Err(err) => return Err(err.to_string()),
            Ok(file) => file.as_raw_fd(),
        };

        // Create a new netns on the current thread.
        let cf = match CLONE_FLAG_TABLE.get(ns_type) {
            None => return Err(format!("Failed to get ns type {}", ns_type).to_string()),
            Some(cf) => cf,
        };

        if let Err(err) = unshare(*cf) {
            return Err(err.to_string());
        }

        // Bind mount the new namespace from the current thread onto the mount point to persist it.
        let source: &str = origin_ns_path.as_str();
        let destination: &str = new_ns_path.as_path().to_str().unwrap_or("none");

        let _recursive = true;
        let _readonly = true;
        let mut flags = MsFlags::empty();

        match FLAGS.get("rbind") {
            Some(x) => {
                let (_, f) = *x;
                flags = flags | f;
            }
            None => (),
        };

        let bare_mount = BareMount::new(source, destination, "none", flags, "", &logger);

        if let Err(err) = bare_mount.mount() {
            return Err(format!(
                "Failed to mount {} to {} with err:{:?}",
                source, destination, err
            ));
        }
        Ok(())
    });

    match new_thread.join() {
        Ok(t) => match t {
            Err(err) => return Err(err),
            Ok(()) => (),
        },
        Err(err) => return Err(format!("Failed to join thread {:?}!", err)),
    }

    let new_ns_path = ns_path.join(ns_type);
    Ok(Namespace {
        path: new_ns_path.into_os_string().into_string().unwrap(),
    })
}
