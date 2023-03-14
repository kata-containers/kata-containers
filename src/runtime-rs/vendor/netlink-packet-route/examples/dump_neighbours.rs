// SPDX-License-Identifier: MIT

use std::{convert::TryFrom, net::IpAddr, string::ToString};

use netlink_packet_route::{
    constants::*,
    nlas::neighbour::Nla,
    NeighbourMessage,
    NetlinkHeader,
    NetlinkMessage,
    NetlinkPayload,
    RtnlMessage,
};
use netlink_sys::{protocols::NETLINK_ROUTE, Socket, SocketAddr};

fn main() {
    let mut socket = Socket::new(NETLINK_ROUTE).unwrap();
    let _port_number = socket.bind_auto().unwrap().port_number();
    socket.connect(&SocketAddr::new(0, 0)).unwrap();

    let mut req = NetlinkMessage {
        header: NetlinkHeader {
            flags: NLM_F_DUMP | NLM_F_REQUEST,
            ..Default::default()
        },
        payload: NetlinkPayload::from(RtnlMessage::GetNeighbour(NeighbourMessage::default())),
    };
    // IMPORTANT: call `finalize()` to automatically set the
    // `message_type` and `length` fields to the appropriate values in
    // the netlink header.
    req.finalize();

    let mut buf = vec![0; req.header.length as usize];
    req.serialize(&mut buf[..]);

    println!(">>> {:?}", req);
    socket.send(&buf[..], 0).unwrap();

    let mut receive_buffer = vec![0; 4096];
    let mut offset = 0;

    'outer: loop {
        let size = socket.recv(&mut &mut receive_buffer[..], 0).unwrap();

        loop {
            let bytes = &receive_buffer[offset..];
            // Parse the message
            let msg: NetlinkMessage<RtnlMessage> = NetlinkMessage::deserialize(bytes).unwrap();

            match msg.payload {
                NetlinkPayload::Done => break 'outer,
                NetlinkPayload::InnerMessage(RtnlMessage::NewNeighbour(entry)) => {
                    let address_family = entry.header.family as u16;
                    if address_family == AF_INET || address_family == AF_INET6 {
                        print_entry(entry);
                    }
                }
                NetlinkPayload::Error(err) => {
                    eprintln!("Received a netlink error message: {:?}", err);
                    return;
                }
                _ => {}
            }

            offset += msg.header.length as usize;
            if offset == size || msg.header.length == 0 {
                offset = 0;
                break;
            }
        }
    }
}

fn format_ip(buf: &[u8]) -> String {
    if let Ok(bytes) = <&[u8; 4]>::try_from(buf) {
        IpAddr::from(*bytes).to_string()
    } else if let Ok(bytes) = <&[u8; 16]>::try_from(buf) {
        IpAddr::from(*bytes).to_string()
    } else {
        panic!("Invalid IP Address");
    }
}

fn format_mac(buf: &[u8]) -> String {
    assert_eq!(buf.len(), 6);
    format!(
        "{:<02x}:{:<02x}:{:<02x}:{:<02x}:{:<02x}:{:<02x}",
        buf[0], buf[1], buf[2], buf[3], buf[4], buf[5]
    )
}

fn state_str(value: u16) -> &'static str {
    match value {
        NUD_INCOMPLETE => "INCOMPLETE",
        NUD_REACHABLE => "REACHABLE",
        NUD_STALE => "STALE",
        NUD_DELAY => "DELAY",
        NUD_PROBE => "PROBE",
        NUD_FAILED => "FAILED",
        NUD_NOARP => "NOARP",
        NUD_PERMANENT => "PERMANENT",
        NUD_NONE => "NONE",
        _ => "UNKNOWN",
    }
}

fn print_entry(entry: NeighbourMessage) {
    let state = state_str(entry.header.state);
    let dest = entry
        .nlas
        .iter()
        .find_map(|nla| {
            if let Nla::Destination(addr) = nla {
                Some(format_ip(&addr[..]))
            } else {
                None
            }
        })
        .unwrap();
    let lladdr = entry
        .nlas
        .iter()
        .find_map(|nla| {
            if let Nla::LinkLocalAddress(addr) = nla {
                Some(format_mac(&addr[..]))
            } else {
                None
            }
        })
        .unwrap();

    println!("{:<30} {:<20} ({})", dest, lladdr, state);
}
