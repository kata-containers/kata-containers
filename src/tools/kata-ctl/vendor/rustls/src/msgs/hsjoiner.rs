use std::collections::VecDeque;

use crate::msgs::base::Payload;
use crate::msgs::codec;
use crate::msgs::enums::{ContentType, ProtocolVersion};
use crate::msgs::handshake::HandshakeMessagePayload;
use crate::msgs::message::{Message, MessagePayload, PlainMessage};

const HEADER_SIZE: usize = 1 + 3;

/// TLS allows for handshake messages of up to 16MB.  We
/// restrict that to 64KB to limit potential for denial-of-
/// service.
const MAX_HANDSHAKE_SIZE: u32 = 0xffff;

/// This works to reconstruct TLS handshake messages
/// from individual TLS messages.  It's guaranteed that
/// TLS messages output from this layer contain precisely
/// one handshake payload.
pub struct HandshakeJoiner {
    /// Completed handshake frames for output.
    pub frames: VecDeque<Message>,

    /// The message payload we're currently accumulating.
    buf: Vec<u8>,
}

impl Default for HandshakeJoiner {
    fn default() -> Self {
        Self::new()
    }
}

enum BufferState {
    /// Buffer contains a header that introduces a message that is too long.
    MessageTooLarge,

    /// Buffer contains a full header and body.
    OneMessage,

    /// We need more data to see a header and complete body.
    NeedsMoreData,
}

impl HandshakeJoiner {
    /// Make a new HandshakeJoiner.
    pub fn new() -> Self {
        Self {
            frames: VecDeque::new(),
            buf: Vec::new(),
        }
    }

    /// Do we want to process this message?
    pub fn want_message(&self, msg: &PlainMessage) -> bool {
        msg.typ == ContentType::Handshake
    }

    /// Do we have any buffered data?
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Take the message, and join/split it as needed.
    /// Return the number of new messages added to the
    /// output deque as a result of this message.
    ///
    /// Returns None if msg or a preceding message was corrupt.
    /// You cannot recover from this situation.  Otherwise returns
    /// a count of how many messages we queued.
    pub fn take_message(&mut self, msg: PlainMessage) -> Option<usize> {
        // The vast majority of the time `self.buf` will be empty since most
        // handshake messages arrive in a single fragment. Avoid allocating and
        // copying in that common case.
        if self.buf.is_empty() {
            self.buf = msg.payload.0;
        } else {
            self.buf
                .extend_from_slice(&msg.payload.0[..]);
        }

        let mut count = 0;
        loop {
            match self.buf_contains_message() {
                BufferState::MessageTooLarge => return None,
                BufferState::NeedsMoreData => break,
                BufferState::OneMessage => {
                    if !self.deframe_one(msg.version) {
                        return None;
                    }

                    count += 1;
                }
            }
        }

        Some(count)
    }

    /// Does our `buf` contain a full handshake payload?  It does if it is big
    /// enough to contain a header, and that header has a length which falls
    /// within `buf`.
    fn buf_contains_message(&self) -> BufferState {
        if self.buf.len() < HEADER_SIZE {
            return BufferState::NeedsMoreData;
        }

        let (header, rest) = self.buf.split_at(HEADER_SIZE);
        match codec::u24::decode(&header[1..]) {
            Some(len) if len.0 > MAX_HANDSHAKE_SIZE => BufferState::MessageTooLarge,
            Some(len) if rest.get(..len.into()).is_some() => BufferState::OneMessage,
            _ => BufferState::NeedsMoreData,
        }
    }

