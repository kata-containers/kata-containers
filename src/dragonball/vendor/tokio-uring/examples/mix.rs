//! Shows how use Tokio types from the `tokio-uring` runtime.
//!
//! Serve a single file over TCP

use std::env;

use tokio_uring::{fs::File, net::TcpListener};

fn main() {
    // The file to serve over TCP is passed as a CLI argument
    let args: Vec<_> = env::args().collect();

    if args.len() <= 1 {
        panic!("no path specified");
    }

    tokio_uring::start(async {
        // Start a TCP listener
        let listener = TcpListener::bind("0.0.0.0:8080".parse().unwrap()).unwrap();

        // Accept new sockets
        loop {
            let (socket, _) = listener.accept().await.unwrap();
            let path = args[1].clone();

            // Spawn a task to send the file back to the socket
            tokio_uring::spawn(async move {
                // Open the file without blocking
                let file = File::open(path).await.unwrap();
                let mut buf = vec![0; 16 * 1_024];

                // Track the current position in the file;
                let mut pos = 0;

                loop {
                    // Read a chunk
                    let (res, b) = file.read_at(buf, pos).await;
                    let n = res.unwrap();

                    if n == 0 {
                        break;
                    }

                    let (res, b) = socket.write(b).await;
                    pos += res.unwrap() as u64;

                    buf = b;
                }
            });
        }
    });
}
