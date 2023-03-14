use futures::{
    future::{self, Either},
    stream::{StreamExt, TryStream, TryStreamExt},
    FutureExt,
};

use crate::{
    packet::{constants::*, nlas::link::Nla, LinkMessage, NetlinkMessage, RtnlMessage},
    try_rtnl,
    Error,
    Handle,
};

pub struct LinkGetRequest {
    handle: Handle,
    message: LinkMessage,
    // There are two ways to retrieve links: we can either dump them
    // all and filter the result, or if we already know the index of
    // the link we're looking for, we can just retrieve that one. If
    // `dump` is `true`, all the links are fetched. Otherwise, only
    // the link that match the given index is fetched.
    dump: bool,
    filter_builder: LinkFilterBuilder,
}

impl LinkGetRequest {
    pub(crate) fn new(handle: Handle) -> Self {
        LinkGetRequest {
            handle,
            message: LinkMessage::default(),
            dump: true,
            filter_builder: LinkFilterBuilder::new(),
        }
    }

    /// Setting filter mask(e.g. RTEXT_FILTER_BRVLAN and etc)
    pub fn set_filter_mask(mut self, family: u8, filter_mask: u32) -> Self {
        self.message.header.interface_family = family;
        self.message.nlas.push(Nla::ExtMask(filter_mask));
        self
    }

    /// Execute the request
    pub fn execute(self) -> impl TryStream<Ok = LinkMessage, Error = Error> {
        let LinkGetRequest {
            mut handle,
            message,
            dump,
            filter_builder,
        } = self;

        let mut req = NetlinkMessage::from(RtnlMessage::GetLink(message));

        if dump {
            req.header.flags = NLM_F_REQUEST | NLM_F_DUMP;
        } else {
            req.header.flags = NLM_F_REQUEST;
        }

        let filter = filter_builder.build();
        match handle.request(req) {
            Ok(response) => Either::Left(
                response
                    .map(move |msg| Ok(try_rtnl!(msg, RtnlMessage::NewLink)))
                    .try_filter(move |msg| future::ready(filter(msg))),
            ),
            Err(e) => Either::Right(future::err::<LinkMessage, Error>(e).into_stream()),
        }
    }

    /// Return a mutable reference to the request
    pub fn message_mut(&mut self) -> &mut LinkMessage {
        &mut self.message
    }

    pub fn match_index(mut self, index: u32) -> Self {
        self.dump = false;
        self.message.header.index = index;
        self
    }

    pub fn set_name_filter(mut self, name: String) -> Self {
        self.filter_builder.name = Some(name);
        self
    }
}

#[derive(Default)]
struct LinkFilterBuilder {
    name: Option<String>,
}

impl LinkFilterBuilder {
    fn new() -> Self {
        Default::default()
    }

    fn build(self) -> impl Fn(&LinkMessage) -> bool {
        move |msg: &LinkMessage| {
            if let Some(name) = &self.name {
                for nla in msg.nlas.iter() {
                    if let Nla::IfName(s) = nla {
                        if s == name {
                            return true;
                        }
                    }
                }
                false
            } else {
                true
            }
        }
    }
}
