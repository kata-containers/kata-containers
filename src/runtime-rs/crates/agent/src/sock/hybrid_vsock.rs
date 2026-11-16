// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::time::Duration;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
    time::Instant,
};

use super::{ConnectConfig, HandshakeReplayPolicy, Sock, Stream};

#[derive(Debug)]
enum HandshakeError {
    PeerClosedBeforeResponse,
    Committed(anyhow::Error),
}

impl HandshakeError {
    fn is_replayable(&self, policy: HandshakeReplayPolicy) -> bool {
        matches!(self, Self::PeerClosedBeforeResponse)
            && policy == HandshakeReplayPolicy::PeerClosedBeforeResponse
    }
}

#[derive(Debug, PartialEq)]
pub struct HybridVsock {
    uds: String,
    port: u32,
}

impl HybridVsock {
    pub fn new(uds: &str, port: u32) -> Self {
        Self {
            uds: uds.to_string(),
            port,
        }
    }

    async fn connect_with_budget(
        &self,
        initial_retry_backoff: Duration,
        reconnect_budget: Duration,
        handshake_replay: HandshakeReplayPolicy,
    ) -> Result<Stream> {
        let deadline = Instant::now() + reconnect_budget;
        let mut retry_backoff = initial_retry_backoff.max(Duration::from_millis(1));
        let mut attempts = 0_u64;
        let mut last_err = None;

        loop {
            let now = Instant::now();
            if now >= deadline {
                return Err(anyhow!(
                    "hybrid vsock: failed to connect UDS for {:?} within {:?} (attempts={}), last_err={:?}",
                    self,
                    reconnect_budget,
                    attempts,
                    last_err
                ));
            }

            attempts += 1;
            let stream = match tokio::time::timeout(deadline - now, UnixStream::connect(&self.uds))
                .await
            {
                Ok(Ok(stream)) => stream,
                Ok(Err(err)) => {
                    let err = anyhow!(err).context("connect UDS");
                    warn!(
                        sl!(),
                        "hybrid vsock: UDS connect attempt {} failed for {:?}: {:#}",
                        attempts,
                        self,
                        err
                    );
                    last_err = Some(err);
                    let now = Instant::now();
                    if now >= deadline {
                        return Err(anyhow!(
                            "hybrid vsock: failed to connect UDS for {:?} within {:?} (attempts={}), last_err={:?}",
                            self,
                            reconnect_budget,
                            attempts,
                            last_err
                        ));
                    }
                    tokio::time::sleep(retry_backoff.min(deadline - now)).await;
                    retry_backoff = retry_backoff.saturating_mul(2);
                    continue;
                }
                Err(err) => {
                    return Err(anyhow!(
                        "hybrid vsock: UDS connect to {:?} timed out after {:?}: {}",
                        self,
                        reconnect_budget,
                        err
                    ));
                }
            };

            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or_else(|| {
                    anyhow!(
                        "hybrid vsock: failed to connect UDS for {:?} within {:?} (attempts={}), last_err={:?}",
                        self,
                        reconnect_budget,
                        attempts,
                        last_err
                    )
                })?;

            match tokio::time::timeout(remaining, handshake(stream, self.port)).await {
                Ok(Ok((stream, local_port))) => {
                    info!(
                        sl!(),
                        "hybrid vsock: connected to {:?} after {} UDS attempt(s)", self, attempts
                    );
                    return Ok(Stream::Unix {
                        stream,
                        hvsock_local_port: Some(local_port),
                    });
                }
                Ok(Err(err)) if err.is_replayable(handshake_replay) => {
                    let err = anyhow!("peer closed before hybrid-vsock handshake response");
                    warn!(
                        sl!(),
                        "hybrid vsock: replayable handshake attempt {} failed for {:?}: {:#}",
                        attempts,
                        self,
                        err
                    );
                    last_err = Some(err);
                }
                Ok(Err(HandshakeError::PeerClosedBeforeResponse)) => {
                    return Err(anyhow!(
                        "hybrid vsock: peer closed committed handshake for {:?} before any response",
                        self
                    ));
                }
                Ok(Err(HandshakeError::Committed(err))) => {
                    return Err(err.context(format!(
                        "hybrid vsock: committed handshake failed for {:?}",
                        self
                    )));
                }
                Err(err) => {
                    return Err(anyhow!(
                        "hybrid vsock: committed handshake for {:?} timed out after {:?}: {}",
                        self,
                        remaining,
                        err
                    ));
                }
            }

            let now = Instant::now();
            if now >= deadline {
                return Err(anyhow!(
                    "hybrid vsock: failed to connect to {:?} within {:?} (attempts={}), last_err={:?}",
                    self,
                    reconnect_budget,
                    attempts,
                    last_err
                ));
            }
            tokio::time::sleep(retry_backoff.min(deadline - now)).await;
            retry_backoff = retry_backoff.saturating_mul(2);
        }
    }
}

