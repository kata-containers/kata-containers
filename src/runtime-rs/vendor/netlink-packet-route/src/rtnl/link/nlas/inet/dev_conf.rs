// SPDX-License-Identifier: MIT

use crate::{
    traits::{Emitable, Parseable},
    DecodeError,
};

pub const DEV_CONF_LEN: usize = 124;

buffer!(InetDevConfBuffer(DEV_CONF_LEN) {
    forwarding: (i32, 0..4),
    mc_forwarding: (i32, 4..8),
    proxy_arp: (i32, 8..12),
    accept_redirects: (i32, 12..16),
    secure_redirects: (i32, 16..20),
    send_redirects: (i32, 20..24),
    shared_media: (i32, 24..28),
    rp_filter: (i32, 28..32),
    accept_source_route: (i32, 32..36),
    bootp_relay: (i32, 36..40),
    log_martians: (i32, 40..44),
    tag: (i32, 44..48),
    arpfilter: (i32, 48..52),
    medium_id: (i32, 52..56),
    noxfrm: (i32, 56..60),
    nopolicy: (i32, 60..64),
    force_igmp_version: (i32, 64..68),
    arp_announce: (i32, 68..72),
    arp_ignore: (i32, 72..76),
    promote_secondaries: (i32, 76..80),
    arp_accept: (i32, 80..84),
    arp_notify: (i32, 84..88),
    accept_local: (i32, 88..92),
    src_vmark: (i32, 92..96),
    proxy_arp_pvlan: (i32, 96..100),
    route_localnet: (i32, 100..104),
    igmpv2_unsolicited_report_interval: (i32, 104..108),
    igmpv3_unsolicited_report_interval: (i32, 108..112),
    ignore_routes_with_linkdown: (i32, 112..116),
    drop_unicast_in_l2_multicast: (i32, 116..120),
    drop_gratuitous_arp: (i32, 120..124),
});

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct InetDevConf {
    pub forwarding: i32,
    pub mc_forwarding: i32,
    pub proxy_arp: i32,
    pub accept_redirects: i32,
    pub secure_redirects: i32,
    pub send_redirects: i32,
    pub shared_media: i32,
    pub rp_filter: i32,
    pub accept_source_route: i32,
    pub bootp_relay: i32,
    pub log_martians: i32,
    pub tag: i32,
    pub arpfilter: i32,
    pub medium_id: i32,
    pub noxfrm: i32,
    pub nopolicy: i32,
    pub force_igmp_version: i32,
    pub arp_announce: i32,
    pub arp_ignore: i32,
    pub promote_secondaries: i32,
    pub arp_accept: i32,
    pub arp_notify: i32,
    pub accept_local: i32,
    pub src_vmark: i32,
    pub proxy_arp_pvlan: i32,
    pub route_localnet: i32,
    pub igmpv2_unsolicited_report_interval: i32,
    pub igmpv3_unsolicited_report_interval: i32,
    pub ignore_routes_with_linkdown: i32,
    pub drop_unicast_in_l2_multicast: i32,
    pub drop_gratuitous_arp: i32,
}

impl<T: AsRef<[u8]>> Parseable<InetDevConfBuffer<T>> for InetDevConf {
    fn parse(buf: &InetDevConfBuffer<T>) -> Result<Self, DecodeError> {
        Ok(Self {
            forwarding: buf.forwarding(),
            mc_forwarding: buf.mc_forwarding(),
            proxy_arp: buf.proxy_arp(),
            accept_redirects: buf.accept_redirects(),
            secure_redirects: buf.secure_redirects(),
            send_redirects: buf.send_redirects(),
            shared_media: buf.shared_media(),
            rp_filter: buf.rp_filter(),
            accept_source_route: buf.accept_source_route(),
            bootp_relay: buf.bootp_relay(),
            log_martians: buf.log_martians(),
            tag: buf.tag(),
            arpfilter: buf.arpfilter(),
            medium_id: buf.medium_id(),
            noxfrm: buf.noxfrm(),
            nopolicy: buf.nopolicy(),
            force_igmp_version: buf.force_igmp_version(),
            arp_announce: buf.arp_announce(),
            arp_ignore: buf.arp_ignore(),
            promote_secondaries: buf.promote_secondaries(),
            arp_accept: buf.arp_accept(),
            arp_notify: buf.arp_notify(),
            accept_local: buf.accept_local(),
            src_vmark: buf.src_vmark(),
            proxy_arp_pvlan: buf.proxy_arp_pvlan(),
            route_localnet: buf.route_localnet(),
            igmpv2_unsolicited_report_interval: buf.igmpv2_unsolicited_report_interval(),
            igmpv3_unsolicited_report_interval: buf.igmpv3_unsolicited_report_interval(),
            ignore_routes_with_linkdown: buf.ignore_routes_with_linkdown(),
            drop_unicast_in_l2_multicast: buf.drop_unicast_in_l2_multicast(),
            drop_gratuitous_arp: buf.drop_gratuitous_arp(),
        })
    }
}

impl Emitable for InetDevConf {
    fn buffer_len(&self) -> usize {
        DEV_CONF_LEN
    }

    fn emit(&self, buffer: &mut [u8]) {
        let mut buffer = InetDevConfBuffer::new(buffer);
        buffer.set_forwarding(self.forwarding);
        buffer.set_mc_forwarding(self.mc_forwarding);
        buffer.set_proxy_arp(self.proxy_arp);
        buffer.set_accept_redirects(self.accept_redirects);
        buffer.set_secure_redirects(self.secure_redirects);
        buffer.set_send_redirects(self.send_redirects);
        buffer.set_shared_media(self.shared_media);
        buffer.set_rp_filter(self.rp_filter);
        buffer.set_accept_source_route(self.accept_source_route);
        buffer.set_bootp_relay(self.bootp_relay);
        buffer.set_log_martians(self.log_martians);
        buffer.set_tag(self.tag);
        buffer.set_arpfilter(self.arpfilter);
        buffer.set_medium_id(self.medium_id);
        buffer.set_noxfrm(self.noxfrm);
        buffer.set_nopolicy(self.nopolicy);
        buffer.set_force_igmp_version(self.force_igmp_version);
        buffer.set_arp_announce(self.arp_announce);
        buffer.set_arp_ignore(self.arp_ignore);
        buffer.set_promote_secondaries(self.promote_secondaries);
        buffer.set_arp_accept(self.arp_accept);
        buffer.set_arp_notify(self.arp_notify);
        buffer.set_accept_local(self.accept_local);
        buffer.set_src_vmark(self.src_vmark);
        buffer.set_proxy_arp_pvlan(self.proxy_arp_pvlan);
        buffer.set_route_localnet(self.route_localnet);
        buffer.set_igmpv2_unsolicited_report_interval(self.igmpv2_unsolicited_report_interval);
        buffer.set_igmpv3_unsolicited_report_interval(self.igmpv3_unsolicited_report_interval);
        buffer.set_ignore_routes_with_linkdown(self.ignore_routes_with_linkdown);
        buffer.set_drop_unicast_in_l2_multicast(self.drop_unicast_in_l2_multicast);
        buffer.set_drop_gratuitous_arp(self.drop_gratuitous_arp);
    }
}
