// SPDX-License-Identifier: MIT

#![cfg(test)]

use crate::{
    nlas::link::{Info, InfoKind, Nla},
    traits::ParseableParametrized,
    LinkHeader,
    LinkMessage,
    NetlinkBuffer,
    RtnlMessage,
    RtnlMessageBuffer,
    RTM_NEWLINK,
};

// This test was added because one of the NLA's payload is a string that is not null
// terminated. I'm not sure if we missed something in the IFLA_LINK_INFO spec, or if
// linux/iproute2 is being a bit inconsistent here.
//
// This message was created using `ip link add qemu-br1 type bridge`.
#[rustfmt::skip]
#[test]
fn test_non_null_terminated_string() {
    let data = vec![
        0x40, 0x00, 0x00, 0x00, // length = 64
        0x10, 0x00, // message type = 16 = (create network interface)
        0x05, 0x06, // flags
        0x81, 0x74, 0x57, 0x5c, // seq id
        0x00, 0x00, 0x00, 0x00, // pid
        0x00, // interface family
        0x00, // padding
        0x00, 0x00, // device type (NET/ROM pseudo)
        0x00, 0x00, 0x00, 0x00, // interface index
        0x00, 0x00, 0x00, 0x00, // device flags
        0x00, 0x00, 0x00, 0x00, // device change flags
        // NLA: device name
        0x0d, 0x00, // length = 13
        0x03, 0x00, // type = 3
        // value=qemu-br1 NOTE THAT THIS IS NULL-TERMINATED
        0x71, 0x65, 0x6d, 0x75, 0x2d, 0x62, 0x72, 0x31, 0x00,
        0x00, 0x00, 0x00, // padding
        // NLA: Link info
        0x10, 0x00, // length = 16
        0x12, 0x00, // type = link info
        // nested NLA:
        0x0a, 0x00, // length = 10
        0x01, 0x00, // type = 1 = IFLA_INFO_KIND
        // "bridge" NOTE THAT THIS IS NOT NULL-TERMINATED!
        0x62, 0x72, 0x69, 0x64, 0x67, 0x65,
        0x00, 0x00, // padding
        ];
    let expected = RtnlMessage::NewLink(LinkMessage {
        header: LinkHeader::default(),
        nlas: vec![
            Nla::IfName(String::from("qemu-br1")),
            Nla::Info(vec![Info::Kind(InfoKind::Bridge)]),
        ],
    });
    let nl_buffer = NetlinkBuffer::new(&data).payload();
    let rtnl_buffer = RtnlMessageBuffer::new(&nl_buffer);
    let actual = RtnlMessage::parse_with_param(&rtnl_buffer, RTM_NEWLINK).unwrap();
    assert_eq!(expected, actual);
}

#[rustfmt::skip]
#[test]
fn test_attach_to_bridge() {
    use crate::*;
    let data = vec![
        0x28, 0x00, 0x00, 0x00, // length
        0x10, 0x00, // type
        0x05, 0x00, // flags
        0x9c, 0x9d, 0x57, 0x5c, // seq id
        0x00, 0x00, 0x00, 0x00, // pid
        0x00, // interface family
        0x00, // padding
        0x00, 0x00, // device type
        0x06, 0x00, 0x00, 0x00, // interface index
        0x00, 0x00, 0x00, 0x00, // device flags
        0x00, 0x00, 0x00, 0x00, // device change flags
        // NLA (set master)
        0x08, 0x00, // length
        0x0a, 0x00, // type
        0x05, 0x00, 0x00, 0x00 // index of the master interface
    ];
    let nl_buffer = NetlinkBuffer::new(&data).payload();
    let rtnl_buffer = RtnlMessageBuffer::new(&nl_buffer);
    let actual = RtnlMessage::parse_with_param(&rtnl_buffer, RTM_NEWLINK).unwrap();
    let expected = RtnlMessage::NewLink(LinkMessage {
        header: LinkHeader {
            interface_family: 0,
            index: 6,
            link_layer_type: 0,
            flags: 0,
            change_mask: 0,
        },
        nlas: vec![Nla::Master(5)],
    });
    assert_eq!(expected, actual);
}
