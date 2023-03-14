// SPDX-License-Identifier: MIT

use crate::{
    traits::{Emitable, Parseable},
    DecodeError,
};

pub const INET6_STATS_LEN: usize = 288;
buffer!(Inet6StatsBuffer(INET6_STATS_LEN) {
    num: (i64, 0..8),
    in_pkts: (i64, 8..16),
    in_octets: (i64, 16..24),
    in_delivers: (i64, 24..32),
    out_forw_datagrams: (i64, 32..40),
    out_pkts: (i64, 40..48),
    out_octets: (i64, 48..56),
    in_hdr_errors: (i64, 56..64),
    in_too_big_errors: (i64, 64..72),
    in_no_routes: (i64, 72..80),
    in_addr_errors: (i64, 80..88),
    in_unknown_protos: (i64, 88..96),
    in_truncated_pkts: (i64, 96..104),
    in_discards: (i64, 104..112),
    out_discards: (i64, 112..120),
    out_no_routes: (i64, 120..128),
    reasm_timeout: (i64, 128..136),
    reasm_reqds: (i64, 136..144),
    reasm_oks: (i64, 144..152),
    reasm_fails: (i64, 152..160),
    frag_oks: (i64, 160..168),
    frag_fails: (i64, 168..176),
    frag_creates: (i64, 176..184),
    in_mcast_pkts: (i64, 184..192),
    out_mcast_pkts: (i64, 192..200),
    in_bcast_pkts: (i64, 200..208),
    out_bcast_pkts: (i64, 208..216),
    in_mcast_octets: (i64, 216..224),
    out_mcast_octets: (i64, 224..232),
    in_bcast_octets: (i64, 232..240),
    out_bcast_octets: (i64, 240..248),
    in_csum_errors: (i64, 248..256),
    in_no_ect_pkts: (i64, 256..264),
    in_ect1_pkts: (i64, 264..272),
    in_ect0_pkts: (i64, 272..280),
    in_ce_pkts: (i64, 280..288),
});

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct Inet6Stats {
    pub num: i64,
    pub in_pkts: i64,
    pub in_octets: i64,
    pub in_delivers: i64,
    pub out_forw_datagrams: i64,
    pub out_pkts: i64,
    pub out_octets: i64,
    pub in_hdr_errors: i64,
    pub in_too_big_errors: i64,
    pub in_no_routes: i64,
    pub in_addr_errors: i64,
    pub in_unknown_protos: i64,
    pub in_truncated_pkts: i64,
    pub in_discards: i64,
    pub out_discards: i64,
    pub out_no_routes: i64,
    pub reasm_timeout: i64,
    pub reasm_reqds: i64,
    pub reasm_oks: i64,
    pub reasm_fails: i64,
    pub frag_oks: i64,
    pub frag_fails: i64,
    pub frag_creates: i64,
    pub in_mcast_pkts: i64,
    pub out_mcast_pkts: i64,
    pub in_bcast_pkts: i64,
    pub out_bcast_pkts: i64,
    pub in_mcast_octets: i64,
    pub out_mcast_octets: i64,
    pub in_bcast_octets: i64,
    pub out_bcast_octets: i64,
    pub in_csum_errors: i64,
    pub in_no_ect_pkts: i64,
    pub in_ect1_pkts: i64,
    pub in_ect0_pkts: i64,
    pub in_ce_pkts: i64,
}

