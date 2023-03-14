// SPDX-License-Identifier: MIT

#![cfg(test)]

use crate::{
    constants::*,
    nlas::Nla,
    parsers::parse_u32,
    tc::{self, constants::*, mirred, u32, ActNla, ActOpt, Action, Stats2, TcOpt},
    traits::{Emitable, Parseable},
    TcHeader,
    TcMessage,
    TcMessageBuffer,
};

#[rustfmt::skip]
    static FILTER_U32_ACTION_PACKET: [u8; 260] = [
        0, 0, 0, 0, // family, pad1, pad2
        3, 0, 0, 0, // Interface index
        0, 8, 0, 128, // handle: 0x800_00_800 => htid | hash | nodeid
        255, 255, 255, 255, // parent: TC_H_ROOT
        0, 3, 0, 192, // info: 0xc000_0300 => pref | protocol
        // nlas
        8, 0, // length
        1, 0, // type: TCA_KIND
        117, 51, 50, 0, // u32\0

        8, 0,
        11, 0, // type: TCA_CHAIN
        0, 0, 0, 0,

        224, 0,
        2, 0, // type: TCA_OPTIONS

            36, 0,
            5, 0, // type: TCA_U32_SEL
                1, 0, 1, 0,
                0, 0,
                0, 0,
                0, 0,
                0, 0,
                0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,

            8, 0,
            2, 0, // TCA_U32_HASH
            0, 0, 0, 128,

            8, 0,
            11, 0, // TCA_U32_FLAGS
            8, 0, 0, 0,

            140, 0,
            7, 0, // TCA_U32_ACT
                136, 0,
                1, 0, // TCA_ACT_TAB
                    11, 0,
                    1, 0, // TCA_ACT_KIND
                        109, 105, 114, 114, 101, 100, 0, 0, // "mirred\0"
                    48, 0,
                    4, 0, // TCA_ACT_STATS
                        20, 0,
                        1, 0, // TCA_STATS_BASIC
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        24, 0,
                        3, 0, // TCA_STATS_QUEUE
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    72, 0,
                    2, 0, // TCA_ACT_OPTIONS
                        32, 0,
                        2, 0, // TCA_MIRRED_PARMS
                            1, 0, 0, 0,
                            0, 0, 0, 0,
                            4, 0, 0, 0,
                            1, 0, 0, 0,
                            1, 0, 0, 0,
                            1, 0, 0, 0,
                            3, 0, 0, 0,
                        36, 0,
                        1, 0, // TCA_MIRRED_TM
                            189, 117, 195, 9, 0, 0, 0, 0,
                            189, 117, 195, 9, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0,
                            109, 226, 238, 72, 0, 0, 0, 0,
            28, 0,
            9, 0, // TCA_U32_PCNT
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];

#[test]
#[allow(clippy::unusual_byte_groupings)]
fn tc_filter_u32_read() {
    let packet = TcMessageBuffer::new(&FILTER_U32_ACTION_PACKET);
    assert_eq!(packet.family(), 0);
    assert_eq!(packet.index(), 3);
    assert_eq!(packet.handle(), 0x800_00_800);
    assert_eq!(packet.parent(), 0xffffffff);
    assert_eq!(packet.info(), 0xc000_0300);

    assert_eq!(packet.nlas().count(), 3);

    let mut nlas = packet.nlas();

    let nla = nlas.next().unwrap().unwrap();
    nla.check_buffer_length().unwrap();
    assert_eq!(nla.length(), 8);
    assert_eq!(nla.kind(), TCA_KIND);
    assert_eq!(nla.value(), "u32\0".as_bytes());

    let nla = nlas.next().unwrap().unwrap();
    nla.check_buffer_length().unwrap();
    assert_eq!(nla.length(), 8);
    assert_eq!(nla.kind(), TCA_CHAIN);
    assert_eq!(parse_u32(nla.value()).unwrap(), 0);

    let nla = nlas.next().unwrap().unwrap();
    nla.check_buffer_length().unwrap();
    assert_eq!(nla.length(), 224);
    assert_eq!(nla.kind(), TCA_OPTIONS);
}

