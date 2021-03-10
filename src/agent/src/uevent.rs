// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::device::online_device;
use crate::linux_abi::*;
use crate::sandbox::Sandbox;
use slog::Logger;

use anyhow::Result;
use netlink_sys::{protocols, SocketAddr, TokioSocket};
use nix::errno::Errno;
use std::fmt::Debug;
use std::os::unix::io::FromRawFd;
use std::sync::Arc;
use tokio::select;
use tokio::sync::watch::Receiver;
use tokio::sync::Mutex;

#[derive(Debug, Default, Clone)]
pub struct Uevent {
    pub action: String,
    pub devpath: String,
    pub devname: String,
    pub subsystem: String,
    seqnum: String,
    pub interface: String,
}

pub trait UeventMatcher: Sync + Send + Debug + 'static {
    fn is_match(&self, uev: &Uevent) -> bool;
}

impl Uevent {
    fn new(message: &str) -> Self {
        let mut msg_iter = message.split('\0');
        let mut event = Uevent::default();

        msg_iter.next(); // skip the first value
        for arg in msg_iter {
            let key_val: Vec<&str> = arg.splitn(2, '=').collect();
            if key_val.len() == 2 {
                match key_val[0] {
                    U_EVENT_ACTION => event.action = String::from(key_val[1]),
                    U_EVENT_DEV_NAME => event.devname = String::from(key_val[1]),
                    U_EVENT_SUB_SYSTEM => event.subsystem = String::from(key_val[1]),
                    U_EVENT_DEV_PATH => event.devpath = String::from(key_val[1]),
                    U_EVENT_SEQ_NUM => event.seqnum = String::from(key_val[1]),
                    U_EVENT_INTERFACE => event.interface = String::from(key_val[1]),
                    _ => (),
                }
            }
        }

        event
    }

    async fn process_add(&self, logger: &Logger, sandbox: &Arc<Mutex<Sandbox>>) {
        // Special case for memory hot-adds first
        let online_path = format!("{}/{}/online", SYSFS_DIR, &self.devpath);
        if online_path.starts_with(SYSFS_MEMORY_ONLINE_PATH) {
            let _ = online_device(online_path.as_ref()).map_err(|e| {
                error!(
                    *logger,
                    "failed to online device";
                    "device" => &self.devpath,
                    "error" => format!("{}", e),
                )
            });
            return;
        }

        let mut sb = sandbox.lock().await;

        // Record the event by sysfs path
        sb.uevent_map.insert(self.devpath.clone(), self.clone());

        // Notify watchers that are interested in the udev event.
        for watch in &mut sb.uevent_watchers {
            if let Some((matcher, _)) = watch {
                if matcher.is_match(&self) {
                    let (_, sender) = watch.take().unwrap();
                    let _ = sender.send(self.clone());
                }
            }
        }
    }

    async fn process(&self, logger: &Logger, sandbox: &Arc<Mutex<Sandbox>>) {
        if self.action == U_EVENT_ACTION_ADD {
            return self.process_add(logger, sandbox).await;
        }
        debug!(*logger, "ignoring event"; "uevent" => format!("{:?}", self));
    }
}

pub async fn watch_uevents(
    sandbox: Arc<Mutex<Sandbox>>,
    mut shutdown: Receiver<bool>,
) -> Result<()> {
    let sref = sandbox.clone();
    let s = sref.lock().await;
    let logger = s.logger.new(o!("subsystem" => "uevent"));

    // Unlock the sandbox to allow a successful shutdown
    drop(s);

    info!(logger, "starting uevents handler");

    let mut socket;

    unsafe {
        let fd = libc::socket(
            libc::AF_NETLINK,
            libc::SOCK_DGRAM | libc::SOCK_CLOEXEC,
            protocols::NETLINK_KOBJECT_UEVENT as libc::c_int,
        );
        socket = TokioSocket::from_raw_fd(fd);
    }

    socket.bind(&SocketAddr::new(0, 1))?;

    loop {
        select! {
            _ = shutdown.changed() => {
                info!(logger, "got shutdown request");
                break;
            }
            result = socket.recv_from_full() => {
                match result {
                    Err(e) => {
                        error!(logger, "failed to receive uevent"; "error" => format!("{}", e))
                    }
                    Ok((buf, addr)) => {
                        if addr.port_number() != 0 {
                            // not our netlink message
                            let err_msg = format!("{:?}", nix::Error::Sys(Errno::EBADMSG));
                            error!(logger, "receive uevent message failed"; "error" => err_msg);
                            continue;
                        }

                        let text = String::from_utf8(buf);
                        match text {
                            Err(e) => {
                                error!(logger, "failed to convert bytes to text"; "error" => format!("{}", e))
                            }
                            Ok(text) => {
                                let event = Uevent::new(&text);
                                info!(logger, "got uevent message"; "event" => format!("{:?}", event));
                                event.process(&logger, &sandbox).await;
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
