// Copyright (c) 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::{Path, PathBuf};
use std::time::Duration;

use agent::sock::Vsock;
use anyhow::{anyhow, Context, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use url::Url;

const DIAL_RETRY: Duration = Duration::from_secs(2);
const VSOCK_SCHEME: &str = "vsock";

pub(crate) fn guest_cid_from_agent_url(agent_url: &str) -> Result<u32> {
    let url = Url::parse(agent_url).context("parse agent url")?;
    if url.scheme() != VSOCK_SCHEME {
        return Err(anyhow!(
            "vsock UDS forward requires {VSOCK_SCHEME} agent URL, got {agent_url:?}"
        ));
    }

    url.host_str()
        .ok_or_else(|| anyhow!("cannot parse guest CID from agent URL {agent_url:?}"))?
        .parse::<u32>()
        .with_context(|| format!("invalid guest CID in agent URL {agent_url:?}"))
}

pub(crate) struct VsockUdsForward {
    shutdown_tx: watch::Sender<bool>,
    task: JoinHandle<()>,
}

impl VsockUdsForward {
    pub(crate) fn start(guest_cid: u32, port: u32, uds: PathBuf) -> Self {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        info!(
            sl!(),
            "vsock UDS forward: started guest_cid={guest_cid} port={port} uds={}",
            uds.display()
        );
        let task = tokio::spawn(run_dial_loop(guest_cid, port, uds, shutdown_rx));

        Self { shutdown_tx, task }
    }

    pub(crate) async fn stop(self) {
        let _ = self.shutdown_tx.send(true);
        self.task.abort();
        let _ = self.task.await;
    }
}

async fn run_dial_loop(
    guest_cid: u32,
    port: u32,
    uds: PathBuf,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    loop {
        if *shutdown_rx.borrow() {
            return;
        }

        let vsock = Vsock::new(guest_cid, port);
        match vsock.connect_once().await {
            Ok(vsock) => {
                if let Err(err) = bridge(vsock, &uds, &mut shutdown_rx).await {
                    debug!(
                        sl!(),
                        "vsock UDS forward: bridge ended guest_cid={guest_cid} port={port} uds={}: {err:#}",
                        uds.display()
                    );
                }
            }
            Err(err) => {
                debug!(
                    sl!(),
                    "vsock UDS forward: guest vsock dial failed guest_cid={guest_cid} port={port}: {err:#}"
                );
            }
        }

        if sleep_or_shutdown(&mut shutdown_rx, DIAL_RETRY).await {
            return;
        }
    }
}

async fn sleep_or_shutdown(shutdown_rx: &mut watch::Receiver<bool>, dur: Duration) -> bool {
    tokio::select! {
        res = shutdown_rx.wait_for(|v| *v) => res.is_ok(),
        _ = tokio::time::sleep(dur) => false,
    }
}

async fn bridge(
    mut vsock: UnixStream,
    uds_path: &Path,
    shutdown_rx: &mut watch::Receiver<bool>,
) -> Result<()> {
    let mut first = [0u8; 1];
    let n = match vsock.read(&mut first).await {
        Ok(0) => return Ok(()),
        Ok(n) => n,
        Err(err) => {
            debug!(
                sl!(),
                "vsock UDS forward: guest vsock closed before first byte uds={}: {err:#}",
                uds_path.display()
            );
            return Ok(());
        }
    };

    let mut uds = match UnixStream::connect(uds_path).await {
        Ok(stream) => stream,
        Err(err) => {
            warn!(
                sl!(),
                "vsock UDS forward: unix dial failed uds={}: {err:#}",
                uds_path.display()
            );
            return Ok(());
        }
    };

    if let Err(err) = uds.write_all(&first[..n]).await {
        warn!(
            sl!(),
            "vsock UDS forward: failed to write first byte to unix socket uds={}: {err:#}",
            uds_path.display()
        );
        let _ = uds.shutdown().await;
        return Ok(());
    }

    // When either leg finishes (guest vsock EOF or host UDS close), tear down both
    // sides so the bridge returns and the dial loop can accept the next guest session.
    let (mut v_read, mut v_write) = vsock.into_split();
    let (mut u_read, mut u_write) = uds.into_split();

    let guest_to_host = tokio::spawn(async move {
        tokio::io::copy(&mut v_read, &mut u_write).await
    });
    let host_to_guest = tokio::spawn(async move {
        tokio::io::copy(&mut u_read, &mut v_write).await
    });
    let guest_to_host_abort = guest_to_host.abort_handle();
    let host_to_guest_abort = host_to_guest.abort_handle();

    tokio::pin! {
        let guest_to_host = guest_to_host;
        let host_to_guest = host_to_guest;
    }

    enum BridgeEnd {
        Shutdown,
        GuestToHost,
        HostToGuest,
    }

    let bridge_end = tokio::select! {
        _ = shutdown_rx.wait_for(|v| *v) => {
            guest_to_host_abort.abort();
            host_to_guest_abort.abort();
            BridgeEnd::Shutdown
        }
        guest_result = &mut guest_to_host => {
            host_to_guest_abort.abort();
            let _ = guest_result;
            BridgeEnd::GuestToHost
        }
        host_result = &mut host_to_guest => {
            guest_to_host_abort.abort();
            let _ = host_result;
            BridgeEnd::HostToGuest
        }
    };

    match bridge_end {
        BridgeEnd::Shutdown => {
            let _ = guest_to_host.await;
            let _ = host_to_guest.await;
        }
        BridgeEnd::GuestToHost => {
            let _ = host_to_guest.await;
        }
        BridgeEnd::HostToGuest => {
            let _ = guest_to_host.await;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::guest_cid_from_agent_url;

    #[test]
    fn test_guest_cid_from_agent_url() {
        assert_eq!(guest_cid_from_agent_url("vsock://3:1024").unwrap(), 3);
        assert_eq!(guest_cid_from_agent_url("vsock://16187:1024").unwrap(), 16187);
        assert!(guest_cid_from_agent_url("hvsock:///tmp/kata.hvsock:1024").is_err());
    }
}
