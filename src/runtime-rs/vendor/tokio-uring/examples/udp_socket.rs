use std::{env, net::SocketAddr};
use tokio_uring::net::UdpSocket;

fn main() {
    let args: Vec<_> = env::args().collect();

    if args.len() <= 1 {
        panic!("no addr specified");
    }

    let socket_addr: SocketAddr = args[1].parse().unwrap();

    tokio_uring::start(async {
        let socket = UdpSocket::bind(socket_addr).await.unwrap();

        let buf = vec![0u8; 128];

        let (result, mut buf) = socket.recv_from(buf).await;
        let (read, socket_addr) = result.unwrap();
        buf.resize(read, 0);
        println!("received from {}: {:?}", socket_addr, &buf[..]);

        let (result, _buf) = socket.send_to(buf, socket_addr).await;
        println!("sent to {}: {}", socket_addr, result.unwrap());
    });
}
