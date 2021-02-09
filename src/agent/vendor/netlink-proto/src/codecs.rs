use std::{fmt::Debug, io, marker::PhantomData, mem::MaybeUninit};

use bytes::{BufMut, BytesMut};
use netlink_packet_core::{
    NetlinkBuffer,
    NetlinkDeserializable,
    NetlinkMessage,
    NetlinkSerializable,
};
use tokio_util::codec::{Decoder, Encoder};

pub struct NetlinkCodec<T> {
    phantom: PhantomData<T>,
}

impl<T> Default for NetlinkCodec<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> NetlinkCodec<T> {
    pub fn new() -> Self {
        NetlinkCodec {
            phantom: PhantomData,
        }
    }
}

// FIXME: it seems that for audit, we're receiving malformed packets.
// See https://github.com/mozilla/libaudit-go/issues/24
impl<T> Decoder for NetlinkCodec<NetlinkMessage<T>>
where
    T: NetlinkDeserializable<T> + Debug + Eq + PartialEq + Clone,
{
    type Item = NetlinkMessage<T>;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        debug!("NetlinkCodec: decoding next message");

        loop {
            // If there's nothing to read, return Ok(None)
            if src.as_ref().is_empty() {
                trace!("buffer is empty");
                src.clear();
                return Ok(None);
            }

            // This is a bit hacky because we don't want to keep `src`
            // borrowed, since we need to mutate it later.
            let len_res = match NetlinkBuffer::new_checked(src.as_ref()) {
                #[cfg(not(feature = "workaround-audit-bug"))]
                Ok(buf) => Ok(buf.length() as usize),
                #[cfg(feature = "workaround-audit-bug")]
                Ok(buf) => {
                    if (src.as_ref().len() as isize - buf.length() as isize) <= 16 {
                        // The audit messages are sometimes truncated,
                        // because the length specified in the header,
                        // does not take the header itself into
                        // account. To workaround this, we tweak the
                        // length. We've noticed two occurences of
                        // truncated packets:
                        //
                        // - the length of the header is not included (see also:
                        //   https://github.com/mozilla/libaudit-go/issues/24)
                        // - some rule message have some padding for alignment (see
                        //   https://github.com/linux-audit/audit-userspace/issues/78) which is not
                        //   taken into account in the buffer length.
                        warn!("found what looks like a truncated audit packet");
                        Ok(src.as_ref().len())
                    } else {
                        Ok(buf.length() as usize)
                    }
                }
                Err(e) => {
                    // We either received a truncated packet, or the
                    // packet if malformed (invalid length field). In
                    // both case, we can't decode the datagram, and we
                    // cannot find the start of the next one (if
                    // any). The only solution is to clear the buffer
                    // and potentially lose some datagrams.
                    error!("failed to decode datagram: {:?}: {:#x?}.", e, src.as_ref());
                    Err(())
                }
            };

            if len_res.is_err() {
                error!("clearing the whole socket buffer. Datagrams may have been lost");
                src.clear();
                return Ok(None);
            }

            let len = len_res.unwrap();

            #[cfg(feature = "workaround-audit-bug")]
            let bytes = {
                let mut bytes = src.split_to(len);
                {
                    let mut buf = NetlinkBuffer::new(bytes.as_mut());
                    // If the buffer contains more bytes than what the header says the length is, it
                    // means we ran into a malformed packet (see comment above), and we just set the
                    // "right" length ourself, so that parsing does not fail.
                    //
                    // How do we know that's the right length? Due to an implementation detail and to
                    // the fact that netlink is a datagram protocol.
                    //
                    // - our implementation of Stream always calls the codec with at most 1 message in
                    //   the buffer, so we know the extra bytes do not belong to another message.
                    // - because netlink is a datagram protocol, we receive entire messages, so we know
                    //   that if those extra bytes do not belong to another message, they belong to
                    //   this one.
                    if len != buf.length() as usize {
                        warn!(
                            "setting packet length to {} instead of {}",
                            len,
                            buf.length()
                        );
                        buf.set_length(len as u32);
                    }
                }
                bytes
            };
            #[cfg(not(feature = "workaround-audit-bug"))]
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
}

impl<T> Encoder for NetlinkCodec<NetlinkMessage<T>>
where
    T: Debug + Eq + PartialEq + Clone + NetlinkSerializable<T>,
{
    type Item = NetlinkMessage<T>;
    type Error = io::Error;

    fn encode(&mut self, msg: Self::Item, buf: &mut BytesMut) -> Result<(), Self::Error> {
        let msg_len = msg.buffer_len();
        // FIXME: we should have a max length for the buffer
        while buf.remaining_mut() < msg_len {
            let new_len = buf.len() + 2048;
            buf.resize(new_len, 0);
        }
        let size = msg.buffer_len();
        if buf.remaining_mut() < size {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "message is {} bytes, but only {} bytes left in the buffer",
                    size,
                    buf.remaining_mut()
                ),
            ));
        }
        unsafe {
            // Safety: we initialize the buffer we're passing to
            // NetlinkMessage::serialize(). In theory, `serialize()`
            // should be safe because it's not supposed to _read_ from
            // the buffer, which is potentially
            // un-initialized. However, since we delegate the actual
            // implementation to users, we cannot guarantee
            // anything. Therefore we have to initialize the buffer
            // here.
            let bytes: &mut [std::mem::MaybeUninit<u8>] = &mut buf.bytes_mut()[..size];
            for b in &mut bytes[..] {
                *b.as_mut_ptr() = 0;
            }
            let initialized_bytes = &mut *(bytes as *mut [MaybeUninit<u8>] as *mut [u8]);

            msg.serialize(initialized_bytes);
            trace!(">>> {:?}", msg);
            buf.advance_mut(size);
        }
        Ok(())
    }
}