#[async_trait]
impl Sock for HybridVsock {
    async fn connect(&self, config: &ConnectConfig) -> Result<Stream> {
        self.connect_with_budget(
            Duration::from_millis(config.dial_timeout_ms),
            Duration::from_millis(config.reconnect_timeout_ms),
            config.handshake_replay,
        )
        .await
    }

    async fn wait_guest_rst(
        &self,
        local_port: u32,
        peer_port: u32,
        deadline: Instant,
    ) -> Result<()> {
        loop {
            let status = tokio::time::timeout_at(
                deadline,
                query_guest_rst(&self.uds, local_port, peer_port),
            )
            .await
            .map_err(|err| anyhow!(err).context("guest RST acknowledgement deadline expired"))??;
            match status {
                GuestRstStatus::Acknowledged => return Ok(()),
                GuestRstStatus::Busy => {
                    tokio::time::sleep_until(std::cmp::min(
                        deadline,
                        Instant::now() + Duration::from_millis(25),
                    ))
                    .await;
                }
            }
        }
    }
}

async fn handshake(
    mut stream: UnixStream,
    port: u32,
) -> std::result::Result<(UnixStream, u32), HandshakeError> {
    stream
        .write_all(format!("connect {port}\n").as_bytes())
        .await
        .map_err(|err| HandshakeError::Committed(anyhow!(err).context("write all")))?;
    let mut reads = BufReader::new(&mut stream);
    let mut response = String::new();
    let bytes = reads
        .read_line(&mut response)
        .await
        .map_err(|err| HandshakeError::Committed(anyhow!(err).context("read line")))?;
    if bytes == 0 {
        return Err(HandshakeError::PeerClosedBeforeResponse);
    }
    let local_port = parse_local_port(&response).map_err(HandshakeError::Committed)?;
    Ok((stream, local_port))
}

fn parse_local_port(response: &str) -> Result<u32> {
    let mut fields = response.split_whitespace();
    let status = fields.next();
    let local_port = fields.next().and_then(|value| value.parse::<u32>().ok());
    if !response.ends_with('\n')
        || status != Some("OK")
        || local_port.is_none()
        || fields.next().is_some()
    {
        return Err(anyhow!(
            "handshake error: malformed response code: {:?}",
            response
        ));
    }
    Ok(local_port.unwrap())
}

#[derive(Debug, PartialEq, Eq)]
enum GuestRstStatus {
    Acknowledged,
    Busy,
}

