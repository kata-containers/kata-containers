// SPDX-License-Identifier: MIT

#[macro_export]
macro_rules! try_rtnl {
    ($msg: expr, $message_type:path) => {{
        use netlink_packet_route::{NetlinkMessage, NetlinkPayload, RtnlMessage};
        use $crate::Error;

        let (header, payload) = $msg.into_parts();
        match payload {
            NetlinkPayload::InnerMessage($message_type(msg)) => msg,
            NetlinkPayload::Error(err) => return Err(Error::NetlinkError(err)),
            _ => {
                return Err(Error::UnexpectedMessage(NetlinkMessage::new(
                    header, payload,
                )))
            }
        }
    }};
}

#[macro_export]
macro_rules! try_nl {
    ($msg: expr) => {{
        use netlink_packet_route::NetlinkPayload;
        use $crate::Error;
        if let NetlinkPayload::Error(err) = $msg.payload {
            return Err(Error::NetlinkError(err));
        }
    }};
}
