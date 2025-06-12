// Copyright (c) 2024 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

//#![deny(warnings)]
use agent::sock::{self, Listener};
use anyhow::{anyhow, Result};
use slog::info;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::net::UnixStream;
use tokio::sync::Notify;

#[derive(Debug)]
pub struct SockPort {
    sockt_port: u32,
    used_ports: HashSet<u32>,
}

pub const SOCK_BASE_PORT: u32 = 1081;

pub const MAX_RETRY: u32 = 32;

pub trait Operation {
    fn get(&mut self) -> Option<u32>;
    #[allow(dead_code)]
    fn put(&mut self, port: u32);
}

impl Operation for SockPort {
    fn get(&mut self) -> Option<u32> {
        let mut value = self.sockt_port;

        for _i in 1..MAX_RETRY {
            if value == u32::MAX {
                value = SOCK_BASE_PORT;
            }

            if !self.used_ports.contains(&value) {
                self.sockt_port = value;
                return Some(value);
            }

            value += 1;
        }

        None
    }

    fn put(&mut self, port: u32) {
        self.used_ports.remove(&port);
        self.sockt_port = port;
    }
}

#[derive(Debug)]
pub struct UdsShare {
    uds_map: HashMap<String, (Arc<Notify>, u32)>,
    sock_port: SockPort,
}

impl UdsShare {
    pub fn new() -> UdsShare {
        UdsShare {
            uds_map: HashMap::new(),
            sock_port: SockPort {
                sockt_port: SOCK_BASE_PORT,
                used_ports: HashSet::new(),
            },
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
                        .insert(volume_src.to_string(), (notifier.clone(), port));

                    let volume = volume_src.to_string();
                    tokio::spawn(async move {
                        loop {
                            match listener {
                                Listener::Unix(ref mut listener) => {
                                    tokio::select! {
                                        new_conn = listener.accept() => {
                                            if let Ok((stream, _addr)) = new_conn {
                                                debug!(sl!(), "share_uds {} got new uds connection from guest", &volume);
                                                copy_stream(stream, &volume).await;
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
                                            if let Ok((stream, _addr)) = new_conn {
                                                debug!(sl!(), "share_uds {} got new vsock connection from guest", &volume);
                                                copy_stream(stream, &volume).await;
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
        if let Some((notifier, port)) = self.uds_map.remove(volume_src) {
            notifier.notify_waiters();
            self.sock_port.put(port);
            Ok(())
        } else {
            Err(anyhow!("the shared uds {} didn't exist", volume_src))
        }
    }
}

async fn copy_stream<T>(mut stream: T, volume: &str)
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    if let Ok(mut hside) = UnixStream::connect(&volume).await {
        let nvolume = volume.to_string();
        tokio::spawn(async move {
            debug!(
                sl!(),
                "do iocopy for share_uds:{} between guest and host", &nvolume
            );
            let res = tokio::io::copy_bidirectional(&mut stream, &mut hside).await;
            debug!(
                sl!(),
                "io copy terminated for {} with {:?} between guest and host", &nvolume, res
            );
        });
    }
}