async fn query_guest_rst(uds: &str, local_port: u32, peer_port: u32) -> Result<GuestRstStatus> {
    let mut stream = UnixStream::connect(uds)
        .await
        .context("connect hvsock guest-RST query")?;
    stream
        .write_all(format!("query-rst {local_port} {peer_port}\n").as_bytes())
        .await
        .context("write hvsock guest-RST query")?;

    let mut response = String::new();
    let bytes = BufReader::new(&mut stream)
        .read_line(&mut response)
        .await
        .context("read hvsock guest-RST query response")?;
    if bytes == 0 {
        return Err(anyhow!("hvsock guest-RST query closed without a response"));
    }
    match response.as_str() {
        "OK\n" => Ok(GuestRstStatus::Acknowledged),
        "BUSY\n" => Ok(GuestRstStatus::Busy),
        _ => Err(anyhow!(
            "malformed hvsock guest-RST query response: {:?}",
            response
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
    use tokio::net::{UnixListener, UnixStream};

    use super::*;

    static NEXT_SOCKET: AtomicU64 = AtomicU64::new(0);

    fn socket_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "kata-hvsock-{name}-{}-{}.sock",
            std::process::id(),
            NEXT_SOCKET.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn handshake_response_requires_exact_local_port() {
        assert_eq!(parse_local_port("OK 42\n").unwrap(), 42);
        for malformed in [
            "OK\n",
            "OK nope\n",
            "OK 42 extra\n",
            "NO 42\n",
            "OK 12",
            "OK 12345",
            "OK",
            "OK ",
        ] {
            assert!(parse_local_port(malformed).is_err(), "{:?}", malformed);
        }
    }

    async fn read_connect_command(stream: &mut UnixStream) {
        let mut command = String::new();
        BufReader::new(stream)
            .read_line(&mut command)
            .await
            .unwrap();
        assert!(command.starts_with("connect "));
    }

    #[tokio::test]
    async fn incomplete_ack_fails() {
        for policy in [
            HandshakeReplayPolicy::PeerClosedBeforeResponse,
            HandshakeReplayPolicy::Never,
        ] {
            for response in ["OK 12", "OK 12345", "OK", "OK "] {
                let path = socket_path("incomplete-ack");
                let listener = UnixListener::bind(&path).unwrap();
                let server = tokio::spawn(async move {
                    let (mut stream, _) = listener.accept().await.unwrap();
                    read_connect_command(&mut stream).await;
                    stream.write_all(response.as_bytes()).await.unwrap();
                    drop(stream);
                    assert!(
                        tokio::time::timeout(Duration::from_millis(100), listener.accept())
                            .await
                            .is_err()
                    );
                });
                let result = HybridVsock::new(path.to_str().unwrap(), 1024)
                    .connect_with_budget(Duration::from_millis(1), Duration::from_secs(1), policy)
                    .await;
                server.await.unwrap();
                std::fs::remove_file(path).unwrap();
                assert!(result.is_err(), "accepted {:?} with {:?}", response, policy);
            }
        }
    }

    #[tokio::test]
    async fn fragmented_ack_succeeds() {
        let (stream, mut peer) = UnixStream::pair().unwrap();
        let server = tokio::spawn(async move {
            read_connect_command(&mut peer).await;
            peer.write_all(b"OK 12").await.unwrap();
            tokio::task::yield_now().await;
            peer.write_all(b"345\n").await.unwrap();
        });
        let (_, local_port) = handshake(stream, 1024).await.unwrap();
        assert_eq!(local_port, 12345);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn guest_rst_query_reuses_exact_tuple_until_deadline() {
        let path = socket_path("guest-rst-query-deadline");
        let listener = UnixListener::bind(&path).unwrap();
        let commands = Arc::new(Mutex::new(Vec::new()));
        let server_commands = commands.clone();
        let server = tokio::spawn(async move {
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut command = String::new();
                BufReader::new(&mut stream)
                    .read_line(&mut command)
                    .await
                    .unwrap();
                server_commands.lock().unwrap().push(command);
                stream.write_all(b"BUSY\n").await.unwrap();
            }
        });

        let sock = HybridVsock::new(path.to_str().unwrap(), 1024);
        assert!(sock
            .wait_guest_rst(42, 1024, Instant::now() + Duration::from_millis(300))
            .await
            .is_err());
        server.abort();
        let commands = commands.lock().unwrap();
        assert!(!commands.is_empty());
        assert!(commands
            .iter()
            .all(|command| command == "query-rst 42 1024\n"));
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn guest_rst_query_reuses_exact_tuple_until_acknowledged() {
        let path = socket_path("guest-rst-query-busy-ok");
        let listener = UnixListener::bind(&path).unwrap();
        let commands = Arc::new(Mutex::new(Vec::new()));
        let server_commands = commands.clone();
        let server = tokio::spawn(async move {
            for response in [b"BUSY\n".as_slice(), b"OK\n".as_slice()] {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut command = String::new();
                BufReader::new(&mut stream)
                    .read_line(&mut command)
                    .await
                    .unwrap();
                server_commands.lock().unwrap().push(command);
                stream.write_all(response).await.unwrap();
            }
        });

        let sock = HybridVsock::new(path.to_str().unwrap(), 1024);
        sock.wait_guest_rst(42, 1024, Instant::now() + Duration::from_secs(1))
            .await
            .unwrap();
        server.await.unwrap();
        assert_eq!(
            commands.lock().unwrap().as_slice(),
            ["query-rst 42 1024\n", "query-rst 42 1024\n"]
        );
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn delayed_committed_handshake_is_not_replayed() {
        let path = socket_path("delayed-handshake");
        let listener = UnixListener::bind(&path).unwrap();
        let accepts = Arc::new(AtomicUsize::new(0));
        let server_accepts = accepts.clone();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            server_accepts.fetch_add(1, Ordering::Relaxed);
            read_connect_command(&mut stream).await;
            tokio::time::sleep(Duration::from_millis(150)).await;
            stream.write_all(b"OK 42\n").await.unwrap();
        });

        let sock = HybridVsock::new(path.to_str().unwrap(), 1024);
        tokio::time::timeout(
            Duration::from_secs(2),
            sock.connect_with_budget(
                Duration::from_millis(10),
                Duration::from_secs(1),
                HandshakeReplayPolicy::Never,
            ),
        )
        .await
        .unwrap()
        .unwrap();
        server.await.unwrap();
        assert_eq!(accepts.load(Ordering::Relaxed), 1);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn all_stalled_handshakes_obey_global_budget() {
        let path = socket_path("all-stalled");
        let listener = UnixListener::bind(&path).unwrap();
        let accepts = Arc::new(AtomicUsize::new(0));
        let server_accepts = accepts.clone();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            server_accepts.fetch_add(1, Ordering::Relaxed);
            read_connect_command(&mut stream).await;
            let mut drained = Vec::new();
            let _ = stream.read_to_end(&mut drained).await;
        });

        let sock = HybridVsock::new(path.to_str().unwrap(), 1024);
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            sock.connect_with_budget(
                Duration::from_millis(10),
                Duration::from_millis(300),
                HandshakeReplayPolicy::Never,
            ),
        )
        .await
        .expect("connect must not outlive its global budget");
        assert!(result.is_err());
        server.await.unwrap();
        assert_eq!(accepts.load(Ordering::Relaxed), 1);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn retries_only_before_the_connect_command_is_sent() {
        let path = socket_path("pre-command-retry");
        let server_path = path.clone();
        let accepts = Arc::new(AtomicUsize::new(0));
        let server_accepts = accepts.clone();
        let server = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let listener = UnixListener::bind(server_path).unwrap();
            let (mut stream, _) = listener.accept().await.unwrap();
            server_accepts.fetch_add(1, Ordering::Relaxed);
            read_connect_command(&mut stream).await;
            stream.write_all(b"OK 9\n").await.unwrap();
        });

        let sock = HybridVsock::new(path.to_str().unwrap(), 1024);
        sock.connect_with_budget(
            Duration::from_millis(10),
            Duration::from_secs(1),
            HandshakeReplayPolicy::Never,
        )
        .await
        .unwrap();
        server.await.unwrap();
        assert_eq!(accepts.load(Ordering::Relaxed), 1);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn malformed_committed_handshake_is_not_replayed() {
        let path = socket_path("malformed-handshake");
        let listener = UnixListener::bind(&path).unwrap();
        let accepts = Arc::new(AtomicUsize::new(0));
        let server_accepts = accepts.clone();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            server_accepts.fetch_add(1, Ordering::Relaxed);
            read_connect_command(&mut stream).await;
            stream.write_all(b"NO\n").await.unwrap();
            assert!(
                tokio::time::timeout(Duration::from_millis(100), listener.accept())
                    .await
                    .is_err()
            );
        });

        let sock = HybridVsock::new(path.to_str().unwrap(), 1024);
        assert!(sock
            .connect_with_budget(
                Duration::from_millis(10),
                Duration::from_secs(1),
                HandshakeReplayPolicy::PeerClosedBeforeResponse,
            )
            .await
            .is_err());
        server.await.unwrap();
        assert_eq!(accepts.load(Ordering::Relaxed), 1);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn eof_after_connect_command_is_not_replayed() {
        let path = socket_path("eof-handshake");
        let listener = UnixListener::bind(&path).unwrap();
        let accepts = Arc::new(AtomicUsize::new(0));
        let server_accepts = accepts.clone();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            server_accepts.fetch_add(1, Ordering::Relaxed);
            read_connect_command(&mut stream).await;
            drop(stream);
            assert!(
                tokio::time::timeout(Duration::from_millis(100), listener.accept())
                    .await
                    .is_err()
            );
        });

        let sock = HybridVsock::new(path.to_str().unwrap(), 1024);
        assert!(sock
            .connect_with_budget(
                Duration::from_millis(10),
                Duration::from_secs(1),
                HandshakeReplayPolicy::Never,
            )
            .await
            .is_err());
        server.await.unwrap();
        assert_eq!(accepts.load(Ordering::Relaxed), 1);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn immediate_ok_uses_one_attempt() {
        let path = socket_path("immediate-ok");
        let listener = UnixListener::bind(&path).unwrap();
        let accepts = Arc::new(AtomicUsize::new(0));
        let server_accepts = accepts.clone();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            server_accepts.fetch_add(1, Ordering::Relaxed);
            read_connect_command(&mut stream).await;
            stream.write_all(b"OK 7\n").await.unwrap();
        });

        let sock = HybridVsock::new(path.to_str().unwrap(), 1024);
        sock.connect_with_budget(
            Duration::from_millis(200),
            Duration::from_secs(1),
            HandshakeReplayPolicy::PeerClosedBeforeResponse,
        )
        .await
        .unwrap();
        server.await.unwrap();
        assert_eq!(accepts.load(Ordering::Relaxed), 1);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn peer_closed_policy_replays_empty_eof_once() {
        let path = socket_path("peer-closed-replay");
        let listener = UnixListener::bind(&path).unwrap();
        let accepts = Arc::new(AtomicUsize::new(0));
        let server_accepts = accepts.clone();
        let server = tokio::spawn(async move {
            let (mut first, _) = listener.accept().await.unwrap();
            server_accepts.fetch_add(1, Ordering::Relaxed);
            read_connect_command(&mut first).await;
            drop(first);

            let (mut second, _) = listener.accept().await.unwrap();
            server_accepts.fetch_add(1, Ordering::Relaxed);
            read_connect_command(&mut second).await;
            second.write_all(b"OK 11\n").await.unwrap();
        });

        let sock = HybridVsock::new(path.to_str().unwrap(), 1024);
        sock.connect_with_budget(
            Duration::from_millis(10),
            Duration::from_secs(1),
            HandshakeReplayPolicy::PeerClosedBeforeResponse,
        )
        .await
        .unwrap();
        server.await.unwrap();
        assert_eq!(accepts.load(Ordering::Relaxed), 2);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn delayed_ok_with_peer_closed_policy_is_not_replayed() {
        let path = socket_path("delayed-peer-closed-policy");
        let listener = UnixListener::bind(&path).unwrap();
        let accepts = Arc::new(AtomicUsize::new(0));
        let server_accepts = accepts.clone();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            server_accepts.fetch_add(1, Ordering::Relaxed);
            read_connect_command(&mut stream).await;
            tokio::time::sleep(Duration::from_millis(150)).await;
            stream.write_all(b"OK 12\n").await.unwrap();
        });

        let sock = HybridVsock::new(path.to_str().unwrap(), 1024);
        sock.connect_with_budget(
            Duration::from_millis(10),
            Duration::from_secs(1),
            HandshakeReplayPolicy::PeerClosedBeforeResponse,
        )
        .await
        .unwrap();
        server.await.unwrap();
        assert_eq!(accepts.load(Ordering::Relaxed), 1);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn committed_errors_are_never_replayable() {
        let error = HandshakeError::Committed(anyhow!("write all failed after partial write"));
        assert!(!error.is_replayable(HandshakeReplayPolicy::PeerClosedBeforeResponse));
        assert!(!error.is_replayable(HandshakeReplayPolicy::Never));
    }
}
