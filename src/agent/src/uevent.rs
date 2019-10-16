// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::device::{online_device, ROOT_BUS_PATH, SCSI_BLOCK_SUFFIX, SYSFS_DIR};
use crate::grpc::SYSFS_MEMORY_ONLINE_PATH;
use crate::netlink::{RtnlHandle, NETLINK_UEVENT};
use crate::sandbox::Sandbox;
use crate::GLOBAL_DEVICE_WATCHER;
use std::sync::{Arc, Mutex};
use std::thread;

pub const U_EVENT_ACTION: &'static str = "ACTION";
pub const U_EVENT_DEV_PATH: &'static str = "DEVPATH";
pub const U_EVENT_SUB_SYSTEM: &'static str = "SUBSYSTEM";
pub const U_EVENT_SEQ_NUM: &'static str = "SEQNUM";
pub const U_EVENT_DEV_NAME: &'static str = "DEVNAME";
pub const U_EVENT_INTERFACE: &'static str = "INTERFACE";

#[derive(Debug, Default)]
pub struct Uevent {
    action: String,
    devpath: String,
    devname: String,
    subsystem: String,
    seqnum: String,
    interface: String,
}

fn parse_uevent(message: &str) -> Uevent {
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

pub fn watch_uevents(sandbox: Arc<Mutex<Sandbox>>) {
    let sref = sandbox.clone();
    let s = sref.lock().unwrap();
    let logger = s.logger.new(o!("subsystem" => "uevent"));

    thread::spawn(move || {
        let rtnl = RtnlHandle::new(NETLINK_UEVENT, 1).unwrap();
        loop {
            match rtnl.recv_message() {
                Err(e) => {
                    error!(logger, "receive uevent message failed"; "error" => format!("{}", e))
                }
                Ok(data) => {
                    let text = String::from_utf8(data);
                    match text {
                        Err(e) => {
                            error!(logger, "failed to convert bytes to text"; "error" => format!("{}", e))
                        }
                        Ok(text) => {
                            let event = parse_uevent(&text);
                            info!(logger, "got uevent message"; "event" => format!("{:?}", event));

                            // Check if device hotplug event results in a device node being created.
                            if event.devname != ""
                                && event.devpath.starts_with(ROOT_BUS_PATH)
                                && event.subsystem == "block"
                            {
                                let watcher = GLOBAL_DEVICE_WATCHER.clone();
                                let mut w = watcher.lock().unwrap();

                                let s = sandbox.clone();
                                let mut sb = s.lock().unwrap();

                                // Add the device node name to the pci device map.
                                sb.pci_device_map
                                    .insert(event.devpath.clone(), event.devname.clone());

                                // Notify watchers that are interested in the udev event.
                                // Close the channel after watcher has been notified.

                                let devpath = event.devpath.clone();

                                let empties: Vec<_> = w
                                    .iter()
                                    .filter(|(dev_addr, _)| {
                                        let pci_p = format!("{}/{}", ROOT_BUS_PATH, *dev_addr);

                                        // blk block device
                                        devpath.starts_with(pci_p.as_str()) ||
                                            // scsi block device
                                            {
                                                (*dev_addr).ends_with(SCSI_BLOCK_SUFFIX) &&
                                                devpath.contains(*dev_addr)
                                            }
                                    })
                                    .map(|(k, sender)| {
                                        let devname = event.devname.clone();
                                        let _ = sender.send(devname);
                                        k.clone()
                                    })
                                    .collect();

                                for empty in empties {
                                    w.remove(&empty);
                                }
                            } else {
                                let online_path =
                                    format!("{}/{}/online", SYSFS_DIR, &event.devpath);
                                if online_path.starts_with(SYSFS_MEMORY_ONLINE_PATH) {
                                    // Check memory hotplug and online if possible
                                    match online_device(online_path.as_ref()) {
                                        Ok(_) => (),
                                        Err(e) => error!(
                                            logger,
                                            "failed to online device";
                                            "device" => &event.devpath,
                                            "error" => format!("{}", e),
                                        ),
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    });
}
