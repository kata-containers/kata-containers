// SPDX-License-Identifier: MIT

use crate::{
    traits::{Emitable, Parseable},
    DecodeError,
};

pub const LINK_INET6_DEV_CONF_LEN: usize = 204;
buffer!(Inet6DevConfBuffer(LINK_INET6_DEV_CONF_LEN) {
    forwarding: (i32, 0..4),
    hoplimit: (i32, 4..8),
    mtu6: (i32, 8..12),
    accept_ra: (i32, 12..16),
    accept_redirects: (i32, 16..20),
    autoconf: (i32, 20..24),
    dad_transmits: (i32, 24..28),
    rtr_solicits: (i32, 28..32),
    rtr_solicit_interval: (i32, 32..36),
    rtr_solicit_delay: (i32, 36..40),
    use_tempaddr: (i32, 40..44),
    temp_valid_lft: (i32, 44..48),
    temp_prefered_lft: (i32, 48..52),
    regen_max_retry: (i32, 52..56),
    max_desync_factor: (i32, 56..60),
    max_addresses: (i32, 60..64),
    force_mld_version: (i32, 64..68),
    accept_ra_defrtr: (i32, 68..72),
    accept_ra_pinfo: (i32, 72..76),
    accept_ra_rtr_pref: (i32, 76..80),
    rtr_probe_interval: (i32, 80..84),
    accept_ra_rt_info_max_plen: (i32, 84..88),
    proxy_ndp: (i32, 88..92),
    optimistic_dad: (i32, 92..96),
    accept_source_route: (i32, 96..100),
    mc_forwarding: (i32, 100..104),
    disable_ipv6: (i32, 104..108),
    accept_dad: (i32, 108..112),
    force_tllao: (i32, 112..116),
    ndisc_notify: (i32, 116..120),
    mldv1_unsolicited_report_interval: (i32, 120..124),
    mldv2_unsolicited_report_interval: (i32, 124..128),
    suppress_frag_ndisc: (i32, 128..132),
    accept_ra_from_local: (i32, 132..136),
    use_optimistic: (i32, 136..140),
    accept_ra_mtu: (i32, 140..144),
    stable_secret: (i32, 144..148),
    use_oif_addrs_only: (i32, 148..152),
    accept_ra_min_hop_limit: (i32, 152..156),
    ignore_routes_with_linkdown: (i32, 156..160),
    drop_unicast_in_l2_multicast: (i32, 160..164),
    drop_unsolicited_na: (i32, 164..168),
    keep_addr_on_down: (i32, 168..172),
    rtr_solicit_max_interval: (i32, 172..176),
    seg6_enabled: (i32, 176..180),
    seg6_require_hmac: (i32, 180..184),
    enhanced_dad: (i32, 184..188),
    addr_gen_mode: (i32, 188..192),
    disable_policy: (i32, 192..196),
    accept_ra_rt_info_min_plen: (i32, 196..200),
    ndisc_tclass: (i32, 200..204),
});

