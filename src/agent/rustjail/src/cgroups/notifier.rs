// Copyright (c) 2020 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Result};
use eventfd::{eventfd, EfdFlags};
use nix::sys::eventfd;
use nix::sys::inotify::{AddWatchFlags, InitFlags, Inotify};
use std::fs::{self, File};
use std::io::Read;
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;

// Convenience macro to obtain the scope logger
macro_rules! sl {
    () => {
        slog_scope::logger().new(o!("subsystem" => "cgroups_notifier"))
    };
}

pub fn notify_oom(cid: &str, cg_dir: String) -> Result<Receiver<String>> {
    if cgroups::hierarchies::is_cgroup2_unified_mode() {
        return notify_on_oom_v2(cid, cg_dir);
    }
    notify_on_oom(cid, cg_dir)
}

// get_value_from_cgroup parse cgroup file with `Flat keyed`
// and get the value of `key`.
// Flat keyed file format:
//   KEY0 VAL0\n
//   KEY1 VAL1\n
fn get_value_from_cgroup(path: &PathBuf, key: &str) -> Result<i64> {
    let content = fs::read_to_string(path)?;
    info!(
        sl!(),
        "get_value_from_cgroup file: {:?}, content: {}", &path, &content
    );

    for line in content.lines() {
        let arr: Vec<&str> = line.split(" ").collect();
        if arr.len() == 2 && arr[0] == key {
            let r = arr[1].parse::<i64>()?;
            return Ok(r);
        }
    }
    Ok(0)
}

// notify_on_oom returns channel on which you can expect event about OOM,
// if process died without OOM this channel will be closed.
pub fn notify_on_oom_v2(containere_id: &str, cg_dir: String) -> Result<Receiver<String>> {
    register_memory_event_v2(containere_id, cg_dir, "memory.events", "cgroup.events")
}

fn register_memory_event_v2(
    containere_id: &str,
    cg_dir: String,
    memory_event_name: &str,
    cgroup_event_name: &str,
) -> Result<Receiver<String>> {
    let event_control_path = Path::new(&cg_dir).join(memory_event_name);
    let cgroup_event_control_path = Path::new(&cg_dir).join(cgroup_event_name);
    info!(
        sl!(),
        "register_memory_event_v2 event_control_path: {:?}", &event_control_path
    );
    info!(
        sl!(),
        "register_memory_event_v2 cgroup_event_control_path: {:?}", &cgroup_event_control_path
    );

    let fd = Inotify::init(InitFlags::empty()).unwrap();

    // watching oom kill
    let ev_fd = fd
        .add_watch(&event_control_path, AddWatchFlags::IN_MODIFY)
        .unwrap();
    // Because no `unix.IN_DELETE|unix.IN_DELETE_SELF` event for cgroup file system, so watching all process exited
    let cg_fd = fd
        .add_watch(&cgroup_event_control_path, AddWatchFlags::IN_MODIFY)
        .unwrap();
    info!(sl!(), "ev_fd: {:?}", ev_fd);
    info!(sl!(), "cg_fd: {:?}", cg_fd);

    let (sender, receiver) = mpsc::channel();
    let containere_id = containere_id.to_string();

    thread::spawn(move || {
        loop {
            let events = fd.read_events().unwrap();
            info!(
                sl!(),
                "container[{}] get events for container: {:?}", &containere_id, &events
            );

            for event in events {
                if event.mask & AddWatchFlags::IN_MODIFY != AddWatchFlags::IN_MODIFY {
                    continue;
                }
                info!(sl!(), "event.wd: {:?}", event.wd);

                if event.wd == ev_fd {
                    let oom = get_value_from_cgroup(&event_control_path, "oom_kill");
                    if oom.unwrap_or(0) > 0 {
                        sender.send(containere_id.clone()).unwrap();
                        return;
                    }
                } else if event.wd == cg_fd {
                    let pids = get_value_from_cgroup(&cgroup_event_control_path, "populated");
                    if pids.unwrap_or(-1) == 0 {
                        return;
                    }
                }
            }
            // When a cgroup is destroyed, an event is sent to eventfd.
            // So if the control path is gone, return instead of notifying.
            if !Path::new(&event_control_path).exists() {
                return;
            }
        }
    });

    Ok(receiver)
}

// notify_on_oom returns channel on which you can expect event about OOM,
// if process died without OOM this channel will be closed.
fn notify_on_oom(cid: &str, dir: String) -> Result<Receiver<String>> {
    if dir == "" {
        return Err(anyhow!("memory controller missing"));
    }

    register_memory_event(cid, dir, "memory.oom_control", "")
}

// level is one of "low", "medium", or "critical"
fn notify_memory_pressure(cid: &str, dir: String, level: &str) -> Result<Receiver<String>> {
    if dir == "" {
        return Err(anyhow!("memory controller missing"));
    }

    if level != "low" && level != "medium" && level != "critical" {
        return Err(anyhow!("invalid pressure level {}", level));
    }

    register_memory_event(cid, dir, "memory.pressure_level", level)
}

fn register_memory_event(
    cid: &str,
    cg_dir: String,
    event_name: &str,
    arg: &str,
) -> Result<Receiver<String>> {
    let path = Path::new(&cg_dir).join(event_name);
    let event_file = File::open(path.clone())?;

    let eventfd = eventfd(0, EfdFlags::EFD_CLOEXEC)?;

    let event_control_path = Path::new(&cg_dir).join("cgroup.event_control");
    let data;
    if arg == "" {
        data = format!("{} {}", eventfd, event_file.as_raw_fd());
    } else {
        data = format!("{} {} {}", eventfd, event_file.as_raw_fd(), arg);
    }

    fs::write(&event_control_path, data)?;

    let mut eventfd_file = unsafe { File::from_raw_fd(eventfd) };

    let (sender, receiver) = mpsc::channel();
    let containere_id = cid.to_string();

    thread::spawn(move || {
        loop {
            let mut buf = [0; 8];
            match eventfd_file.read(&mut buf) {
                Err(err) => {
                    warn!(sl!(), "failed to read from eventfd: {:?}", err);
                    return;
                }
                Ok(_) => {
                    let content = fs::read_to_string(path.clone());
                    info!(
                        sl!(),
                        "OOM event for container: {}, content: {:?}", &containere_id, content
                    );
                }
            }

            // When a cgroup is destroyed, an event is sent to eventfd.
            // So if the control path is gone, return instead of notifying.
            if !Path::new(&event_control_path).exists() {
                return;
            }
            sender.send(containere_id.clone()).unwrap();
        }
    });

    Ok(receiver)
}
