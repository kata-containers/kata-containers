// SPDX-License-Identifier: MIT

use crate::{
    constants::*,
    traits::{Parseable, ParseableParametrized},
    AddressHeader,
    AddressMessage,
    AddressMessageBuffer,
    DecodeError,
    LinkMessage,
    LinkMessageBuffer,
    NeighbourMessage,
    NeighbourMessageBuffer,
    NeighbourTableMessage,
    NeighbourTableMessageBuffer,
    NsidMessage,
    NsidMessageBuffer,
    RouteHeader,
    RouteMessage,
    RouteMessageBuffer,
    RtnlMessage,
    RuleMessage,
    RuleMessageBuffer,
    TcMessage,
    TcMessageBuffer,
};
use anyhow::Context;

buffer!(RtnlMessageBuffer);

impl<'a, T: AsRef<[u8]> + ?Sized> ParseableParametrized<RtnlMessageBuffer<&'a T>, u16>
    for RtnlMessage
{
    #[rustfmt::skip]
    fn parse_with_param(buf: &RtnlMessageBuffer<&'a T>, message_type: u16) -> Result<Self, DecodeError> {
        use self::RtnlMessage::*;
        let message = match message_type {

            // Link messages
            RTM_NEWLINK | RTM_GETLINK | RTM_DELLINK | RTM_SETLINK => {
                let msg = match LinkMessageBuffer::new_checked(&buf.inner()) {
                    Ok(buf) => LinkMessage::parse(&buf).context("invalid link message")?,
                    // HACK: iproute2 sends invalid RTM_GETLINK message, where the header is
                    // limited to the interface family (1 byte) and 3 bytes of padding.
                    Err(e) => {
                        if buf.inner().len() == 4 && message_type == RTM_GETLINK {
                            let mut msg = LinkMessage::default();
                            msg.header.interface_family = buf.inner()[0];
                            msg
                        } else {
                            return Err(e);
                        }
                    }
                };
                match message_type {
                    RTM_NEWLINK => NewLink(msg),
                    RTM_GETLINK => GetLink(msg),
                    RTM_DELLINK => DelLink(msg),
                    RTM_SETLINK => SetLink(msg),
                    _ => unreachable!(),
                }
            }

            // Address messages
            RTM_NEWADDR | RTM_GETADDR | RTM_DELADDR => {
                let msg = match AddressMessageBuffer::new_checked(&buf.inner()) {
                    Ok(buf) => AddressMessage::parse(&buf).context("invalid link message")?,
                    // HACK: iproute2 sends invalid RTM_GETADDR message, where the header is
                    // limited to the interface family (1 byte) and 3 bytes of padding.
                    Err(e) => {
                        if buf.inner().len() == 4 && message_type == RTM_GETADDR {
                            let mut msg = AddressMessage {
                                header: AddressHeader::default(),
                                nlas: vec![],
                            };
                            msg.header.family = buf.inner()[0];
                            msg
                        } else {
                            return Err(e);
                        }
                    }
                };
                match message_type {
                    RTM_NEWADDR => NewAddress(msg),
                    RTM_GETADDR => GetAddress(msg),
                    RTM_DELADDR => DelAddress(msg),
                    _ => unreachable!(),
                }
            }

            // Neighbour messages
            RTM_NEWNEIGH | RTM_GETNEIGH | RTM_DELNEIGH => {
                let err = "invalid neighbour message";
                let msg = NeighbourMessage::parse(&NeighbourMessageBuffer::new_checked(&buf.inner()).context(err)?).context(err)?;
                match message_type {
                    RTM_GETNEIGH => GetNeighbour(msg),
                    RTM_NEWNEIGH => NewNeighbour(msg),
                    RTM_DELNEIGH => DelNeighbour(msg),
                    _ => unreachable!(),
                }
            }

            // Neighbour table messages
            RTM_NEWNEIGHTBL | RTM_GETNEIGHTBL | RTM_SETNEIGHTBL => {
                let err = "invalid neighbour table message";
                let msg = NeighbourTableMessage::parse(&NeighbourTableMessageBuffer::new_checked(&buf.inner()).context(err)?).context(err)?;
                match message_type {
                    RTM_GETNEIGHTBL => GetNeighbourTable(msg),
                    RTM_NEWNEIGHTBL => NewNeighbourTable(msg),
                    RTM_SETNEIGHTBL => SetNeighbourTable(msg),
                    _ => unreachable!(),
                }
            }

            // Route messages
            RTM_NEWROUTE | RTM_GETROUTE | RTM_DELROUTE => {
                let msg = match RouteMessageBuffer::new_checked(&buf.inner()) {
                    Ok(buf) => RouteMessage::parse(&buf).context("invalid route message")?,
                    // HACK: iproute2 sends invalid RTM_GETROUTE message, where the header is
                    // limited to the interface family (1 byte) and 3 bytes of padding.
                    Err(e) => {
                        // Not only does iproute2 sends invalid messages, it's also inconsistent in
                        // doing so: for link and address messages, the length advertised in the
                        // netlink header includes the 3 bytes of padding but it does not seem to
                        // be the case for the route message, hence the buf.length() == 1 check.
                        if (buf.inner().len() == 4 || buf.inner().len() == 1) && message_type == RTM_GETROUTE {
                            let mut msg = RouteMessage {
                                header: RouteHeader::default(),
                                nlas: vec![],
                            };
                            msg.header.address_family = buf.inner()[0];
                            msg
                        } else {
                            return Err(e);
                        }
                    }
                };
                match message_type {
                    RTM_NEWROUTE => NewRoute(msg),
                    RTM_GETROUTE => GetRoute(msg),
                    RTM_DELROUTE => DelRoute(msg),
                    _ => unreachable!(),
                }
            }

            RTM_NEWRULE | RTM_GETRULE | RTM_DELRULE => {
                let err = "invalid fib rule message";
                let msg = RuleMessage::parse(&RuleMessageBuffer::new_checked(&buf.inner()).context(err)?).context(err)?;
                match message_type {
                    RTM_NEWRULE => NewRule(msg),
                    RTM_DELRULE => DelRule(msg),
                    RTM_GETRULE => GetRule(msg),
                    _ => unreachable!()
                }
            }
            // TC Messages
            RTM_NEWQDISC | RTM_DELQDISC | RTM_GETQDISC |
            RTM_NEWTCLASS | RTM_DELTCLASS | RTM_GETTCLASS |
            RTM_NEWTFILTER | RTM_DELTFILTER | RTM_GETTFILTER |
            RTM_NEWCHAIN | RTM_DELCHAIN | RTM_GETCHAIN => {
                let err = "invalid tc message";
                let msg = TcMessage::parse(&TcMessageBuffer::new_checked(&buf.inner()).context(err)?).context(err)?;
                match message_type {
                    RTM_NEWQDISC => NewQueueDiscipline(msg),
                    RTM_DELQDISC => DelQueueDiscipline(msg),
                    RTM_GETQDISC => GetQueueDiscipline(msg),
                    RTM_NEWTCLASS => NewTrafficClass(msg),
                    RTM_DELTCLASS => DelTrafficClass(msg),
                    RTM_GETTCLASS => GetTrafficClass(msg),
                    RTM_NEWTFILTER => NewTrafficFilter(msg),
                    RTM_DELTFILTER => DelTrafficFilter(msg),
                    RTM_GETTFILTER => GetTrafficFilter(msg),
                    RTM_NEWCHAIN => NewTrafficChain(msg),
                    RTM_DELCHAIN => DelTrafficChain(msg),
                    RTM_GETCHAIN => GetTrafficChain(msg),
                    _ => unreachable!(),
                }
            }

            // ND ID Messages
            RTM_NEWNSID | RTM_GETNSID | RTM_DELNSID => {
                let err = "invalid nsid message";
                let msg = NsidMessage::parse(&NsidMessageBuffer::new_checked(&buf.inner()).context(err)?).context(err)?;
                match message_type {
                    RTM_NEWNSID => NewNsId(msg),
                    RTM_DELNSID => DelNsId(msg),
                    RTM_GETNSID => GetNsId(msg),
                    _ => unreachable!(),
                }
            }

            _ => return Err(format!("Unknown message type: {}", message_type).into()),
        };
        Ok(message)
    }
}