#[test]
fn tc_filter_u32_parse() {
    let packet = TcMessageBuffer::new_checked(&FILTER_U32_ACTION_PACKET).unwrap();

    // TcMessage
    let msg = TcMessage::parse(&packet).unwrap();
    assert_eq!(msg.header.index, 3);
    assert_eq!(msg.header.info, 0xc000_0300);
    assert_eq!(msg.nlas.len(), 3);

    // Nlas
    let mut iter = msg.nlas.iter();

    // TCA_KIND
    assert_eq!(
        iter.next().unwrap(),
        &tc::Nla::Kind(String::from(u32::KIND))
    );

    // TCA_CHAIN
    assert!(matches!(iter.next().unwrap(), &tc::Nla::Chain(_)));

    // TCA_OPTIONS
    let nla = iter.next().unwrap();
    let filter = if let tc::Nla::Options(f) = nla {
        assert_eq!(f.len(), 5);
        f
    } else {
        panic!("expect options nla");
    };

    // u32 option
    let mut fi = filter.iter();
    let fa = fi.next().unwrap();
    let ua = if let TcOpt::U32(u) = fa {
        u
    } else {
        panic!("expect u32 nla");
    };

    // TCA_U32_SEL
    let sel = if let u32::Nla::Sel(s) = ua {
        s
    } else {
        panic!("expect sel nla");
    };
    assert_eq!(sel.flags, TC_U32_TERMINAL);
    assert_eq!(sel.nkeys, 1);
    assert_eq!(sel.keys.len(), 1);
    assert_eq!(sel.keys[0], u32::Key::default());

    // TCA_U32_HASH
    assert_eq!(fi.next().unwrap(), &TcOpt::U32(u32::Nla::Hash(0x80000000)));
    // TCA_U32_FLAGS
    assert_eq!(fi.next().unwrap(), &TcOpt::U32(u32::Nla::Flags(0x00000008)));

    // TCA_U32_ACT
    let fa = fi.next().unwrap();
    let acts = if let TcOpt::U32(u) = fa {
        if let u32::Nla::Act(a) = u {
            a
        } else {
            panic!("expect u32 action");
        }
    } else {
        panic!("expect u32 nla");
    };

    // TCA_ACT_TAB
    let mut act_iter = acts.iter();

    let act = act_iter.next().unwrap();
    assert_eq!(act.kind(), 1); // TCA_ACT_TAB
    assert_eq!(act.buffer_len(), 136); // TCA_ACT_TAB
    assert_eq!(act.tab, 1);

    let mut act_nlas_iter = act.nlas.iter();

    // TCA_ACT_KIND
    assert_eq!(
        act_nlas_iter.next().unwrap(),
        &ActNla::Kind("mirred".to_string())
    );

    // TCA_ACT_STATS
    assert!(matches!(act_nlas_iter.next().unwrap(), ActNla::Stats(_)));

    // TCA_ACT_OPTIONS
    let act_nla = act_nlas_iter.next().unwrap();
    let act_opts = if let ActNla::Options(opts) = act_nla {
        opts
    } else {
        panic!("expect action options");
    };

    let mut act_opts_iter = act_opts.iter();

    // TCA_MIRRED_PARMS
    let act_opt = act_opts_iter.next().unwrap();
    if let ActOpt::Mirred(mirred::Nla::Parms(p)) = act_opt {
        assert_eq!(p.index, 1);
        assert_eq!(p.capab, 0);
        assert_eq!(p.action, 4);
        assert_eq!(p.refcnt, 1);
        assert_eq!(p.bindcnt, 1);
        assert_eq!(p.eaction, 1);
        assert_eq!(p.ifindex, 3);
    } else {
        panic!("expect action mirred");
    }
    // TCA_MIRRED_TM
    let act_opt = act_opts_iter.next().unwrap();
    assert_eq!(act_opt.kind(), TCA_MIRRED_TM);
    assert_eq!(act_opt.buffer_len(), 36);

    // TCA_U32_PCNT
    let fa = fi.next().unwrap();
    assert_eq!(fa.kind(), TCA_U32_PCNT);
    assert_eq!(fa.buffer_len(), 28);
}

#[test]
#[allow(clippy::unusual_byte_groupings)]
fn tc_filter_u32_emit() {
    // TcHeader
    let header = TcHeader {
        index: 3,
        handle: 0x800_00_800,
        parent: 0xffffffff,
        info: 0xc000_0300,
        ..Default::default()
    };

    // Tc Nlas
    let nlas = vec![
        tc::Nla::Kind(u32::KIND.to_string()),
        tc::Nla::Chain(vec![0, 0, 0, 0]),
        tc::Nla::Options(vec![
            TcOpt::U32(u32::Nla::Sel(u32::Sel {
                flags: TC_U32_TERMINAL,
                offshift: 0,
                nkeys: 1,
                offmask: 0,
                off: 0,
                offoff: 0,
                hoff: 0,
                hmask: 0,
                keys: vec![u32::Key::default()],
            })),
            TcOpt::U32(u32::Nla::Hash(0x80000000)),
            TcOpt::U32(u32::Nla::Flags(0x00000008)),
            TcOpt::U32(u32::Nla::Act(vec![Action {
                tab: TCA_ACT_TAB,
                nlas: vec![
                    ActNla::Kind(mirred::KIND.to_string()),
                    ActNla::Stats(vec![
                        Stats2::StatsBasic(vec![0u8; 16]),
                        Stats2::StatsQueue(vec![0u8; 20]),
                    ]),
                    ActNla::Options(vec![
                        ActOpt::Mirred(mirred::Nla::Parms(mirred::TcMirred {
                            index: 1,
                            capab: 0,
                            action: 4,
                            refcnt: 1,
                            bindcnt: 1,
                            eaction: 1,
                            ifindex: 3,
                        })),
                        ActOpt::Mirred(mirred::Nla::Tm(
                            FILTER_U32_ACTION_PACKET[200..232].to_vec(),
                        )),
                    ]),
                ],
            }])),
            TcOpt::U32(u32::Nla::Pcnt(vec![0u8; 24])),
        ]),
    ];

    let msg = TcMessage::from_parts(header, nlas);
    let mut buf = vec![0; 260];
    assert_eq!(msg.buffer_len(), 260);
    msg.emit(&mut buf[..]);
    assert_eq!(&buf, &FILTER_U32_ACTION_PACKET);
}
