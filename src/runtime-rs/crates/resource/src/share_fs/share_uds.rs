// Copyright (c) 2024 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

//#![deny(warnings)]
use agent::sock::{self, Listener};
use anyhow::{anyhow, Result};
use slog::info;
use std::{collections::HashMap, sync::Arc};
use tokio;
use tokio::net::UnixStream;
use tokio::sync::Notify;

pub type SockPort = u32;
pub const SOCK_BASE_PORT: SockPort = 1080;

pub const MAX_RETRY: u32 = 32;

pub trait Operation {
    fn get(&mut self) -> Option<u32>;
    fn put(&mut self);
}

impl Operation for SockPort {
    fn get(&mut self) -> Option<u32> {
        let value = *self;

        if value == u32::MAX {
            return None;
        }

        *self += 1;

        Some(value)
    }

    fn put(&mut self) {
        if *self > SOCK_BASE_PORT {
            *self -= 1;
        }
    }
}

#[derive(Debug)]
pub struct UdsShare {
    uds_map: HashMap<String, Arc<Notify>>,
    sock_port: SockPort,
}

impl UdsShare {
    pub fn new() -> UdsShare {
        UdsShare {
            uds_map: HashMap::new(),
            sock_port: SOCK_BASE_PORT,
        }
    }

    pub async fn share_uds(&mut self, volume_src: &str, sock_addr: &str) -> Result<Option<u32>> {
        if self.uds_map.contains_key(volume_src) {
            return Ok(None);
        }

        let mut vport;

        for _i in 1..MAX_RETRY {
            if let Some(port) = self.sock_port.get() {
                vport = Some(port);
                let sock = sock::new(sock_addr, port)?;
                if let Ok(mut listener) = sock.listen().await {
                    let notifier = Arc::new(Notify::new());

                    self.uds_map
                        .insert(volume_src.to_string(), notifier.clone());

                    let volume = volume_src.to_string();
                    tokio::spawn(async move {
                        loop {
                            match listener {
                                Listener::Unix(ref mut listener) => {
                                    tokio::select! {
                                        new_conn = listener.accept() => {
                                            if let Ok((mut stream, _addr)) = new_conn {
                                                debug!(sl!(), "share_uds {} got new uds connection from guest", &volume);
                                                if let Ok(mut hside) = UnixStream::connect(&volume).await {
                                                    let nvolume = volume.clone();
                                                    tokio::spawn(async move {
                                                        debug!(sl!(), "do iocopy for share_uds:{} between guest and host", &nvolume);
                                                        let res = tokio::io::copy_bidirectional(&mut stream, &mut hside).await;
                                                        debug!(sl!(), "io copy terminated for {} with {:?} between guest and host", &nvolume, res);
                                                    });
                                                }
                                            }
                                        },
                                        _ = notifier.notified() => {
                                            info!(sl!(), "destroy the share_uds {}", &volume);
                                            break
                                        }
                                    }
                                }
                                Listener::Vsock(ref mut listener) => {
                                    tokio::select! {
                                        new_conn = listener.accept() => {
                                            if let Ok((mut stream, _addr)) = new_conn {
                                                debug!(sl!(), "share_uds {} got new vsock connection from guest", &volume);
                                                    if let Ok(mut hside) = UnixStream::connect(&volume).await {
                                                        let nvolume = volume.clone();
                                                        tokio::spawn(async move {
                                                            debug!(sl!(), "do iocopy for share_uds:{} between guest and host", &nvolume);
                                                            let res = tokio::io::copy_bidirectional(&mut stream, &mut hside).await;
                                                            debug!(sl!(), "io copy terminated for {} with {:?} between guest and host", &nvolume, res);
                                                        });
                                                    }


                                            }
                                        },
                                        _ = notifier.notified() => {
                                            info!(sl!(), "destroy the share_uds {}", &volume);
                                            break
                                        }
                                    }
                                }
                            }
                        }
                    });

                    return Ok(vport);
                }
            }
        }

        Err(anyhow!("failed to share uds"))
    }

    pub async fn cleanup_uds_pass(&mut self, volume_src: &str) -> Result<()> {
        if let Some(notifier) = self.uds_map.remove(volume_src) {
            notifier.notify_waiters();
            Ok(())
        } else {
            Err(anyhow!("the shared uds {} didn't exist", volume_src))
        }
    }
}
