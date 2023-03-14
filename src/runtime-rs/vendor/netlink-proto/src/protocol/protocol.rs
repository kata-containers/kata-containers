// SPDX-License-Identifier: MIT

use std::{
    collections::{hash_map, HashMap, VecDeque},
    fmt::Debug,
};

use netlink_packet_core::{
    constants::*,
    NetlinkDeserializable,
    NetlinkMessage,
    NetlinkPayload,
    NetlinkSerializable,
};

use super::Request;
use crate::sys::SocketAddr;

#[derive(Debug, Eq, PartialEq, Hash)]
struct RequestId {
    sequence_number: u32,
    port: u32,
}

impl RequestId {
    fn new(sequence_number: u32, port: u32) -> Self {
        Self {
            sequence_number,
            port,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Response<T, M> {
    pub done: bool,
    pub message: NetlinkMessage<T>,
    pub metadata: M,
}

#[derive(Debug)]
struct PendingRequest<M> {
    expecting_ack: bool,
    metadata: M,
}

#[derive(Debug, Default)]
pub(crate) struct Protocol<T, M> {
    /// Counter that is incremented for each message sent
    sequence_id: u32,

    /// Requests for which we're awaiting a response. Metadata are
    /// associated with each request.
    pending_requests: HashMap<RequestId, PendingRequest<M>>,

    /// Responses to pending requests
    pub incoming_responses: VecDeque<Response<T, M>>,

    /// Requests from remote peers
    pub incoming_requests: VecDeque<(NetlinkMessage<T>, SocketAddr)>,

    /// The messages to be sent out
    pub outgoing_messages: VecDeque<(NetlinkMessage<T>, SocketAddr)>,
}

impl<T, M> Protocol<T, M>
where
    T: Debug + NetlinkSerializable + NetlinkDeserializable,
    M: Debug + Clone,
{
    pub fn new() -> Self {
        Self {
            sequence_id: 0,
            pending_requests: HashMap::new(),
            incoming_responses: VecDeque::new(),
            incoming_requests: VecDeque::new(),
            outgoing_messages: VecDeque::new(),
        }
    }

    pub fn handle_message(&mut self, message: NetlinkMessage<T>, source: SocketAddr) {
        let request_id = RequestId::new(message.header.sequence_number, source.port_number());
        debug!("handling messages (request id = {:?})", request_id);
        if let hash_map::Entry::Occupied(entry) = self.pending_requests.entry(request_id) {
            Self::handle_response(&mut self.incoming_responses, entry, message);
        } else {
            self.incoming_requests.push_back((message, source));
        }
    }

    fn handle_response(
        incoming_responses: &mut VecDeque<Response<T, M>>,
        entry: hash_map::OccupiedEntry<RequestId, PendingRequest<M>>,
        message: NetlinkMessage<T>,
    ) {
        let entry_key;
        let mut request_id = entry.key();
        debug!("handling response to request {:?}", request_id);

        // A request is processed if we receive an Ack, Error,
        // Done, Overrun, or InnerMessage without the
        // multipart flag and we were not expecting an Ack
        let done = match message.payload {
            NetlinkPayload::InnerMessage(_)
                if message.header.flags & NLM_F_MULTIPART == NLM_F_MULTIPART =>
            {
                false
            }
            NetlinkPayload::InnerMessage(_) => !entry.get().expecting_ack,
            _ => true,
        };

        let metadata = if done {
            trace!("request {:?} fully processed", request_id);
            let (k, v) = entry.remove_entry();
            entry_key = k;
            request_id = &entry_key;
            v.metadata
        } else {
            trace!("more responses to request {:?} may come", request_id);
            entry.get().metadata.clone()
        };

        let response = Response::<T, M> {
            done,
            message,
            metadata,
        };
        incoming_responses.push_back(response);
        debug!("done handling response to request {:?}", request_id);
    }

    pub fn request(&mut self, request: Request<T, M>) {
        let Request {
            mut message,
            metadata,
            destination,
        } = request;

        self.set_sequence_id(&mut message);
        let request_id = RequestId::new(self.sequence_id, destination.port_number());
        let flags = message.header.flags;
        self.outgoing_messages.push_back((message, destination));

        // If we expect a response, we store the request id so that we
        // can map the response to this specific request.
        //
        // Note that we expect responses in three cases only:
        //  - when the request has the NLM_F_REQUEST flag
        //  - when the request has the NLM_F_ACK flag
        //  - when the request has the NLM_F_ECHO flag
        let expecting_ack = flags & NLM_F_ACK == NLM_F_ACK;
        if flags & NLM_F_REQUEST == NLM_F_REQUEST
            || flags & NLM_F_ECHO == NLM_F_ECHO
            || expecting_ack
        {
            self.pending_requests.insert(
                request_id,
                PendingRequest {
                    expecting_ack,
                    metadata,
                },
            );
        }
    }

    fn set_sequence_id(&mut self, message: &mut NetlinkMessage<T>) {
        self.sequence_id += 1;
        message.header.sequence_number = self.sequence_id;
    }
}
