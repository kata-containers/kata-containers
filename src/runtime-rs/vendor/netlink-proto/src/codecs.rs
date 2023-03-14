// SPDX-License-Identifier: MIT

use std::{fmt::Debug, io};

use bytes::{BufMut, BytesMut};
use netlink_packet_core::{
    NetlinkBuffer,
    NetlinkDeserializable,
    NetlinkMessage,
    NetlinkSerializable,
};

/// Protocol to serialize and deserialize messages to and from datagrams
///
/// This is separate from `tokio_util::codec::{Decoder, Encoder}` as the implementations
/// rely on the buffer containing full datagrams; they won't work well with simple
/// bytestreams.
///
/// Officially there should be exactly one implementation of this, but the audit
/// subsystem ignores way too many rules of the protocol, so they need a separate
/// implementation.
///
/// Although one could make a tighter binding between `NetlinkMessageCodec` and
/// the message types (NetlinkDeserializable+NetlinkSerializable) it can handle,
/// this would put quite some overhead on subsystems that followed the spec - so
/// we simply default to the proper implementation (in `Connection`) and the
/// `audit` code needs to overwrite it.
pub trait NetlinkMessageCodec {
    /// Decode message of given type from datagram payload
    ///
    /// There might be more than one message; this needs to be called until it
    /// either returns `Ok(None)` or an error.
    fn decode<T>(src: &mut BytesMut) -> io::Result<Option<NetlinkMessage<T>>>
    where
        T: NetlinkDeserializable + Debug;

    /// Encode message to (datagram) buffer
    fn encode<T>(msg: NetlinkMessage<T>, buf: &mut BytesMut) -> io::Result<()>
    where
        T: NetlinkSerializable + Debug;
}

/// Standard implementation of `NetlinkMessageCodec`
pub struct NetlinkCodec {
    // we don't need an instance of this, just the type
    _private: (),
}

impl NetlinkMessageCodec for NetlinkCodec {
    fn decode<T>(src: &mut BytesMut) -> io::Result<Option<NetlinkMessage<T>>>
    where
        T: NetlinkDeserializable + Debug,
    {
        debug!("NetlinkCodec: decoding next message");

        loop {
            // If there's nothing to read, return Ok(None)
            if src.is_empty() {
                trace!("buffer is empty");
                return Ok(None);
            }

            // This is a bit hacky because we don't want to keep `src`
            // borrowed, since we need to mutate it later.
            let len = match NetlinkBuffer::new_checked(src.as_ref()) {
                Ok(buf) => buf.length() as usize,
                Err(e) => {
                    // We either received a truncated packet, or the
                    // packet if malformed (invalid length field). In
                    // both case, we can't decode the datagram, and we
                    // cannot find the start of the next one (if
                    // any). The only solution is to clear the buffer
                    // and potentially lose some datagrams.
                    error!(
                        "failed to decode datagram, clearing buffer: {:?}: {:#x?}.",
                        e,
                        src.as_ref()
                    );
                    src.clear();
                    return Ok(None);
                }
            };

            let bytes = src.split_to(len);

            let parsed = NetlinkMessage::<T>::deserialize(&bytes);
            match parsed {
                Ok(packet) => {
                    trace!("<<< {:?}", packet);
                    return Ok(Some(packet));
                }
                Err(e) => {
                    error!("failed to decode packet {:#x?}: {}", &bytes, e);
                    // continue looping, there may be more datagrams in the buffer
                }
            }
        }
    }

    fn encode<T>(msg: NetlinkMessage<T>, buf: &mut BytesMut) -> io::Result<()>
    where
        T: Debug + NetlinkSerializable,
    {
        let msg_len = msg.buffer_len();
        if buf.remaining_mut() < msg_len {
            // BytesMut can expand till usize::MAX... unlikely to hit this one.
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "message is {} bytes, but only {} bytes left in the buffer",
                    msg_len,
                    buf.remaining_mut()
                ),
            ));
        }

        // As NetlinkMessage::serialize needs an initialized buffer anyway
        // no need for any `unsafe` magic.
        let old_len = buf.len();
        let new_len = old_len + msg_len;
        buf.resize(new_len, 0);
        msg.serialize(&mut buf[old_len..][..msg_len]);
        trace!(">>> {:?}", msg);
        Ok(())
    }
}
