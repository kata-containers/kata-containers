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

    // Check whether this is a block device hot-add event.
    fn is_block_add_event(&self) -> bool {
        let pci_root_bus_path = create_pci_root_bus_path();
        self.action == U_EVENT_ACTION_ADD
            && self.subsystem == "block"
            && {
                self.devpath.starts_with(pci_root_bus_path.as_str())
                    || self.devpath.starts_with(ACPI_DEV_PATH) // NVDIMM/PMEM devices
            }
            && !self.devname.is_empty()
    }

    async fn handle_block_add_event(&self, sandbox: &Arc<Mutex<Sandbox>>) {
        let pci_root_bus_path = create_pci_root_bus_path();
        let mut sb = sandbox.lock().await;

        // Record the event by sysfs path
        sb.uevent_map.insert(self.devpath.clone(), self.clone());

        // Notify watchers that are interested in the udev event.
        // Close the channel after watcher has been notified.
        let devpath = self.devpath.clone();
        let keys: Vec<_> = sb
            .dev_watcher
            .keys()
            .filter(|dev_addr| {
                let pci_p = format!("{}{}", pci_root_bus_path, *dev_addr);

                // blk block device
                devpath.starts_with(pci_p.as_str()) ||
                // scsi block device
                {
                    (*dev_addr).ends_with(SCSI_BLOCK_SUFFIX) &&
                        devpath.contains(*dev_addr)
                } ||
                // nvdimm/pmem device
                {
                    let pmem_suffix = format!("/{}/{}", SCSI_BLOCK_SUFFIX, self.devname);
                    devpath.starts_with(ACPI_DEV_PATH) &&
                        devpath.ends_with(pmem_suffix.as_str()) &&
                        dev_addr.ends_with(pmem_suffix.as_str())
                }
            })
            .cloned()
            .collect();

        for k in keys {
            let devname = self.devname.clone();
            // unwrap() is safe because logic above ensures k exists
            // in the map, and it's locked so no-one else can change
            // that
            let sender = sb.dev_watcher.remove(&k).unwrap();
            let _ = sender.send(devname);
        }
    }

    async fn process(&self, logger: &Logger, sandbox: &Arc<Mutex<Sandbox>>) {
        if self.is_block_add_event() {
            return self.handle_block_add_event(sandbox).await;
        } else if self.action == U_EVENT_ACTION_ADD {
            let online_path = format!("{}/{}/online", SYSFS_DIR, &self.devpath);
            // It's a memory hot-add event.
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
