// Copyright (c) 2020 Ant Group
//
// SPDX-License-Identifier: Apache-2.0 or MIT
//

use eventfd::{eventfd, EfdFlags};
use nix::sys::eventfd;
use std::fs::{self, File};
use std::io::Read;
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use crate::error::ErrorKind::*;
use crate::error::*;

// notify_on_oom returns channel on which you can expect event about OOM,
// if process died without OOM this channel will be closed.
pub fn notify_on_oom_v2(key: &str, dir: &PathBuf) -> Result<Receiver<String>> {
    register_memory_event(key, dir, "memory.oom_control", "")
}

// notify_on_oom returns channel on which you can expect event about OOM,
// if process died without OOM this channel will be closed.
pub fn notify_on_oom_v1(key: &str, dir: &PathBuf) -> Result<Receiver<String>> {
    register_memory_event(key, dir, "memory.oom_control", "")
}

// level is one of "low", "medium", or "critical"
pub fn notify_memory_pressure(key: &str, dir: &PathBuf, level: &str) -> Result<Receiver<String>> {
    if level != "low" && level != "medium" && level != "critical" {
        return Err(Error::from_string(format!(
            "invalid pressure level {}",
            level
        )));
    }

    register_memory_event(key, dir, "memory.pressure_level", level)
}

fn register_memory_event(
    key: &str,
    cg_dir: &PathBuf,
    event_name: &str,
    arg: &str,
) -> Result<Receiver<String>> {
    let path = cg_dir.join(event_name);
    let event_file = File::open(path).map_err(|e| Error::with_cause(ReadFailed, e))?;

    let eventfd =
        eventfd(0, EfdFlags::EFD_CLOEXEC).map_err(|e| Error::with_cause(ReadFailed, e))?;

    let event_control_path = cg_dir.join("cgroup.event_control");
    let data;
    if arg == "" {
        data = format!("{} {}", eventfd, event_file.as_raw_fd());
    } else {
        data = format!("{} {} {}", eventfd, event_file.as_raw_fd(), arg);
    }

    // write to file and set mode to 0700(FIXME)
    fs::write(&event_control_path, data).map_err(|e| Error::with_cause(WriteFailed, e))?;

    let mut eventfd_file = unsafe { File::from_raw_fd(eventfd) };

    let (sender, receiver) = mpsc::channel();
    let key = key.to_string();

    thread::spawn(move || {
        loop {
            let mut buf = [0; 8];
            match eventfd_file.read(&mut buf) {
                Err(_err) => {
                    return;
                }
                Ok(_) => {}
            }

            // When a cgroup is destroyed, an event is sent to eventfd.
            // So if the control path is gone, return instead of notifying.
            if !Path::new(&event_control_path).exists() {
                return;
            }
            sender.send(key.clone()).unwrap();
        }
    });

    Ok(receiver)
}
