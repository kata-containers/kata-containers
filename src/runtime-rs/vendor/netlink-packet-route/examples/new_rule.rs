// SPDX-License-Identifier: MIT

use netlink_packet_core::{NetlinkHeader, NetlinkMessage, NetlinkPayload};
use netlink_packet_route::{constants::*, rule, RtnlMessage, RuleHeader, RuleMessage};
use netlink_sys::{protocols::NETLINK_ROUTE, Socket, SocketAddr};

fn main() {
    let mut socket = Socket::new(NETLINK_ROUTE).unwrap();
    let _port_number = socket.bind_auto().unwrap().port_number();
    socket.connect(&SocketAddr::new(0, 0)).unwrap();

    let mut msg = NetlinkMessage {
        header: NetlinkHeader {
            flags: NLM_F_REQUEST | NLM_F_CREATE | NLM_F_EXCL | NLM_F_ACK,
            ..Default::default()
        },
        payload: NetlinkPayload::from(RtnlMessage::NewRule(RuleMessage {
            header: RuleHeader {
                family: AF_INET as u8,
                table: RT_TABLE_DEFAULT,
                action: FR_ACT_TO_TBL,
                ..Default::default()
            },
            nlas: vec![
                rule::Nla::Table(254),
                rule::Nla::SuppressPrefixLen(4294967295),
                rule::Nla::Priority(1000),
                rule::Nla::Protocol(2),
            ],
        })),
    };

    msg.finalize();
    let mut buf = vec![0; 1024 * 8];

    msg.serialize(&mut buf[..msg.buffer_len()]);

    println!(">>> {:?}", msg);

    socket
        .send(&buf, 0)
        .expect("failed to send netlink message");

    let mut receive_buffer = vec![0; 4096];

    while let Ok(_size) = socket.recv(&mut receive_buffer, 0) {
        loop {
            let bytes = &receive_buffer[..];
            let rx_packet = <NetlinkMessage<RtnlMessage>>::deserialize(bytes);
            println!("<<< {:?}", rx_packet);
            if let Ok(rx_packet) = rx_packet {
                if let NetlinkPayload::Error(e) = rx_packet.payload {
                    eprintln!("{:?}", e);
                }
            }
            return;
        }
    }
}