impl<T: AsRef<[u8]>> Parseable<Inet6DevConfBuffer<T>> for Inet6DevConf {
    fn parse(buf: &Inet6DevConfBuffer<T>) -> Result<Self, DecodeError> {
        Ok(Self {
            forwarding: buf.forwarding(),
            hoplimit: buf.hoplimit(),
            mtu6: buf.mtu6(),
            accept_ra: buf.accept_ra(),
            accept_redirects: buf.accept_redirects(),
            autoconf: buf.autoconf(),
            dad_transmits: buf.dad_transmits(),
            rtr_solicits: buf.rtr_solicits(),
            rtr_solicit_interval: buf.rtr_solicit_interval(),
            rtr_solicit_delay: buf.rtr_solicit_delay(),
            use_tempaddr: buf.use_tempaddr(),
            temp_valid_lft: buf.temp_valid_lft(),
            temp_prefered_lft: buf.temp_prefered_lft(),
            regen_max_retry: buf.regen_max_retry(),
            max_desync_factor: buf.max_desync_factor(),
            max_addresses: buf.max_addresses(),
            force_mld_version: buf.force_mld_version(),
            accept_ra_defrtr: buf.accept_ra_defrtr(),
            accept_ra_pinfo: buf.accept_ra_pinfo(),
            accept_ra_rtr_pref: buf.accept_ra_rtr_pref(),
            rtr_probe_interval: buf.rtr_probe_interval(),
            accept_ra_rt_info_max_plen: buf.accept_ra_rt_info_max_plen(),
            proxy_ndp: buf.proxy_ndp(),
            optimistic_dad: buf.optimistic_dad(),
            accept_source_route: buf.accept_source_route(),
            mc_forwarding: buf.mc_forwarding(),
            disable_ipv6: buf.disable_ipv6(),
            accept_dad: buf.accept_dad(),
            force_tllao: buf.force_tllao(),
            ndisc_notify: buf.ndisc_notify(),
            mldv1_unsolicited_report_interval: buf.mldv1_unsolicited_report_interval(),
            mldv2_unsolicited_report_interval: buf.mldv2_unsolicited_report_interval(),
            suppress_frag_ndisc: buf.suppress_frag_ndisc(),
            accept_ra_from_local: buf.accept_ra_from_local(),
            use_optimistic: buf.use_optimistic(),
            accept_ra_mtu: buf.accept_ra_mtu(),
            stable_secret: buf.stable_secret(),
            use_oif_addrs_only: buf.use_oif_addrs_only(),
            accept_ra_min_hop_limit: buf.accept_ra_min_hop_limit(),
            ignore_routes_with_linkdown: buf.ignore_routes_with_linkdown(),
            drop_unicast_in_l2_multicast: buf.drop_unicast_in_l2_multicast(),
            drop_unsolicited_na: buf.drop_unsolicited_na(),
            keep_addr_on_down: buf.keep_addr_on_down(),
            rtr_solicit_max_interval: buf.rtr_solicit_max_interval(),
            seg6_enabled: buf.seg6_enabled(),
            seg6_require_hmac: buf.seg6_require_hmac(),
            enhanced_dad: buf.enhanced_dad(),
            addr_gen_mode: buf.addr_gen_mode(),
            disable_policy: buf.disable_policy(),
            accept_ra_rt_info_min_plen: buf.accept_ra_rt_info_min_plen(),
            ndisc_tclass: buf.ndisc_tclass(),
        })
    }
}

impl Emitable for Inet6DevConf {
    fn buffer_len(&self) -> usize {
        LINK_INET6_DEV_CONF_LEN
    }

