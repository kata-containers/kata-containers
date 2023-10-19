#![feature(impl_trait_in_assoc_type)]
#![feature(type_alias_impl_trait)]

use containerd_snapshots::server;
use log::{error, info, warn};
use snapshotter::TarDevSnapshotter;
use std::{env, io, path::Path, process, sync::Arc};
use tokio::net::UnixListener;
use tonic::transport::Server;

mod snapshotter;

#[tokio::main]
pub async fn main() {
    env_logger::init();

    let argv: Vec<String> = env::args().collect();
    if argv.len() != 3 && argv.len() != 4 {
        error!(
            "Usage: {} <data-root-path> <listen-socket-name> [containerd-socket]",
            argv[0]
        );
        process::exit(1);
    }

    let containerd_socket = if argv.len() >= 4 {
        &argv[3]
    } else {
        "/var/run/containerd/containerd.sock"
    };

    // TODO: Check that the directory is accessible.

    let incoming = {
        let uds = match bind(&argv[2]) {
            Ok(l) => l,
            Err(e) => {
                error!("UnixListener::bind failed: {e:?}");
                process::exit(1);
            }
        };

        async_stream::stream! {
            loop {
                let item = uds.accept().await.map(|p| p.0);
                yield item;
            }
        }
    };

    info!("Snapshotter started");
    if let Err(e) = Server::builder()
        .add_service(server(Arc::new(TarDevSnapshotter::new(
            Path::new(&argv[1]),
            containerd_socket.to_string(),
        ))))
        .serve_with_incoming(incoming)
        .await
    {
        error!("serve_with_incoming failed: {:?}", e);
        process::exit(1);
    }
}

fn bind(addr: &str) -> io::Result<UnixListener> {
    // Try to bind. Return on success or failure other than "address in use".
    match UnixListener::bind(addr) {
        Ok(l) => return Ok(l),
        Err(e) => {
            if e.kind() != io::ErrorKind::AddrInUse {
                return Err(e);
            }
        }
    }

    // Try to remove the existing socket and bind again.
    warn!(
        "Listen address ({}) already exists, trying to remove it",
        addr
    );
    let _ = std::fs::remove_file(addr);
    UnixListener::bind(addr)
}