impl<T: AsRef<[u8]>> Parseable<Inet6StatsBuffer<T>> for Inet6Stats {
    fn parse(buf: &Inet6StatsBuffer<T>) -> Result<Self, DecodeError> {
        Ok(Self {
            num: buf.num(),
            in_pkts: buf.in_pkts(),
            in_octets: buf.in_octets(),
            in_delivers: buf.in_delivers(),
            out_forw_datagrams: buf.out_forw_datagrams(),
            out_pkts: buf.out_pkts(),
            out_octets: buf.out_octets(),
            in_hdr_errors: buf.in_hdr_errors(),
            in_too_big_errors: buf.in_too_big_errors(),
            in_no_routes: buf.in_no_routes(),
            in_addr_errors: buf.in_addr_errors(),
            in_unknown_protos: buf.in_unknown_protos(),
            in_truncated_pkts: buf.in_truncated_pkts(),
            in_discards: buf.in_discards(),
            out_discards: buf.out_discards(),
            out_no_routes: buf.out_no_routes(),
            reasm_timeout: buf.reasm_timeout(),
            reasm_reqds: buf.reasm_reqds(),
            reasm_oks: buf.reasm_oks(),
            reasm_fails: buf.reasm_fails(),
            frag_oks: buf.frag_oks(),
            frag_fails: buf.frag_fails(),
            frag_creates: buf.frag_creates(),
            in_mcast_pkts: buf.in_mcast_pkts(),
            out_mcast_pkts: buf.out_mcast_pkts(),
            in_bcast_pkts: buf.in_bcast_pkts(),
            out_bcast_pkts: buf.out_bcast_pkts(),
            in_mcast_octets: buf.in_mcast_octets(),
            out_mcast_octets: buf.out_mcast_octets(),
            in_bcast_octets: buf.in_bcast_octets(),
            out_bcast_octets: buf.out_bcast_octets(),
            in_csum_errors: buf.in_csum_errors(),
            in_no_ect_pkts: buf.in_no_ect_pkts(),
            in_ect1_pkts: buf.in_ect1_pkts(),
            in_ect0_pkts: buf.in_ect0_pkts(),
            in_ce_pkts: buf.in_ce_pkts(),
        })
    }
}

impl Emitable for Inet6Stats {
    fn buffer_len(&self) -> usize {
        INET6_STATS_LEN
    }

    fn emit(&self, buffer: &mut [u8]) {
        let mut buffer = Inet6StatsBuffer::new(buffer);
        buffer.set_num(self.num);
        buffer.set_in_pkts(self.in_pkts);
        buffer.set_in_octets(self.in_octets);
        buffer.set_in_delivers(self.in_delivers);
        buffer.set_out_forw_datagrams(self.out_forw_datagrams);
        buffer.set_out_pkts(self.out_pkts);
        buffer.set_out_octets(self.out_octets);
        buffer.set_in_hdr_errors(self.in_hdr_errors);
        buffer.set_in_too_big_errors(self.in_too_big_errors);
        buffer.set_in_no_routes(self.in_no_routes);
        buffer.set_in_addr_errors(self.in_addr_errors);
        buffer.set_in_unknown_protos(self.in_unknown_protos);
        buffer.set_in_truncated_pkts(self.in_truncated_pkts);
        buffer.set_in_discards(self.in_discards);
        buffer.set_out_discards(self.out_discards);
        buffer.set_out_no_routes(self.out_no_routes);
        buffer.set_reasm_timeout(self.reasm_timeout);
        buffer.set_reasm_reqds(self.reasm_reqds);
        buffer.set_reasm_oks(self.reasm_oks);
        buffer.set_reasm_fails(self.reasm_fails);
        buffer.set_frag_oks(self.frag_oks);
        buffer.set_frag_fails(self.frag_fails);
        buffer.set_frag_creates(self.frag_creates);
        buffer.set_in_mcast_pkts(self.in_mcast_pkts);
        buffer.set_out_mcast_pkts(self.out_mcast_pkts);
        buffer.set_in_bcast_pkts(self.in_bcast_pkts);
        buffer.set_out_bcast_pkts(self.out_bcast_pkts);
        buffer.set_in_mcast_octets(self.in_mcast_octets);
        buffer.set_out_mcast_octets(self.out_mcast_octets);
        buffer.set_in_bcast_octets(self.in_bcast_octets);
        buffer.set_out_bcast_octets(self.out_bcast_octets);
        buffer.set_in_csum_errors(self.in_csum_errors);
        buffer.set_in_no_ect_pkts(self.in_no_ect_pkts);
        buffer.set_in_ect1_pkts(self.in_ect1_pkts);
        buffer.set_in_ect0_pkts(self.in_ect0_pkts);
        buffer.set_in_ce_pkts(self.in_ce_pkts);
    }
}
