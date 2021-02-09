//! Network device information from `/proc/net/dev`.

use std::fs::File;
use std::io::{Read, Result};

use nom::{space, line_ending};
use parsers::{
    map_result,
    parse_u64,
};

const NET_DEV_FILE: &'static str = "/proc/net/dev";

/// Network device status information.
///
/// See `man 5 proc` and `Linux/net/core/net-procfs.c`
pub struct DeviceStatus {
    /// Name of the interface representing this device.
    pub interface:           String,

    /// Number of received bytes.
    pub receive_bytes:       u64,
    /// Number of received packets.
    pub receive_packets:     u64,
    /// Number of bad packets received.
    pub receive_errs:        u64,
    /// Number of incoming packets dropped.
    pub receive_drop:        u64,
    /// Number of incoming packets dropped due to fifo overrun.
    pub receive_fifo:        u64,
    /// Number of incoming packets dropped due to frame alignment errors.
    pub receive_frame:       u64,
    /// Number of CSLIP packets received.
    pub receive_compressed:  u64,
    /// Number of multicast packets received.
    pub receive_multicast:   u64,

    /// Number of transmitted bytes.
    pub transmit_bytes:      u64,
    /// Number of transmitted packets.
    pub transmit_packets:    u64,
    /// Number of occurred transmission problems.
    pub transmit_errs:       u64,
    /// Number of outgoing packets dropped.
    pub transmit_drop:       u64,
    /// Number of outgoing packets dropped due to fifo overrun.
    pub transmit_fifo:       u64,
    /// Number of occurred packet collisions.
    pub transmit_colls:      u64,
    /// Number of occurred carrier errors.
    pub transmit_carrier:    u64,
    /// Number of CSLIP packets transmitted.
    pub transmit_compressed: u64,
}

named!(interface_stats<DeviceStatus>,
    do_parse!(
        opt!(space) >>
        interface: take_until_and_consume!(":") >>
        space >>
        receive_bytes:       terminated!(parse_u64, space) >>
        receive_packets:     terminated!(parse_u64, space) >>
        receive_errs:        terminated!(parse_u64, space) >>
        receive_drop:        terminated!(parse_u64, space) >>
        receive_fifo:        terminated!(parse_u64, space) >>
        receive_frame:       terminated!(parse_u64, space) >>
        receive_compressed:  terminated!(parse_u64, space) >>
        receive_multicast:   terminated!(parse_u64, space) >>
        transmit_bytes:      terminated!(parse_u64, space) >>
        transmit_packets:    terminated!(parse_u64, space) >>
        transmit_errs:       terminated!(parse_u64, space) >>
        transmit_drop:       terminated!(parse_u64, space) >>
        transmit_fifo:       terminated!(parse_u64, space) >>
        transmit_colls:      terminated!(parse_u64, space) >>
        transmit_carrier:    terminated!(parse_u64, space) >>
        transmit_compressed: parse_u64 >>
        (DeviceStatus {
            interface:           String::from_utf8_lossy(interface).to_string(),
            receive_bytes:       receive_bytes,
            receive_packets:     receive_packets,
            receive_errs:        receive_errs,
            receive_drop:        receive_drop,
            receive_fifo:        receive_fifo,
            receive_frame:       receive_frame,
            receive_compressed:  receive_compressed,
            receive_multicast:   receive_multicast,
            transmit_bytes:      transmit_bytes,
            transmit_packets:    transmit_packets,
            transmit_errs:       transmit_errs,
            transmit_drop:       transmit_drop,
            transmit_fifo:       transmit_fifo,
            transmit_colls:      transmit_colls,
            transmit_carrier:    transmit_carrier,
            transmit_compressed: transmit_compressed,
        })));

named!(interface_list< Vec<DeviceStatus> >,
    do_parse!(
        interfaces: separated_list!(line_ending, interface_stats) >>
        line_ending >>
        (interfaces)));

named!(empty_list< Vec<DeviceStatus> >,
    value!(Vec::new(), eof!()));

named!(parse_dev< Vec<DeviceStatus> >,
    do_parse!(
        count!(take_until_and_consume!("\n"), 2) >>
        interfaces: alt_complete!(interface_list | empty_list) >>
        (interfaces)));

/// Returns list of all network devices and information about their state.
pub fn dev() -> Result<Vec<DeviceStatus>> {
    let mut file = File::open(NET_DEV_FILE)?;

    let mut buffer = vec![];
    file.read_to_end(&mut buffer)?;

    map_result(parse_dev(buffer.as_slice()))
}

#[cfg(test)]
mod test {
    use super::{dev, parse_dev};
    use parsers::map_result;

    #[test]
    fn two_interfaces() {
        let file = br#"Inter-|   Receive                                                |  Transmit
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed
    lo:  206950    2701    0    0    0     0          0         0   206950    2701    0    0    0     0       0          0
wlp58s0: 631994599  596110    0    1    0     0          0         0 47170335  384943    0    0    0     0       0          0
"#;
        let interfaces = map_result(parse_dev(file)).unwrap();

        assert!(interfaces.len() == 2);

        assert!(interfaces[0].interface          == "lo");
        assert!(interfaces[0].receive_bytes      == 206950);
        assert!(interfaces[0].receive_packets    == 2701);
        assert!(interfaces[0].receive_errs       == 0);
        assert!(interfaces[0].receive_drop       == 0);
        assert!(interfaces[0].receive_fifo       == 0);
        assert!(interfaces[0].receive_frame      == 0);
        assert!(interfaces[0].receive_compressed == 0);
        assert!(interfaces[0].receive_multicast  == 0);
        assert!(interfaces[0].transmit_bytes     == 206950);
        assert!(interfaces[0].transmit_packets   == 2701);
        assert!(interfaces[0].transmit_errs      == 0);
        assert!(interfaces[0].transmit_drop      == 0);
        assert!(interfaces[0].transmit_fifo      == 0);
        assert!(interfaces[0].transmit_colls     == 0);
        assert!(interfaces[0].transmit_carrier   == 0);

        assert!(interfaces[1].interface           == "wlp58s0");
        assert!(interfaces[1].receive_bytes       == 631994599);
        assert!(interfaces[1].receive_packets     == 596110);
        assert!(interfaces[1].receive_errs        == 0);
        assert!(interfaces[1].receive_drop        == 1);
        assert!(interfaces[1].receive_fifo        == 0);
        assert!(interfaces[1].receive_frame       == 0);
        assert!(interfaces[1].receive_multicast   == 0);
        assert!(interfaces[1].transmit_bytes      == 47170335);
        assert!(interfaces[1].transmit_packets    == 384943);
        assert!(interfaces[1].transmit_errs       == 0);
        assert!(interfaces[1].transmit_drop       == 0);
        assert!(interfaces[1].transmit_fifo       == 0);
        assert!(interfaces[1].transmit_colls      == 0);
        assert!(interfaces[1].transmit_carrier    == 0);
        assert!(interfaces[1].transmit_compressed == 0);
    }

    #[test]
    fn no_interfaces() {
        let file = br#"Inter-|   Receive                                                |  Transmit
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed
"#;
        let interfaces = map_result(parse_dev(file)).unwrap();
        assert!(interfaces.len() == 0);
    }

    #[test]
    fn parse_native() {
        dev().unwrap();
    }
}
