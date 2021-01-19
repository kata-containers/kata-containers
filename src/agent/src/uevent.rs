// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::device::online_device;
use crate::linux_abi::*;
use crate::sandbox::Sandbox;
use crate::GLOBAL_DEVICE_WATCHER;
use slog::Logger;

use netlink_sys::{Protocol, Socket, SocketAddr};
use nix::errno::Errno;
use std::os::unix::io::FromRawFd;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Default)]
struct Uevent {
    action: String,
    devpath: String,
    devname: String,
    subsystem: String,
    seqnum: String,
    interface: String,
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
            && self.devname != ""
    }

    async fn handle_block_add_event(&self, sandbox: &Arc<Mutex<Sandbox>>) {
        let pci_root_bus_path = create_pci_root_bus_path();

        // Keep the same lock order as device::get_device_name(), otherwise it may cause deadlock.
        let watcher = GLOBAL_DEVICE_WATCHER.clone();
        let mut w = watcher.lock().await;
        let mut sb = sandbox.lock().await;

        // Add the device node name to the pci device map.
        sb.pci_device_map
            .insert(self.devpath.clone(), self.devname.clone());

        // Notify watchers that are interested in the udev event.
        // Close the channel after watcher has been notified.
        let devpath = self.devpath.clone();
        let empties: Vec<_> = w
            .iter_mut()
            .filter(|(dev_addr, _)| {
                let pci_p = format!("{}/{}", pci_root_bus_path, *dev_addr);

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
            .map(|(k, sender)| {
                let devname = self.devname.clone();
                let sender = sender.take().unwrap();
                let _ = sender.send(devname);
                k.clone()
            })
            .collect();

        // Remove notified nodes from the watcher map.
        for empty in empties {
            w.remove(&empty);
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

pub async fn watch_uevents(sandbox: Arc<Mutex<Sandbox>>) {
    let sref = sandbox.clone();
    let s = sref.lock().await;
    let logger = s.logger.new(o!("subsystem" => "uevent"));

    tokio::spawn(async move {
        let mut socket;
        unsafe {
            let fd = libc::socket(
                libc::AF_NETLINK,
                libc::SOCK_DGRAM | libc::SOCK_CLOEXEC,
                Protocol::KObjectUevent as libc::c_int,
            );
            socket = Socket::from_raw_fd(fd);
        }
        socket.bind(&SocketAddr::new(0, 1)).unwrap();

        loop {
            match socket.recv_from_full().await {
                Err(e) => {
                    error!(logger, "receive uevent message failed"; "error" => format!("{}", e))
                }
                Ok((buf, addr)) => {
                    if addr.port_number() != 0 {
                        // not our netlink message
                        let err_msg = format!("{:?}", nix::Error::Sys(Errno::EBADMSG));
                        error!(logger, "receive uevent message failed"; "error" => err_msg);
                        return;
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
    });
}
