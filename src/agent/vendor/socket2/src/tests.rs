use std::io::Write;
use std::str;

use crate::{Domain, Protocol, Type};

#[test]
fn domain_fmt_debug() {
    let tests = &[
        (Domain::ipv4(), "AF_INET"),
        (Domain::ipv6(), "AF_INET6"),
        #[cfg(unix)]
        (Domain::unix(), "AF_UNIX"),
        (0.into(), "AF_UNSPEC"),
        (500.into(), "500"),
    ];

    let mut buf = Vec::new();
    for (input, want) in tests {
        buf.clear();
        write!(buf, "{:?}", input).unwrap();
        let got = str::from_utf8(&buf).unwrap();
        assert_eq!(got, *want);
    }
}

#[test]
fn type_fmt_debug() {
    let tests = &[
        (Type::stream(), "SOCK_STREAM"),
        (Type::dgram(), "SOCK_DGRAM"),
        (Type::seqpacket(), "SOCK_SEQPACKET"),
        (Type::raw(), "SOCK_RAW"),
        (500.into(), "500"),
    ];

    let mut buf = Vec::new();
    for (input, want) in tests {
        buf.clear();
        write!(buf, "{:?}", input).unwrap();
        let got = str::from_utf8(&buf).unwrap();
        assert_eq!(got, *want);
    }
}

#[test]
fn protocol_fmt_debug() {
    let tests = &[
        (Protocol::icmpv4(), "IPPROTO_ICMP"),
        (Protocol::icmpv6(), "IPPROTO_ICMPV6"),
        (Protocol::tcp(), "IPPROTO_TCP"),
        (Protocol::udp(), "IPPROTO_UDP"),
        (500.into(), "500"),
    ];

    let mut buf = Vec::new();
    for (input, want) in tests {
        buf.clear();
        write!(buf, "{:?}", input).unwrap();
        let got = str::from_utf8(&buf).unwrap();
        assert_eq!(got, *want);
    }
}
