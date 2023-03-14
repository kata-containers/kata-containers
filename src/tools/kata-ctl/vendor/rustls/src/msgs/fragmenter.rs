use crate::msgs::base::Payload;
use crate::msgs::enums::{ContentType, ProtocolVersion};
use crate::msgs::message::{BorrowedPlainMessage, PlainMessage};
use crate::Error;
use std::collections::VecDeque;

pub const MAX_FRAGMENT_LEN: usize = 16384;
pub const PACKET_OVERHEAD: usize = 1 + 2 + 2;
pub const MAX_FRAGMENT_SIZE: usize = MAX_FRAGMENT_LEN + PACKET_OVERHEAD;

pub struct MessageFragmenter {
    max_frag: usize,
}

impl MessageFragmenter {
    /// Make a new fragmenter.
    ///
    /// `max_fragment_size` is the maximum fragment size that will be produced --
    /// this includes overhead. A `max_fragment_size` of 10 will produce TLS fragments
    /// up to 10 bytes.
    pub fn new(max_fragment_size: Option<usize>) -> Result<Self, Error> {
        let mut new = Self { max_frag: 0 };
        new.set_max_fragment_size(max_fragment_size)?;
        Ok(new)
    }

    /// Take the Message `msg` and re-fragment it into new
    /// messages whose fragment is no more than max_frag.
    /// The new messages are appended to the `out` deque.
    /// Payloads are copied.
    pub fn fragment(&self, msg: PlainMessage, out: &mut VecDeque<PlainMessage>) {
        // Non-fragment path
        if msg.payload.0.len() <= self.max_frag {
            out.push_back(msg);
            return;
        }

        for chunk in msg.payload.0.chunks(self.max_frag) {
            out.push_back(PlainMessage {
                typ: msg.typ,
                version: msg.version,
                payload: Payload(chunk.to_vec()),
            });
        }
    }

    /// Enqueue borrowed fragments of (version, typ, payload) which
    /// are no longer than max_frag onto the `out` deque.
    pub fn fragment_borrow<'a>(
        &self,
        typ: ContentType,
        version: ProtocolVersion,
        payload: &'a [u8],
        out: &mut VecDeque<BorrowedPlainMessage<'a>>,
    ) {
        for chunk in payload.chunks(self.max_frag) {
            let cm = BorrowedPlainMessage {
                typ,
                version,
                payload: chunk,
            };
            out.push_back(cm);
        }
    }

    pub fn set_max_fragment_size(&mut self, new: Option<usize>) -> Result<(), Error> {
        self.max_frag = match new {
            Some(sz @ 32..=MAX_FRAGMENT_SIZE) => sz - PACKET_OVERHEAD,
            None => MAX_FRAGMENT_LEN,
            _ => return Err(Error::BadMaxFragmentSize),
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{MessageFragmenter, PACKET_OVERHEAD};
    use crate::msgs::base::Payload;
    use crate::msgs::enums::{ContentType, ProtocolVersion};
    use crate::msgs::message::PlainMessage;
    use std::collections::VecDeque;

    fn msg_eq(
        mm: Option<PlainMessage>,
        total_len: usize,
        typ: &ContentType,
        version: &ProtocolVersion,
        bytes: &[u8],
    ) {
        let m = mm.unwrap();
        let buf = m
            .clone()
            .into_unencrypted_opaque()
            .encode();

        assert_eq!(&m.typ, typ);
        assert_eq!(&m.version, version);
        assert_eq!(m.payload.0, bytes.to_vec());

        assert_eq!(total_len, buf.len());
    }

    #[test]
    fn smoke() {
        let typ = ContentType::Handshake;
        let version = ProtocolVersion::TLSv1_2;
        let data: Vec<u8> = (1..70u8).collect();
        let m = PlainMessage {
            typ,
            version,
            payload: Payload::new(data),
        };

        let frag = MessageFragmenter::new(Some(32)).unwrap();
        let mut q = VecDeque::new();
        frag.fragment(m, &mut q);
        msg_eq(
            q.pop_front(),
            32,
            &typ,
            &version,
            &[
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
                24, 25, 26, 27,
            ],
        );
        msg_eq(
            q.pop_front(),
            32,
            &typ,
            &version,
            &[
                28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48,
                49, 50, 51, 52, 53, 54,
            ],
        );
        msg_eq(
            q.pop_front(),
            20,
            &typ,
            &version,
            &[55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69],
        );
        assert_eq!(q.len(), 0);
    }

    #[test]
    fn non_fragment() {
        let m = PlainMessage {
            typ: ContentType::Handshake,
            version: ProtocolVersion::TLSv1_2,
            payload: Payload::new(b"\x01\x02\x03\x04\x05\x06\x07\x08".to_vec()),
        };

        let frag = MessageFragmenter::new(Some(32)).unwrap();
        let mut q = VecDeque::new();
        frag.fragment(m, &mut q);
        msg_eq(
            q.pop_front(),
            PACKET_OVERHEAD + 8,
            &ContentType::Handshake,
            &ProtocolVersion::TLSv1_2,
            b"\x01\x02\x03\x04\x05\x06\x07\x08",
        );
        assert_eq!(q.len(), 0);
    }
}
