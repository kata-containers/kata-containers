// SPDX-License-Identifier: MIT

#[cfg(feature = "rich_nlas")]
mod test_rich_nlas {
    use crate::{
        rtnl::route::{
            nlas::{NextHop, NextHopFlags, Nla},
            RouteFlags,
            RouteMessage,
            RouteMessageBuffer,
        },
        utils::{Emitable, Parseable},
    };
    use std::net::Ipv6Addr;

    #[rustfmt::skip]
    static ROUTE_MSG: [u8; 100] = [
        0x0a, // address family
        0x40, // length of destination
        0x00, // length of source
        0x00, // TOS
        0xfe, // routing table id
        0x03, // routing protocol (boot)
        0x00, // route origin (global)
        0x01, // gateway or direct route
        0x00, 0x00, 0x00, 0x00,

            // Route destination address NLA
            0x14, 0x00, // Length (20)
            0x01, 0x00, // Type
            // Value
            0x10, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,

            // RTA_MULTIPATH attribute
            0x44, 0x00, // Length (68)
            0x09, 0x00, // Type

                // next-hop 1
                0x1c, 0x00, // length (28)
                0x00, // flags
                0x00, // hops
                0x00, 0x00, 0x00, 0x00, // interface ID
                    // nested RTA_GATEWAY
                    0x14, 0x00, // Length (14)
                    0x05, 0x00, // Type
                    // Value
                    0xfc, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,

                // next-hop 2
                0x1c, 0x00, // length (28)
                0x00, // flags
                0x00, // hops
                0x00, 0x00, 0x00, 0x00, // interface ID
                    // nested RTA_GATEWAY
                    0x14, 0x00, // Length (14)
                    0x05, 0x00, // Type
                    // Value
                    0xfc, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,

                // next-hop 3
                0x08, 0x00, // length (8)
                0x00,  // flags
                0x00, // hops
                0x02, 0x00, 0x00, 0x00, // interface ID
    ];

    fn route_message() -> RouteMessage {
        let mut msg = RouteMessage::default();
        msg.header.address_family = 0x0a;
        msg.header.destination_prefix_length = 0x40;
        msg.header.source_prefix_length = 0;
        msg.header.tos = 0;
        msg.header.table = 0xfe;
        msg.header.protocol = 0x03;
        msg.header.scope = 0x00;
        msg.header.kind = 0x01;
        msg.header.flags = RouteFlags::empty();
        msg.nlas = vec![
            Nla::Destination("1001::".parse::<Ipv6Addr>().unwrap().octets().to_vec()),
            Nla::MultiPath(vec![
                NextHop {
                    flags: NextHopFlags::empty(),
                    hops: 0,
                    interface_id: 0,
                    nlas: vec![Nla::Gateway(
                        "fc00::1".parse::<Ipv6Addr>().unwrap().octets().to_vec(),
                    )],
                },
                NextHop {
                    flags: NextHopFlags::empty(),
                    hops: 0,
                    interface_id: 0,
                    nlas: vec![Nla::Gateway(
                        "fc01::1".parse::<Ipv6Addr>().unwrap().octets().to_vec(),
                    )],
                },
                NextHop {
                    flags: NextHopFlags::empty(),
                    hops: 0,
                    interface_id: 2,
                    nlas: vec![],
                },
            ]),
        ];
        msg
    }

    #[test]
    fn parse_message_with_multipath_nla() {
        let expected = route_message();
        let actual =
            RouteMessage::parse(&RouteMessageBuffer::new_checked(&&ROUTE_MSG[..]).unwrap())
                .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn emit_message_with_multipath_nla() {
        let msg = route_message();
        let mut buf = vec![0; 100];
        assert_eq!(msg.buffer_len(), 100);
        msg.emit(&mut buf[..]);
        assert_eq!(buf, ROUTE_MSG);
    }
}