    fn emit(&self, buffer: &mut [u8]) {
        let mut buffer = Inet6DevConfBuffer::new(buffer);
        buffer.set_forwarding(self.forwarding);
        buffer.set_hoplimit(self.hoplimit);
        buffer.set_mtu6(self.mtu6);
        buffer.set_accept_ra(self.accept_ra);
        buffer.set_accept_redirects(self.accept_redirects);
        buffer.set_autoconf(self.autoconf);
        buffer.set_dad_transmits(self.dad_transmits);
        buffer.set_rtr_solicits(self.rtr_solicits);
        buffer.set_rtr_solicit_interval(self.rtr_solicit_interval);
        buffer.set_rtr_solicit_delay(self.rtr_solicit_delay);
        buffer.set_use_tempaddr(self.use_tempaddr);
        buffer.set_temp_valid_lft(self.temp_valid_lft);
        buffer.set_temp_prefered_lft(self.temp_prefered_lft);
        buffer.set_regen_max_retry(self.regen_max_retry);
        buffer.set_max_desync_factor(self.max_desync_factor);
        buffer.set_max_addresses(self.max_addresses);
        buffer.set_force_mld_version(self.force_mld_version);
        buffer.set_accept_ra_defrtr(self.accept_ra_defrtr);
        buffer.set_accept_ra_pinfo(self.accept_ra_pinfo);
        buffer.set_accept_ra_rtr_pref(self.accept_ra_rtr_pref);
        buffer.set_rtr_probe_interval(self.rtr_probe_interval);
        buffer.set_accept_ra_rt_info_max_plen(self.accept_ra_rt_info_max_plen);
        buffer.set_proxy_ndp(self.proxy_ndp);
        buffer.set_optimistic_dad(self.optimistic_dad);
        buffer.set_accept_source_route(self.accept_source_route);
        buffer.set_mc_forwarding(self.mc_forwarding);
        buffer.set_disable_ipv6(self.disable_ipv6);
        buffer.set_accept_dad(self.accept_dad);
        buffer.set_force_tllao(self.force_tllao);
        buffer.set_ndisc_notify(self.ndisc_notify);
        buffer.set_mldv1_unsolicited_report_interval(self.mldv1_unsolicited_report_interval);
        buffer.set_mldv2_unsolicited_report_interval(self.mldv2_unsolicited_report_interval);
        buffer.set_suppress_frag_ndisc(self.suppress_frag_ndisc);
        buffer.set_accept_ra_from_local(self.accept_ra_from_local);
        buffer.set_use_optimistic(self.use_optimistic);
        buffer.set_accept_ra_mtu(self.accept_ra_mtu);
        buffer.set_stable_secret(self.stable_secret);
        buffer.set_use_oif_addrs_only(self.use_oif_addrs_only);
        buffer.set_accept_ra_min_hop_limit(self.accept_ra_min_hop_limit);
        buffer.set_ignore_routes_with_linkdown(self.ignore_routes_with_linkdown);
        buffer.set_drop_unicast_in_l2_multicast(self.drop_unicast_in_l2_multicast);
        buffer.set_drop_unsolicited_na(self.drop_unsolicited_na);
        buffer.set_keep_addr_on_down(self.keep_addr_on_down);
        buffer.set_rtr_solicit_max_interval(self.rtr_solicit_max_interval);
        buffer.set_seg6_enabled(self.seg6_enabled);
        buffer.set_seg6_require_hmac(self.seg6_require_hmac);
        buffer.set_enhanced_dad(self.enhanced_dad);
        buffer.set_addr_gen_mode(self.addr_gen_mode);
        buffer.set_disable_policy(self.disable_policy);
        buffer.set_accept_ra_rt_info_min_plen(self.accept_ra_rt_info_min_plen);
        buffer.set_ndisc_tclass(self.ndisc_tclass);
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct Inet6DevConf {
    pub forwarding: i32,
    pub hoplimit: i32,
    pub mtu6: i32,
    pub accept_ra: i32,
    pub accept_redirects: i32,
    pub autoconf: i32,
    pub dad_transmits: i32,
    pub rtr_solicits: i32,
    pub rtr_solicit_interval: i32,
    pub rtr_solicit_delay: i32,
    pub use_tempaddr: i32,
    pub temp_valid_lft: i32,
    pub temp_prefered_lft: i32,
    pub regen_max_retry: i32,
    pub max_desync_factor: i32,
    pub max_addresses: i32,
    pub force_mld_version: i32,
    pub accept_ra_defrtr: i32,
    pub accept_ra_pinfo: i32,
    pub accept_ra_rtr_pref: i32,
    pub rtr_probe_interval: i32,
    pub accept_ra_rt_info_max_plen: i32,
    pub proxy_ndp: i32,
    pub optimistic_dad: i32,
    pub accept_source_route: i32,
    pub mc_forwarding: i32,
    pub disable_ipv6: i32,
    pub accept_dad: i32,
    pub force_tllao: i32,
    pub ndisc_notify: i32,
    pub mldv1_unsolicited_report_interval: i32,
    pub mldv2_unsolicited_report_interval: i32,
    pub suppress_frag_ndisc: i32,
    pub accept_ra_from_local: i32,
    pub use_optimistic: i32,
    pub accept_ra_mtu: i32,
    pub stable_secret: i32,
    pub use_oif_addrs_only: i32,
    pub accept_ra_min_hop_limit: i32,
    pub ignore_routes_with_linkdown: i32,
    pub drop_unicast_in_l2_multicast: i32,
    pub drop_unsolicited_na: i32,
    pub keep_addr_on_down: i32,
    pub rtr_solicit_max_interval: i32,
    pub seg6_enabled: i32,
    pub seg6_require_hmac: i32,
    pub enhanced_dad: i32,
    pub addr_gen_mode: i32,
    pub disable_policy: i32,
    pub accept_ra_rt_info_min_plen: i32,
    pub ndisc_tclass: i32,
}
