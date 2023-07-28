use anyhow::Result;
use lazy_static::lazy_static;
use rustjail::process::ProcessIo;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_vsock::SockAddr::Vsock;
use tokio_vsock::{VsockListener, VsockStream};

lazy_static! {
    static ref HVSOCK_STREAMS: Arc<Mutex<HashMap<u32, VsockStream>>> =
        Arc::new(Mutex::new(HashMap::new()));
}

// Convenience function to obtain the scope logger.
fn sl() -> slog::Logger {
    slog_scope::logger().new(o!("subsystem" => "passfd_io"))
}

pub(crate) async fn start_listen(port: u32) -> Result<()> {
    info!(sl(), "start listening on port {}", port);
    let mut listener = VsockListener::bind(libc::VMADDR_CID_ANY, port)?;
    tokio::spawn(async move {
        loop {
            if let Ok((stream, peer_addr)) = listener.accept().await {
                if let Vsock(addr) = peer_addr {
                    let port = addr.port();
                    info!(sl(), "accept connection from peer port {}", port);
                    HVSOCK_STREAMS.lock().await.insert(port, stream);
                }
            }
        }
    });
    Ok(())
}

async fn take_stream(port: u32) -> Option<VsockStream> {
    let mut mapping = HVSOCK_STREAMS.lock().await;
    mapping.remove(&port)
}

macro_rules! take_io_stream {
    ($port: ident) => {
        if $port == 0 {
            None
        } else {
            take_stream($port).await
        }
    };
}

pub(crate) async fn take_io_streams(
    stdin_port: u32,
    stdout_port: u32,
    stderr_port: u32,
) -> ProcessIo {
    let stdin = take_io_stream!(stdin_port);
    let stdout = take_io_stream!(stdout_port);
    let stderr = take_io_stream!(stderr_port);
    debug!(
        sl(),
        "take passfd io streams {} {} {}", stdin_port, stdout_port, stderr_port
    );
    ProcessIo::new(stdin, stdout, stderr)
}
