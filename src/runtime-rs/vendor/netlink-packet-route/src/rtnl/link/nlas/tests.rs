// SPDX-License-Identifier: MIT

use crate::{utils::nla::Nla, DecodeError};

use super::*;
// https://lists.infradead.org/pipermail/libnl/2015-November/002034.html
// https://elixir.bootlin.com/linux/latest/source/include/uapi/linux/if_link.h#L89
#[rustfmt::skip]
static BYTES: [u8; 748] = [
    // AF_SPEC (L=748, T=26)
    0xec, 0x02, 0x1a, 0x00,
        // AF_INET (L=132, T=2)
        0x84, 0x00, 0x02, 0x00,
            // IFLA_INET_CONF (L=128, T=1)
            0x80, 0x00, 0x01, 0x00,
            0x01, 0x00, 0x00, 0x00, // 1  forwarding
            0x00, 0x00, 0x00, 0x00, // 2  mc_forwarding
            0x00, 0x00, 0x00, 0x00, // 3  proxy_arp
            0x01, 0x00, 0x00, 0x00, // 4  accept_redirects
            0x01, 0x00, 0x00, 0x00, // 5  secure_redirects
            0x01, 0x00, 0x00, 0x00, // 6  send_redirects
            0x01, 0x00, 0x00, 0x00, // 7  shared_media
            0x00, 0x00, 0x00, 0x00, // 8  rp_filter
            0x01, 0x00, 0x00, 0x00, // 9  accept_source_route
            0x00, 0x00, 0x00, 0x00, // 10 bootp_relay   (40 bytes)
            0x00, 0x00, 0x00, 0x00, // 11 log_martians
            0x00, 0x00, 0x00, 0x00, // 12 tag
            0x00, 0x00, 0x00, 0x00, // 13 arpfilter
            0x00, 0x00, 0x00, 0x00, // 14 medium_id
            0x01, 0x00, 0x00, 0x00, // 15 noxfrm
            0x01, 0x00, 0x00, 0x00, // 16 nopolicy
            0x00, 0x00, 0x00, 0x00, // 17 force_igmp_version
            0x00, 0x00, 0x00, 0x00, // 18 arp_announce
            0x00, 0x00, 0x00, 0x00, // 19 arp_ignore
            0x00, 0x00, 0x00, 0x00, // 20 promote_secondaries  (80 bytes)
            0x00, 0x00, 0x00, 0x00, // 21 arp_accept
            0x00, 0x00, 0x00, 0x00, // 22 arp_notify
            0x00, 0x00, 0x00, 0x00, // 23 accept_local
            0x00, 0x00, 0x00, 0x00, // 24 src_vmark
            0x00, 0x00, 0x00, 0x00, // 25 proxy_arp_pvlan
            0x00, 0x00, 0x00, 0x00, // 26 route_localnet
            0x10, 0x27, 0x00, 0x00, // 27 igmpv2_unsolicited_report_interval
            0xe8, 0x03, 0x00, 0x00, // 28 igmpv3_unsolicited_report_interval
            0x00, 0x00, 0x00, 0x00, // 29 ignore_routes_with_linkdown
            0x00, 0x00, 0x00, 0x00, // 30 drop_unicast_in_l2_multicast  (120 bytes)
            0x00, 0x00, 0x00, 0x00, // 31 drop_gratuitous_arp

        // AF_INET6 (L=612, T=10)
        0x64, 0x02, 0x0a, 0x00,
            // IFLA_INET6_FLAGS (L=8,T=1)
            0x08, 0x00, 0x01, 0x00,
            0x00, 0x00, 0x00, 0x80,

            // IFLA_INET6_CACHEINFO (L=20, T=5)
            0x14, 0x00, 0x05, 0x00,
            0xff, 0xff, 0x00, 0x00, // max_reasm_len
            0xaf, 0x00, 0x00, 0x00, // tstamp
            0x82, 0x64, 0x00, 0x00, // reachable_time
            0xe8, 0x03, 0x00, 0x00, // retrans_time

            // IFLA_INET6_CONF (L=208, T=2)
            0xd0, 0x00, 0x02, 0x00,
            0x00, 0x00, 0x00, 0x00, // forwarding
            0x40, 0x00, 0x00, 0x00, // hoplimit
            0x00, 0x00, 0x01, 0x00, // mtu6
            0x01, 0x00, 0x00, 0x00, // accept_ra
            0x01, 0x00, 0x00, 0x00, // accept_redirects
            0x01, 0x00, 0x00, 0x00, // autoconf
            0x01, 0x00, 0x00, 0x00, // dad_transmits
            0xff, 0xff, 0xff, 0xff, // rtr_solicits
            0xa0, 0x0f, 0x00, 0x00, // rtr_solicit_interval
            0xe8, 0x03, 0x00, 0x00, // rtr_solicit_delay
            0xff, 0xff, 0xff, 0xff, // use_tempaddr
            0x80, 0x3a, 0x09, 0x00, // temp_valid_lft
            0x80, 0x51, 0x01, 0x00, // temp_prefered_lft
            0x03, 0x00, 0x00, 0x00, // regen_max_retry
            0x58, 0x02, 0x00, 0x00, // max_desync_factor
            0x10, 0x00, 0x00, 0x00, // max_addresses
            0x00, 0x00, 0x00, 0x00, // force_mld_version
            0x01, 0x00, 0x00, 0x00, // accept_ra_defrtr
            0x01, 0x00, 0x00, 0x00, // accept_ra_pinfo
            0x01, 0x00, 0x00, 0x00, // accept_ra_rtr_pref
            0x60, 0xea, 0x00, 0x00, // rtr_probe_interval
            0x00, 0x00, 0x00, 0x00, // accept_ra_rt_info_max_plen
            0x00, 0x00, 0x00, 0x00, // proxy_ndp
            0x00, 0x00, 0x00, 0x00, // optimistic_dad
            0x00, 0x00, 0x00, 0x00, // accept_source_route
            0x00, 0x00, 0x00, 0x00, // mc_forwarding
            0x00, 0x00, 0x00, 0x00, // disable_ipv6
            0xff, 0xff, 0xff, 0xff, // accept_dad
            0x00, 0x00, 0x00, 0x00, // force_tllao
            0x00, 0x00, 0x00, 0x00, // ndisc_notify
            0x10, 0x27, 0x00, 0x00, // mldv1_unsolicited_report_interval
            0xe8, 0x03, 0x00, 0x00, // mldv2_unsolicited_report_interval
            0x01, 0x00, 0x00, 0x00, // suppress_frag_ndisc
            0x00, 0x00, 0x00, 0x00, // accept_ra_from_local
            0x00, 0x00, 0x00, 0x00, // use_optimistic
            0x01, 0x00, 0x00, 0x00, // accept_ra_mtu
            0x00, 0x00, 0x00, 0x00, // stable_secret
            0x00, 0x00, 0x00, 0x00, // use_oif_addrs_only
            0x01, 0x00, 0x00, 0x00, // accept_ra_min_hop_limit
            0x00, 0x00, 0x00, 0x00, // ignore_routes_with_linkdown
            0x00, 0x00, 0x00, 0x00, // drop_unicast_in_l2_multicast
            0x00, 0x00, 0x00, 0x00, // drop_unsolicited_na
            0x00, 0x00, 0x00, 0x00, // keep_addr_on_down
            0x80, 0xee, 0x36, 0x00, // rtr_solicit_max_interval
            0x00, 0x00, 0x00, 0x00, // seg6_enabled
            0x00, 0x00, 0x00, 0x00, // seg6_require_hmac
            0x01, 0x00, 0x00, 0x00, // enhanced_dad
            0x00, 0x00, 0x00, 0x00, // addr_gen_mode
            0x00, 0x00, 0x00, 0x00, // disable_policy
            0x00, 0x00, 0x00, 0x00, // accept_ra_rt_info_min_plen
            0x00, 0x00, 0x00, 0x00, // ndisc_tclass

            // IFLA_INET6_STATS (L=292, T=3)
            0x24, 0x01, 0x03, 0x00,
            0x24, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 1  num
            0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 2  in_pkts
            0xa4, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 3  in_octets
            0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 4  in_delivers
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 5  out_forw_datagrams
            0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 6  out_pkts
            0xa4, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 7  out_octets
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 8  in_hdr_errors
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 9  in_too_big_errors
            0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 10 in_no_routes      (40 bytes)
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 11 in_addr_errors
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 12 in_unknown_protos
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 13 in_truncated_pkts
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 14 in_discards
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 15 out_discards
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 16 out_no_routes
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 17 reasm_timeout
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 18 reasm_reqds
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 19 reasm_oks
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 20 reasm_fails       (80 bytes)
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 21 frag_oks
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 22 frag_fails
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 23 frag_creates
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 24 in_mcast_pkts
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 25 out_mcast_pkts
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 26 in_bcast_pkts
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 27 out_bcast_pkts
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 28 in_mcast_octets
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 29 out_mcast_octets
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 30 in_bcast_octets   (120 bytes)
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 31 out_bcast_octets
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 32 in_csum_errors
            0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 33 in_no_ect_pkts
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 34 in_ect1_pkts
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 35 in_ect0_pkts
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 36 in_ce_pkts

            // IFLA_INET6_ICMP6STATS (L=52, T=6)
            0x34, 0x00, 0x06, 0x00,
            0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // num
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // in_msgs
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // in_errors
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // out_msgs
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // out_errors
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // csum_errors

            // IFLA_INET6_TOKEN (L=20, T=7)
            0x14, 0x00, 0x07, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,

            // IFLA_INET6_ADDR_GEN_MODE (L=5, T=8)
            0x05, 0x00, 0x08, 0x00,
            0x00, 0x00, 0x00, 0x00];

