// SPDX-License-Identifier: MIT

use netlink_packet_core::{NetlinkHeader, NetlinkMessage, NLM_F_DUMP, NLM_F_REQUEST};
use netlink_packet_route::rtnl::{LinkMessage, RtnlMessage};

fn main() {
    // Create the netlink message, that contains the rtnetlink
    // message
    let mut packet = NetlinkMessage {
        header: NetlinkHeader {
            sequence_number: 1,
            flags: NLM_F_DUMP | NLM_F_REQUEST,
            ..Default::default()
        },
        payload: RtnlMessage::GetLink(LinkMessage::default()).into(),
    };

    // Set a few fields in the packet's header
    packet.header.flags = NLM_F_DUMP | NLM_F_REQUEST;
    packet.header.sequence_number = 1;

    // Before serializing the packet, it is very important to call
    // finalize() to ensure the header of the message is consistent
    // with its payload. Otherwise, a panic may occur when calling
    // `serialize()`
    packet.finalize();

    // Prepare a buffer to serialize the packet. Note that we never
    // set explicitely `packet.header.length` above. This was done
    // automatically when we called `finalize()`
    let mut buf = vec![0; packet.header.length as usize];
    // Serialize the packet
    packet.serialize(&mut buf[..]);

    // Deserialize the packet
    let deserialized_packet =
        NetlinkMessage::<RtnlMessage>::deserialize(&buf).expect("Failed to deserialize message");

    // Normally, the deserialized packet should be exactly the same
    // than the serialized one.
    assert_eq!(deserialized_packet, packet);

    println!("{:?}", packet);
}