    /// Take a TLS handshake payload off the front of `buf`, and put it onto
    /// the back of our `frames` deque inside a normal `Message`.
    ///
    /// Returns false if the stream is desynchronised beyond repair.
    fn deframe_one(&mut self, version: ProtocolVersion) -> bool {
        let used = {
            let mut rd = codec::Reader::init(&self.buf);
            let parsed = match HandshakeMessagePayload::read_version(&mut rd, version) {
                Some(p) => p,
                None => return false,
            };

            let m = Message {
                version,
                payload: MessagePayload::Handshake {
                    parsed,
                    encoded: Payload::new(&self.buf[..rd.used()]),
                },
            };

            self.frames.push_back(m);
            rd.used()
        };
        self.buf = self.buf.split_off(used);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::HandshakeJoiner;
    use crate::msgs::base::Payload;
    use crate::msgs::codec::Codec;
    use crate::msgs::enums::{ContentType, HandshakeType, ProtocolVersion};
    use crate::msgs::handshake::{HandshakeMessagePayload, HandshakePayload};
    use crate::msgs::message::{Message, MessagePayload, PlainMessage};

    #[test]
    fn want() {
        let hj = HandshakeJoiner::new();
        assert!(hj.is_empty());

        let wanted = PlainMessage {
            typ: ContentType::Handshake,
            version: ProtocolVersion::TLSv1_2,
            payload: Payload::new(b"hello world".to_vec()),
        };

        let unwanted = PlainMessage {
            typ: ContentType::Alert,
            version: ProtocolVersion::TLSv1_2,
            payload: Payload::new(b"ponytown".to_vec()),
        };

        assert!(hj.want_message(&wanted));
        assert!(!hj.want_message(&unwanted));
    }

    fn pop_eq(expect: &PlainMessage, hj: &mut HandshakeJoiner) {
        let got = hj.frames.pop_front().unwrap();
        assert_eq!(got.payload.content_type(), expect.typ);
        assert_eq!(got.version, expect.version);

        let (mut left, mut right) = (Vec::new(), Vec::new());
        got.payload.encode(&mut left);
        expect.payload.encode(&mut right);

        assert_eq!(left, right);
    }

    #[test]
    fn split() {
        // Check we split two handshake messages within one PDU.
        let mut hj = HandshakeJoiner::new();

        // two HelloRequests
        let msg = PlainMessage {
            typ: ContentType::Handshake,
            version: ProtocolVersion::TLSv1_2,
            payload: Payload::new(b"\x00\x00\x00\x00\x00\x00\x00\x00".to_vec()),
        };

        assert!(hj.want_message(&msg));
        assert_eq!(hj.take_message(msg), Some(2));
        assert!(hj.is_empty());

        let expect = Message {
            version: ProtocolVersion::TLSv1_2,
            payload: MessagePayload::handshake(HandshakeMessagePayload {
                typ: HandshakeType::HelloRequest,
                payload: HandshakePayload::HelloRequest,
            }),
        }
        .into();

        pop_eq(&expect, &mut hj);
        pop_eq(&expect, &mut hj);
    }

    #[test]
    fn broken() {
        // Check obvious crap payloads are reported as errors, not panics.
        let mut hj = HandshakeJoiner::new();

        // short ClientHello
        let msg = PlainMessage {
            typ: ContentType::Handshake,
            version: ProtocolVersion::TLSv1_2,
            payload: Payload::new(b"\x01\x00\x00\x02\xff\xff".to_vec()),
        };

        assert!(hj.want_message(&msg));
        assert_eq!(hj.take_message(msg), None);
    }

    #[test]
    fn join() {
        // Check we join one handshake message split over two PDUs.
        let mut hj = HandshakeJoiner::new();
        assert!(hj.is_empty());

        // Introduce Finished of 16 bytes, providing 4.
        let mut msg = PlainMessage {
            typ: ContentType::Handshake,
            version: ProtocolVersion::TLSv1_2,
            payload: Payload::new(b"\x14\x00\x00\x10\x00\x01\x02\x03\x04".to_vec()),
        };

        assert!(hj.want_message(&msg));
        assert_eq!(hj.take_message(msg), Some(0));
        assert!(!hj.is_empty());

        // 11 more bytes.
        msg = PlainMessage {
            typ: ContentType::Handshake,
            version: ProtocolVersion::TLSv1_2,
            payload: Payload::new(b"\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e".to_vec()),
        };

        assert!(hj.want_message(&msg));
        assert_eq!(hj.take_message(msg), Some(0));
        assert!(!hj.is_empty());

        // Final 1 byte.
        msg = PlainMessage {
            typ: ContentType::Handshake,
            version: ProtocolVersion::TLSv1_2,
            payload: Payload::new(b"\x0f".to_vec()),
        };

        assert!(hj.want_message(&msg));
        assert_eq!(hj.take_message(msg), Some(1));
        assert!(hj.is_empty());

        let payload = b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\x0f".to_vec();
        let expect = Message {
            version: ProtocolVersion::TLSv1_2,
            payload: MessagePayload::handshake(HandshakeMessagePayload {
                typ: HandshakeType::Finished,
                payload: HandshakePayload::Finished(Payload::new(payload)),
            }),
        }
        .into();

        pop_eq(&expect, &mut hj);
    }

    #[test]
    fn test_rejects_giant_certs() {
        let mut hj = HandshakeJoiner::new();
        let msg = PlainMessage {
            typ: ContentType::Handshake,
            version: ProtocolVersion::TLSv1_2,
            payload: Payload::new(b"\x0b\x01\x00\x04\x01\x00\x01\x00\xff\xfe".to_vec()),
        };

        assert!(hj.want_message(&msg));
        assert_eq!(hj.take_message(msg), None);
        assert!(!hj.is_empty());
    }
}