lazy_static! {
    static ref BUFFER: NlaBuffer<&'static [u8]> = NlaBuffer::new_checked(&BYTES[..]).unwrap();
}

fn get_nlas() -> impl Iterator<Item = Result<NlaBuffer<&'static [u8]>, DecodeError>> {
    NlasIterator::new(&*BUFFER.value())
}

fn get_byte_buffer(nla: &dyn Emitable) -> Vec<u8> {
    let mut buf = vec![0u8; nla.buffer_len()];
    nla.emit(&mut buf);
    buf
}

lazy_static! {
    static ref PARSED_AF_INET6: AfSpecInet = AfSpecInet::Inet6(vec![
        Inet6::Flags(2147483648),
        Inet6::CacheInfo(get_byte_buffer(&Inet6CacheInfo {
            max_reasm_len: 65535,
            tstamp: 175,
            reachable_time: 25730,
            retrans_time: 1000,
        })),
        Inet6::DevConf(get_byte_buffer(&Inet6DevConf {
            forwarding: 0,
            hoplimit: 64,
            mtu6: 65536,
            accept_ra: 1,
            accept_redirects: 1,
            autoconf: 1,
            dad_transmits: 1,
            rtr_solicits: -1,
            rtr_solicit_interval: 4000,
            rtr_solicit_delay: 1000,
            use_tempaddr: -1,
            temp_valid_lft: 604800,
            temp_prefered_lft: 86400,
            regen_max_retry: 3,
            max_desync_factor: 600,
            max_addresses: 16,
            force_mld_version: 0,
            accept_ra_defrtr: 1,
            accept_ra_pinfo: 1,
            accept_ra_rtr_pref: 1,
            rtr_probe_interval: 60000,
            accept_ra_rt_info_max_plen: 0,
            proxy_ndp: 0,
            optimistic_dad: 0,
            accept_source_route: 0,
            mc_forwarding: 0,
            disable_ipv6: 0,
            accept_dad: -1,
            force_tllao: 0,
            ndisc_notify: 0,
            mldv1_unsolicited_report_interval: 10000,
            mldv2_unsolicited_report_interval: 1000,
            suppress_frag_ndisc: 1,
            accept_ra_from_local: 0,
            use_optimistic: 0,
            accept_ra_mtu: 1,
            stable_secret: 0,
            use_oif_addrs_only: 0,
            accept_ra_min_hop_limit: 1,
            ignore_routes_with_linkdown: 0,
            drop_unicast_in_l2_multicast: 0,
            drop_unsolicited_na: 0,
            keep_addr_on_down: 0,
            rtr_solicit_max_interval: 3600000,
            seg6_enabled: 0,
            seg6_require_hmac: 0,
            enhanced_dad: 1,
            addr_gen_mode: 0,
            disable_policy: 0,
            accept_ra_rt_info_min_plen: 0,
            ndisc_tclass: 0,
        })),
        Inet6::Stats(get_byte_buffer(&Inet6Stats {
            num: 36,
            in_pkts: 6,
            in_octets: 420,
            in_delivers: 6,
            out_forw_datagrams: 0,
            out_pkts: 6,
            out_octets: 420,
            in_hdr_errors: 0,
            in_too_big_errors: 0,
            in_no_routes: 2,
            in_addr_errors: 0,
            in_unknown_protos: 0,
            in_truncated_pkts: 0,
            in_discards: 0,
            out_discards: 0,
            out_no_routes: 0,
            reasm_timeout: 0,
            reasm_reqds: 0,
            reasm_oks: 0,
            reasm_fails: 0,
            frag_oks: 0,
            frag_fails: 0,
            frag_creates: 0,
            in_mcast_pkts: 0,
            out_mcast_pkts: 0,
            in_bcast_pkts: 0,
            out_bcast_pkts: 0,
            in_mcast_octets: 0,
            out_mcast_octets: 0,
            in_bcast_octets: 0,
            out_bcast_octets: 0,
            in_csum_errors: 0,
            in_no_ect_pkts: 6,
            in_ect1_pkts: 0,
            in_ect0_pkts: 0,
            in_ce_pkts: 0,
        })),
        Inet6::IcmpStats(get_byte_buffer(&Icmp6Stats {
            num: 6,
            in_msgs: 0,
            in_errors: 0,
            out_msgs: 0,
            out_errors: 0,
            csum_errors: 0,
        })),
        Inet6::Token([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        Inet6::AddrGenMode(0),
    ]);
}

lazy_static! {
    static ref PARSED_AF_INET: AfSpecInet =
        AfSpecInet::Inet(vec![Inet::DevConf(get_byte_buffer(&InetDevConf {
            forwarding: 1,
            mc_forwarding: 0,
            proxy_arp: 0,
            accept_redirects: 1,
            secure_redirects: 1,
            send_redirects: 1,
            shared_media: 1,
            rp_filter: 0,
            accept_source_route: 1,
            bootp_relay: 0,
            log_martians: 0,
            tag: 0,
            arpfilter: 0,
            medium_id: 0,
            noxfrm: 1,
            nopolicy: 1,
            force_igmp_version: 0,
            arp_announce: 0,
            arp_ignore: 0,
            promote_secondaries: 0,
            arp_accept: 0,
            arp_notify: 0,
            accept_local: 0,
            src_vmark: 0,
            proxy_arp_pvlan: 0,
            route_localnet: 0,
            igmpv2_unsolicited_report_interval: 10000,
            igmpv3_unsolicited_report_interval: 1000,
            ignore_routes_with_linkdown: 0,
            drop_unicast_in_l2_multicast: 0,
            drop_gratuitous_arp: 0,
        }))]);
}

#[test]
fn af_spec_header() {
    assert_eq!(BUFFER.length(), 748);
    assert_eq!(BUFFER.kind(), IFLA_AF_SPEC as u16);
}

#[test]
fn parse_af_inet() {
    let mut nlas = get_nlas();
    // take the first nla
    let inet_buf = nlas.next().unwrap().unwrap();

    // buffer checks
    assert_eq!(inet_buf.length(), 132);
    assert_eq!(inet_buf.kind(), AF_INET);
    assert_eq!(inet_buf.value().len(), 128);

    // parsing check
    let parsed = AfSpecInet::parse(&inet_buf).unwrap();
    assert_eq!(parsed, *PARSED_AF_INET);
}

#[test]
fn emit_af_inet() {
    let mut bytes = vec![0xff; 132];

    // Note: the value is a Vec of nlas, so the padding is automatically added for each nla.
    assert_eq!(PARSED_AF_INET.value_len(), 128);
    assert_eq!(PARSED_AF_INET.buffer_len(), 128 + 4);

    PARSED_AF_INET.emit(&mut bytes[..]);

    let buf = NlaBuffer::new_checked(&bytes[..]).unwrap();

    let mut nlas = get_nlas();
    let expected_buf = nlas.next().unwrap().unwrap();

    assert_eq!(expected_buf.kind(), buf.kind());
    assert_eq!(expected_buf.length(), buf.length());
    assert_eq!(expected_buf.value(), buf.value());
}

#[test]
fn emit_af_inet6() {
    let mut bytes = vec![0xff; 612];

    // Note: the value is a Vec of nlas, so the padding is automatically added for each nla.
    assert_eq!(PARSED_AF_INET6.value_len(), 608);
    assert_eq!(PARSED_AF_INET6.buffer_len(), 608 + 4);
    PARSED_AF_INET6.emit(&mut bytes[..]);

    let buf = NlaBuffer::new_checked(&bytes[..]).unwrap();

    let mut nlas = get_nlas();
    let _ = nlas.next();
    let expected_buf = nlas.next().unwrap().unwrap();

    assert_eq!(expected_buf.kind(), buf.kind());
    assert_eq!(expected_buf.length(), buf.length());
    assert_eq!(expected_buf.value(), buf.value());
}

#[test]
fn parse_af_inet6() {
    let mut nlas = get_nlas();
    // take the first nla
    let _ = nlas.next().unwrap();
    let inet6_buf = nlas.next().unwrap().unwrap();

    assert_eq!(inet6_buf.length(), 612);
    assert_eq!(inet6_buf.kind(), AF_INET6);
    assert_eq!(inet6_buf.value().len(), 608);
    let parsed = AfSpecInet::parse(&inet6_buf).unwrap();

    assert_eq!(parsed, *PARSED_AF_INET6);

    // Normally this is the end of the nla iterator
    assert!(nlas.next().is_none());
}
