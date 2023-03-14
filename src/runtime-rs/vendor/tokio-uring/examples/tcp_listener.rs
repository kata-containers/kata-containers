use std::{env, net::SocketAddr};

use tokio_uring::net::TcpListener;

fn main() {
    let args: Vec<_> = env::args().collect();

    if args.len() <= 1 {
        panic!("no addr specified");
    }

    let socket_addr: SocketAddr = args[1].parse().unwrap();

    tokio_uring::start(async {
        let listener = TcpListener::bind(socket_addr).unwrap();

        loop {
            let (stream, socket_addr) = listener.accept().await.unwrap();
            tokio_uring::spawn(async move {
                let buf = vec![1u8; 128];

                let (result, buf) = stream.write(buf).await;
                println!("written to {}: {}", socket_addr, result.unwrap());

                let (result, buf) = stream.read(buf).await;
                let read = result.unwrap();
                println!("read from {}: {:?}", socket_addr, &buf[..read]);
            });
        }
    });
}
