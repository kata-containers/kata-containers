// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use netlink_packet_route::{
    link::nlas::{Info, InfoBridge, InfoData, InfoKind, Nla},
    LinkMessage,
};

use super::{Link, LinkAttrs};

#[allow(clippy::box_default)]
pub fn get_link_from_message(mut msg: LinkMessage) -> Box<dyn Link> {
    let mut base = LinkAttrs {
        index: msg.header.index,
        flags: msg.header.flags,
        link_layer_type: msg.header.link_layer_type,
        ..Default::default()
    };
    if msg.header.flags & libc::IFF_PROMISC as u32 != 0 {
        base.promisc = 1;
    }
    let mut link: Option<Box<dyn Link>> = None;
    while let Some(attr) = msg.nlas.pop() {
        match attr {
            Nla::Info(infos) => {
                link = Some(link_info(infos));
            }
            Nla::Address(a) => {
                base.hardware_addr = a;
            }
            Nla::IfName(i) => {
                base.name = i;
            }
            Nla::Mtu(m) => {
                base.mtu = m;
            }
            Nla::Link(l) => {
                base.parent_index = l;
            }
            Nla::Master(m) => {
                base.master_index = m;
            }
            Nla::TxQueueLen(t) => {
                base.txq_len = t;
            }
            Nla::IfAlias(a) => {
                base.alias = a;
            }
            Nla::Stats(_s) => {}
            Nla::Stats64(_s) => {}
            Nla::Xdp(_x) => {}
            Nla::ProtoInfo(_) => {}
            Nla::OperState(_) => {}
            Nla::NetnsId(n) => {
                base.net_ns_id = n;
            }
            Nla::GsoMaxSize(i) => {
                base.gso_max_size = i;
            }
            Nla::GsoMaxSegs(e) => {
                base.gso_max_seqs = e;
            }
            Nla::VfInfoList(_) => {}
            Nla::NumTxQueues(t) => {
                base.num_tx_queues = t;
            }
            Nla::NumRxQueues(r) => {
                base.num_rx_queues = r;
            }
            Nla::Group(g) => {
                base.group = g;
            }
            _ => {
                // skip unused attr
            }
        }
    }

    let mut ret = link.unwrap_or_else(|| Box::new(Device::default()));
    ret.set_attrs(base);
    ret
}

#[allow(clippy::box_default)]
fn link_info(mut infos: Vec<Info>) -> Box<dyn Link> {
    let mut link: Option<Box<dyn Link>> = None;
    while let Some(info) = infos.pop() {
        match info {
            Info::Kind(kind) => match kind {
                InfoKind::Tun => {
                    if link.is_none() {
                        link = Some(Box::new(Tuntap::default()));
                    }
                }
                InfoKind::Veth => {
                    if link.is_none() {
                        link = Some(Box::new(Veth::default()));
                    }
                }
                InfoKind::IpVlan => {
                    if link.is_none() {
                        link = Some(Box::new(IpVlan::default()));
                    }
                }
                InfoKind::MacVlan => {
                    if link.is_none() {
                        link = Some(Box::new(MacVlan::default()));
                    }
                }
                InfoKind::Vlan => {
                    if link.is_none() {
                        link = Some(Box::new(Vlan::default()));
                    }
                }
                InfoKind::Bridge => {
                    if link.is_none() {
                        link = Some(Box::new(Bridge::default()));
                    }
                }
                _ => {
                    if link.is_none() {
                        link = Some(Box::new(Device::default()));
                    }
                }
            },
            Info::Data(data) => match data {
                InfoData::Tun(_) => {
                    link = Some(Box::new(Tuntap::default()));
                }
                InfoData::Veth(_) => {
                    link = Some(Box::new(Veth::default()));
                }
                InfoData::IpVlan(_) => {
                    link = Some(Box::new(IpVlan::default()));
                }
                InfoData::MacVlan(_) => {
                    link = Some(Box::new(MacVlan::default()));
                }
                InfoData::Vlan(_) => {
                    link = Some(Box::new(Vlan::default()));
                }
                InfoData::Bridge(ibs) => {
                    link = Some(Box::new(parse_bridge(ibs)));
                }
                _ => {
                    link = Some(Box::new(Device::default()));
                }
            },
            Info::SlaveKind(_sk) => {
                if link.is_none() {
                    link = Some(Box::new(Device::default()));
                }
            }
            Info::SlaveData(_sd) => {
                link = Some(Box::new(Device::default()));
            }
            _ => {
                link = Some(Box::new(Device::default()));
            }
        }
    }
    link.unwrap()
}

fn parse_bridge(mut ibs: Vec<InfoBridge>) -> Bridge {
    let mut bridge = Bridge::default();
    while let Some(ib) = ibs.pop() {
        match ib {
            InfoBridge::HelloTime(ht) => {
                bridge.hello_time = ht;
            }
            InfoBridge::MulticastSnooping(m) => {
                bridge.multicast_snooping = m == 1;
            }
            InfoBridge::VlanFiltering(v) => {
                bridge.vlan_filtering = v == 1;
            }
            _ => {}
        }
    }
    bridge
}

macro_rules! impl_network_dev {
    ($r_type: literal , $r_struct: ty) => {
        impl Link for $r_struct {
            fn attrs(&self) -> &LinkAttrs {
                self.attrs.as_ref().unwrap()
            }
            fn set_attrs(&mut self, attr: LinkAttrs) {
                self.attrs = Some(attr);
            }
            fn r#type(&self) -> &'static str {
                $r_type
            }
        }
    };
}

macro_rules! define_and_impl_network_dev {
    ($r_type: literal , $r_struct: tt) => {
        #[derive(Debug, PartialEq, Eq, Clone, Default)]
        pub struct $r_struct {
            attrs: Option<LinkAttrs>,
        }

        impl_network_dev!($r_type, $r_struct);
    };
}

define_and_impl_network_dev!("device", Device);
define_and_impl_network_dev!("tuntap", Tuntap);
define_and_impl_network_dev!("veth", Veth);
define_and_impl_network_dev!("ipvlan", IpVlan);
define_and_impl_network_dev!("macvlan", MacVlan);
define_and_impl_network_dev!("vlan", Vlan);

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct Bridge {
    attrs: Option<LinkAttrs>,
    pub multicast_snooping: bool,
    pub hello_time: u32,
    pub vlan_filtering: bool,
}

impl_network_dev!("bridge", Bridge);
