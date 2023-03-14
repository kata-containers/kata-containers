// Don't throw clippy warnings for manual string stripping.
// The suggested fix with `strip_prefix` removes support for Rust 1.33 and 1.38
#![allow(clippy::manual_strip)]

//! Information about the networking layer.
//!
//! This module corresponds to the `/proc/net` directory and contains various information about the
//! networking layer.
//!
//! # Example
//!
//! Here's an example that will print out all of the open and listening TCP sockets, and their
//! corresponding processes, if know.  This mimics the "netstat" utility, but for TCP only.  You
//! can run this example yourself with:
//!
//! > cargo run --example=netstat
//!
//! ```rust
//! # use procfs::process::{Process, FDTarget};
//! # use std::collections::HashMap;
//! let all_procs = procfs::process::all_processes().unwrap();
//!
//! // build up a map between socket inodes and processes:
//! let mut map: HashMap<u64, &Process> = HashMap::new();
//! for process in &all_procs {
//!     if let Ok(fds) = process.fd() {
//!         for fd in fds {
//!             if let FDTarget::Socket(inode) = fd.target {
//!                 map.insert(inode, process);
//!             }
//!         }
//!     }
//! }
//!
//! // get the tcp table
//! let tcp = procfs::net::tcp().unwrap();
//! let tcp6 = procfs::net::tcp6().unwrap();
//! println!("{:<26} {:<26} {:<15} {:<8} {}", "Local address", "Remote address", "State", "Inode", "PID/Program name");
//! for entry in tcp.into_iter().chain(tcp6) {
//!     // find the process (if any) that has an open FD to this entry's inode
//!     let local_address = format!("{}", entry.local_address);
//!     let remote_addr = format!("{}", entry.remote_address);
//!     let state = format!("{:?}", entry.state);
//!     if let Some(process) = map.get(&entry.inode) {
//!         println!("{:<26} {:<26} {:<15} {:<12} {}/{}", local_address, remote_addr, state, entry.inode, process.stat.pid, process.stat.comm);
//!     } else {
//!         // We might not always be able to find the process associated with this socket
//!         println!("{:<26} {:<26} {:<15} {:<12} -", local_address, remote_addr, state, entry.inode);
//!     }
//! }
use crate::from_iter;
use crate::ProcResult;
use std::collections::HashMap;

use crate::FileWrapper;
use bitflags::bitflags;
use byteorder::{ByteOrder, NativeEndian, NetworkEndian};
use std::io::{BufRead, BufReader, Read};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::{path::PathBuf, str::FromStr};

#[derive(Debug, Clone, PartialEq)]
pub enum TcpState {
    Established = 1,
    SynSent,
    SynRecv,
    FinWait1,
    FinWait2,
    TimeWait,
    Close,
    CloseWait,
    LastAck,
    Listen,
    Closing,
    NewSynRecv,
}

impl TcpState {
    pub fn from_u8(num: u8) -> Option<TcpState> {
        match num {
            0x01 => Some(TcpState::Established),
            0x02 => Some(TcpState::SynSent),
            0x03 => Some(TcpState::SynRecv),
            0x04 => Some(TcpState::FinWait1),
            0x05 => Some(TcpState::FinWait2),
            0x06 => Some(TcpState::TimeWait),
            0x07 => Some(TcpState::Close),
            0x08 => Some(TcpState::CloseWait),
            0x09 => Some(TcpState::LastAck),
            0x0A => Some(TcpState::Listen),
            0x0B => Some(TcpState::Closing),
            0x0C => Some(TcpState::NewSynRecv),
            _ => None,
        }
    }

    pub fn to_u8(&self) -> u8 {
        match self {
            TcpState::Established => 0x01,
            TcpState::SynSent => 0x02,
            TcpState::SynRecv => 0x03,
            TcpState::FinWait1 => 0x04,
            TcpState::FinWait2 => 0x05,
            TcpState::TimeWait => 0x06,
            TcpState::Close => 0x07,
            TcpState::CloseWait => 0x08,
            TcpState::LastAck => 0x09,
            TcpState::Listen => 0x0A,
            TcpState::Closing => 0x0B,
            TcpState::NewSynRecv => 0x0C,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum UdpState {
    Established = 1,
    Close = 7,
}

impl UdpState {
    pub fn from_u8(num: u8) -> Option<UdpState> {
        match num {
            0x01 => Some(UdpState::Established),
            0x07 => Some(UdpState::Close),
            _ => None,
        }
    }

    pub fn to_u8(&self) -> u8 {
        match self {
            UdpState::Established => 0x01,
            UdpState::Close => 0x07,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnixState {
    UNCONNECTED = 1,
    CONNECTING = 2,
    CONNECTED = 3,
    DISCONNECTING = 4,
}

impl UnixState {
    pub fn from_u8(num: u8) -> Option<UnixState> {
        match num {
            0x01 => Some(UnixState::UNCONNECTED),
            0x02 => Some(UnixState::CONNECTING),
            0x03 => Some(UnixState::CONNECTED),
            0x04 => Some(UnixState::DISCONNECTING),
            _ => None,
        }
    }

    pub fn to_u8(&self) -> u8 {
        match self {
            UnixState::UNCONNECTED => 0x01,
            UnixState::CONNECTING => 0x02,
            UnixState::CONNECTED => 0x03,
            UnixState::DISCONNECTING => 0x04,
        }
    }
}

/// An entry in the TCP socket table
#[derive(Debug, Clone)]
pub struct TcpNetEntry {
    pub local_address: SocketAddr,
    pub remote_address: SocketAddr,
    pub state: TcpState,
    pub rx_queue: u32,
    pub tx_queue: u32,
    pub inode: u64,
}

/// An entry in the UDP socket table
#[derive(Debug, Clone)]
pub struct UdpNetEntry {
    pub local_address: SocketAddr,
    pub remote_address: SocketAddr,
    pub state: UdpState,
    pub rx_queue: u32,
    pub tx_queue: u32,
    pub inode: u64,
}

/// An entry in the Unix socket table
#[derive(Debug, Clone)]
pub struct UnixNetEntry {
    /// The number of users of the socket
    pub ref_count: u32,
    /// The socket type.
    ///
    /// Possible values are `SOCK_STREAM`, `SOCK_DGRAM`, or `SOCK_SEQPACKET`.  These constants can
    /// be found in the libc crate.
    pub socket_type: u16,
    /// The state of the socket
    pub state: UnixState,
    /// The inode number of the socket
    pub inode: u64,
    /// The bound pathname (if any) of the socket.
    ///
    /// Sockets in the abstract namespace are included, and are shown with a path that commences
    /// with the '@' character.
    pub path: Option<PathBuf>,
}

/// Parses an address in the form 00010203:1234
///
/// Also supports IPv6
fn parse_addressport_str(s: &str) -> ProcResult<SocketAddr> {
    let mut las = s.split(':');
    let ip_part = expect!(las.next(), "ip_part");
    let port = expect!(las.next(), "port");
    let port = from_str!(u16, port, 16);

    if ip_part.len() == 8 {
        let bytes = expect!(hex::decode(&ip_part));
        let ip_u32 = NetworkEndian::read_u32(&bytes);

        let ip = Ipv4Addr::new(
            (ip_u32 & 0xff) as u8,
            ((ip_u32 & 0xff << 8) >> 8) as u8,
            ((ip_u32 & 0xff << 16) >> 16) as u8,
            ((ip_u32 & 0xff << 24) >> 24) as u8,
        );

        Ok(SocketAddr::V4(SocketAddrV4::new(ip, port)))
    } else if ip_part.len() == 32 {
        let bytes = expect!(hex::decode(&ip_part));

        let ip_a = NativeEndian::read_u32(&bytes[0..]);
        let ip_b = NativeEndian::read_u32(&bytes[4..]);
        let ip_c = NativeEndian::read_u32(&bytes[8..]);
        let ip_d = NativeEndian::read_u32(&bytes[12..]);

        let ip = Ipv6Addr::new(
            ((ip_a >> 16) & 0xffff) as u16,
            (ip_a & 0xffff) as u16,
            ((ip_b >> 16) & 0xffff) as u16,
            (ip_b & 0xffff) as u16,
            ((ip_c >> 16) & 0xffff) as u16,
            (ip_c & 0xffff) as u16,
            ((ip_d >> 16) & 0xffff) as u16,
            (ip_d & 0xffff) as u16,
        );

        Ok(SocketAddr::V6(SocketAddrV6::new(ip, port, 0, 0)))
    } else {
        Err(build_internal_error!(format!(
            "Unable to parse {:?} as an address:port",
            s
        )))
    }
}

/// Reads TCP socket table from the provided `reader`.
pub fn read_tcp_table<R: Read>(reader: BufReader<R>) -> ProcResult<Vec<TcpNetEntry>> {
    let mut vec = Vec::new();

    // first line is a header we need to skip
    for line in reader.lines().skip(1) {
        let line = line?;
        let mut s = line.split_whitespace();
        s.next();
        let local_address = expect!(s.next(), "tcp::local_address");
        let rem_address = expect!(s.next(), "tcp::rem_address");
        let state = expect!(s.next(), "tcp::st");
        let mut tx_rx_queue = expect!(s.next(), "tcp::tx_queue:rx_queue").splitn(2, ':');
        let tx_queue = from_str!(u32, expect!(tx_rx_queue.next(), "tcp::tx_queue"), 16);
        let rx_queue = from_str!(u32, expect!(tx_rx_queue.next(), "tcp::rx_queue"), 16);
        s.next(); // skip tr and tm->when
        s.next(); // skip retrnsmt
        s.next(); // skip uid
        s.next(); // skip timeout
        let inode = expect!(s.next(), "tcp::inode");

        vec.push(TcpNetEntry {
            local_address: parse_addressport_str(local_address)?,
            remote_address: parse_addressport_str(rem_address)?,
            rx_queue,
            tx_queue,
            state: expect!(TcpState::from_u8(from_str!(u8, state, 16))),
            inode: from_str!(u64, inode),
        });
    }

    Ok(vec)
}

/// Reads UDP socket table from the provided `reader`.
pub fn read_udp_table<R: Read>(reader: BufReader<R>) -> ProcResult<Vec<UdpNetEntry>> {
    let mut vec = Vec::new();

    // first line is a header we need to skip
    for line in reader.lines().skip(1) {
        let line = line?;
        let mut s = line.split_whitespace();
        s.next();
        let local_address = expect!(s.next(), "udp::local_address");
        let rem_address = expect!(s.next(), "udp::rem_address");
        let state = expect!(s.next(), "udp::st");
        let mut tx_rx_queue = expect!(s.next(), "udp::tx_queue:rx_queue").splitn(2, ':');
        let tx_queue: u32 = from_str!(u32, expect!(tx_rx_queue.next(), "udp::tx_queue"), 16);
        let rx_queue: u32 = from_str!(u32, expect!(tx_rx_queue.next(), "udp::rx_queue"), 16);
        s.next(); // skip tr and tm->when
        s.next(); // skip retrnsmt
        s.next(); // skip uid
        s.next(); // skip timeout
        let inode = expect!(s.next(), "udp::inode");

        vec.push(UdpNetEntry {
            local_address: parse_addressport_str(local_address)?,
            remote_address: parse_addressport_str(rem_address)?,
            rx_queue,
            tx_queue,
            state: expect!(UdpState::from_u8(from_str!(u8, state, 16))),
            inode: from_str!(u64, inode),
        });
    }

    Ok(vec)
}

/// Reads the tcp socket table
pub fn tcp() -> ProcResult<Vec<TcpNetEntry>> {
    let file = FileWrapper::open("/proc/net/tcp")?;

    read_tcp_table(BufReader::new(file))
}

/// Reads the tcp6 socket table
pub fn tcp6() -> ProcResult<Vec<TcpNetEntry>> {
    let file = FileWrapper::open("/proc/net/tcp6")?;

    read_tcp_table(BufReader::new(file))
}

/// Reads the udp socket table
pub fn udp() -> ProcResult<Vec<UdpNetEntry>> {
    let file = FileWrapper::open("/proc/net/udp")?;

    read_udp_table(BufReader::new(file))
}

/// Reads the udp6 socket table
pub fn udp6() -> ProcResult<Vec<UdpNetEntry>> {
    let file = FileWrapper::open("/proc/net/udp6")?;

    read_udp_table(BufReader::new(file))
}

/// Reads the unix socket table
pub fn unix() -> ProcResult<Vec<UnixNetEntry>> {
    let file = FileWrapper::open("/proc/net/unix")?;
    let reader = BufReader::new(file);

    let mut vec = Vec::new();

    // first line is a header we need to skip
    for line in reader.lines().skip(1) {
        let line = line?;
        let mut s = line.split_whitespace();
        s.next(); // skip table slot number
        let ref_count = from_str!(u32, expect!(s.next()), 16);
        s.next(); // skip protocol, always zero
        s.next(); // skip internal kernel flags
        let socket_type = from_str!(u16, expect!(s.next()), 16);
        let state = from_str!(u8, expect!(s.next()), 16);
        let inode = from_str!(u64, expect!(s.next()));
        let path = s.next().map(PathBuf::from);

        vec.push(UnixNetEntry {
            ref_count,
            socket_type,
            inode,
            state: expect!(UnixState::from_u8(state)),
            path,
        });
    }

    Ok(vec)
}

/// An entry in the ARP table
#[derive(Debug, Clone)]
pub struct ARPEntry {
    /// IPv4 address
    pub ip_address: Ipv4Addr,
    /// Hardware type
    ///
    /// This will almost always be ETHER (or maybe INFINIBAND)
    pub hw_type: ARPHardware,
    /// Internal kernel flags
    pub flags: ARPFlags,
    /// MAC Address
    pub hw_address: Option<[u8; 6]>,
    /// Device name
    pub device: String,
}

bitflags! {
    /// Hardware type for an ARP table entry.
    // source: include/uapi/linux/if_arp.h
    pub struct ARPHardware: u32 {
        /// NET/ROM pseudo
        const NETROM = 0;
        /// Ethernet
        const ETHER = 1;
        /// Experimental ethernet
        const EETHER = 2;
        /// AX.25 Level 2
        const AX25 = 3;
        /// PROnet token ring
        const PRONET = 4;
        /// Chaosnet
        const CHAOS = 5;
        /// IEEE 802.2 Ethernet/TR/TB
        const IEEE802 = 6;
        /// Arcnet
        const ARCNET = 7;
        /// APPLEtalk
        const APPLETLK = 8;
        /// Frame Relay DLCI
        const DLCI = 15;
        /// ATM
        const ATM = 19;
        /// Metricom STRIP
        const METRICOM = 23;
        //// IEEE 1394 IPv4 - RFC 2734
        const IEEE1394 = 24;
        /// EUI-64
        const EUI64 = 27;
        /// InfiniBand
        const INFINIBAND = 32;
    }
}

bitflags! {
    /// Flags for ARP entries
    // source: include/uapi/linux/if_arp.h
    pub struct ARPFlags: u32 {
            /// Completed entry
            const COM = 0x02;
            /// Permanent entry
            const PERM = 0x04;
            /// Publish entry
            const PUBL = 0x08;
            /// Has requested trailers
            const USETRAILERS = 0x10;
            /// Want to use a netmask (only for proxy entries)
            const NETMASK = 0x20;
            // Don't answer this address
            const DONTPUB = 0x40;
    }
}

/// Reads the ARP table
pub fn arp() -> ProcResult<Vec<ARPEntry>> {
    let file = FileWrapper::open("/proc/net/arp")?;
    let reader = BufReader::new(file);

    let mut vec = Vec::new();

    // First line is a header we need to skip
    for line in reader.lines().skip(1) {
        // Check if there might have been an IO error.
        let line = line?;
        let mut line = line.split_whitespace();
        let ip_address = expect!(Ipv4Addr::from_str(expect!(line.next())));
        let hw = from_str!(u32, &expect!(line.next())[2..], 16);
        let hw = ARPHardware::from_bits_truncate(hw);
        let flags = from_str!(u32, &expect!(line.next())[2..], 16);
        let flags = ARPFlags::from_bits_truncate(flags);

        let mac = expect!(line.next());
        let mut mac: Vec<Result<u8, _>> = mac.split(':').map(|s| Ok(from_str!(u8, s, 16))).collect();

        let mac = if mac.len() == 6 {
            let mac_block_f = mac.pop().unwrap()?;
            let mac_block_e = mac.pop().unwrap()?;
            let mac_block_d = mac.pop().unwrap()?;
            let mac_block_c = mac.pop().unwrap()?;
            let mac_block_b = mac.pop().unwrap()?;
            let mac_block_a = mac.pop().unwrap()?;
            if mac_block_a == 0
                && mac_block_b == 0
                && mac_block_c == 0
                && mac_block_d == 0
                && mac_block_e == 0
                && mac_block_f == 0
            {
                None
            } else {
                Some([
                    mac_block_a,
                    mac_block_b,
                    mac_block_c,
                    mac_block_d,
                    mac_block_e,
                    mac_block_f,
                ])
            }
        } else {
            None
        };

        // mask is always "*"
        let _mask = expect!(line.next());
        let dev = expect!(line.next());

        vec.push(ARPEntry {
            ip_address,
            hw_type: hw,
            flags,
            hw_address: mac,
            device: dev.to_string(),
        })
    }

    Ok(vec)
}

/// General statistics for a network interface/device
///
/// For an example, see the [interface_stats.rs](https://github.com/eminence/procfs/tree/master/examples)
/// example in the source repo.
#[derive(Debug, Clone)]
pub struct DeviceStatus {
    /// Name of the interface
    pub name: String,
    /// Total bytes received
    pub recv_bytes: u64,
    /// Total packets received
    pub recv_packets: u64,
    /// Bad packets received
    pub recv_errs: u64,
    /// Packets dropped
    pub recv_drop: u64,
    /// Fifo overrun
    pub recv_fifo: u64,
    /// Frame alignment errors
    pub recv_frame: u64,
    /// Number of compressed packets received
    pub recv_compressed: u64,
    /// Number of multicast packets received
    pub recv_multicast: u64,

    /// Total bytes transmitted
    pub sent_bytes: u64,
    /// Total packets transmitted
    pub sent_packets: u64,
    /// Number of transmission errors
    pub sent_errs: u64,
    /// Number of packets dropped during transmission
    pub sent_drop: u64,
    pub sent_fifo: u64,
    /// Number of collisions
    pub sent_colls: u64,
    /// Number of packets not sent due to carrier errors
    pub sent_carrier: u64,
    /// Number of compressed packets transmitted
    pub sent_compressed: u64,
}

impl DeviceStatus {
    fn from_str(s: &str) -> ProcResult<DeviceStatus> {
        let mut split = s.trim().split_whitespace();
        let name: String = expect!(from_iter(&mut split));
        let recv_bytes = expect!(from_iter(&mut split));
        let recv_packets = expect!(from_iter(&mut split));
        let recv_errs = expect!(from_iter(&mut split));
        let recv_drop = expect!(from_iter(&mut split));
        let recv_fifo = expect!(from_iter(&mut split));
        let recv_frame = expect!(from_iter(&mut split));
        let recv_compressed = expect!(from_iter(&mut split));
        let recv_multicast = expect!(from_iter(&mut split));
        let sent_bytes = expect!(from_iter(&mut split));
        let sent_packets = expect!(from_iter(&mut split));
        let sent_errs = expect!(from_iter(&mut split));
        let sent_drop = expect!(from_iter(&mut split));
        let sent_fifo = expect!(from_iter(&mut split));
        let sent_colls = expect!(from_iter(&mut split));
        let sent_carrier = expect!(from_iter(&mut split));
        let sent_compressed = expect!(from_iter(&mut split));

        Ok(DeviceStatus {
            name: name.trim_end_matches(':').to_owned(),
            recv_bytes,
            recv_packets,
            recv_errs,
            recv_drop,
            recv_fifo,
            recv_frame,
            recv_compressed,
            recv_multicast,
            sent_bytes,
            sent_packets,
            sent_errs,
            sent_drop,
            sent_fifo,
            sent_colls,
            sent_carrier,
            sent_compressed,
        })
    }
}

/// Returns basic network device statistics for all interfaces
///
/// This data is from the `/proc/net/dev` file.
///
/// For an example, see the [interface_stats.rs](https://github.com/eminence/procfs/tree/master/examples)
/// example in the source repo.
pub fn dev_status() -> ProcResult<HashMap<String, DeviceStatus>> {
    let file = FileWrapper::open("/proc/net/dev")?;
    let buf = BufReader::new(file);
    let mut map = HashMap::new();
    // the first two lines are headers, so skip them
    for line in buf.lines().skip(2) {
        let dev = DeviceStatus::from_str(&line?)?;
        map.insert(dev.name.clone(), dev);
    }

    Ok(map)
}

/// An entry in the ipv4 route table
#[derive(Debug, Clone)]
pub struct RouteEntry {
    /// Interface to which packets for this route will be sent
    pub iface: String,
    /// The destination network or destination host
    pub destination: Ipv4Addr,
    pub gateway: Ipv4Addr,
    pub flags: u16,
    /// Number of references to this route
    pub refcnt: u16,
    /// Count of lookups for the route
    pub in_use: u16,
    /// The 'distance' to the target (usually counted in hops)
    pub metrics: u32,
    pub mask: Ipv4Addr,
    /// Default maximum transmission unit for TCP connections over this route
    pub mtu: u32,
    /// Default window size for TCP connections over this route
    pub window: u32,
    /// Initial RTT (Round Trip Time)
    pub irtt: u32,
}

/// Reads the ipv4 route table
///
/// This data is from the `/proc/net/route` file
pub fn route() -> ProcResult<Vec<RouteEntry>> {
    let file = FileWrapper::open("/proc/net/route")?;
    let reader = BufReader::new(file);

    let mut vec = Vec::new();

    // First line is a header we need to skip
    for line in reader.lines().skip(1) {
        // Check if there might have been an IO error.
        let line = line?;
        let mut line = line.split_whitespace();
        // network interface name, e.g. eth0
        let iface = expect!(line.next());
        let destination = from_str!(u32, expect!(line.next()), 16).to_ne_bytes().into();
        let gateway = from_str!(u32, expect!(line.next()), 16).to_ne_bytes().into();
        let flags = from_str!(u16, expect!(line.next()), 16);
        let refcnt = from_str!(u16, expect!(line.next()), 10);
        let in_use = from_str!(u16, expect!(line.next()), 10);
        let metrics = from_str!(u32, expect!(line.next()), 10);
        let mask = from_str!(u32, expect!(line.next()), 16).to_ne_bytes().into();
        let mtu = from_str!(u32, expect!(line.next()), 10);
        let window = from_str!(u32, expect!(line.next()), 10);
        let irtt = from_str!(u32, expect!(line.next()), 10);
        vec.push(RouteEntry {
            iface: iface.to_string(),
            destination,
            gateway,
            flags,
            refcnt,
            in_use,
            metrics,
            mask,
            mtu,
            window,
            irtt,
        });
    }

    Ok(vec)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;

    #[test]
    fn test_parse_ipaddr() {
        use std::str::FromStr;

        let addr = parse_addressport_str("0100007F:1234").unwrap();
        assert_eq!(addr.port(), 0x1234);
        match addr.ip() {
            IpAddr::V4(addr) => assert_eq!(addr, Ipv4Addr::new(127, 0, 0, 1)),
            _ => panic!("Not IPv4"),
        }

        // When you connect to [2a00:1450:4001:814::200e]:80 (ipv6.google.com) the entry with
        // 5014002A14080140000000000E200000:0050 remote endpoint is created in /proc/net/tcp6
        // on Linux 4.19.
        let addr = parse_addressport_str("5014002A14080140000000000E200000:0050").unwrap();
        assert_eq!(addr.port(), 80);
        match addr.ip() {
            IpAddr::V6(addr) => assert_eq!(addr, Ipv6Addr::from_str("2a00:1450:4001:814::200e").unwrap()),
            _ => panic!("Not IPv6"),
        }

        // IPv6 test case from https://stackoverflow.com/questions/41940483/parse-ipv6-addresses-from-proc-net-tcp6-python-2-7/41948004#41948004
        let addr = parse_addressport_str("B80D01200000000067452301EFCDAB89:0").unwrap();
        assert_eq!(addr.port(), 0);
        match addr.ip() {
            IpAddr::V6(addr) => assert_eq!(addr, Ipv6Addr::from_str("2001:db8::123:4567:89ab:cdef").unwrap()),
            _ => panic!("Not IPv6"),
        }

        let addr = parse_addressport_str("1234:1234");
        assert!(addr.is_err());
    }

    #[test]
    fn test_tcpstate_from() {
        assert_eq!(TcpState::from_u8(0xA).unwrap(), TcpState::Listen);
    }

    #[test]
    fn test_tcp() {
        for entry in tcp().unwrap() {
            println!("{:?}", entry);
            assert_eq!(entry.state, TcpState::from_u8(entry.state.to_u8()).unwrap());
        }
    }

    #[test]
    fn test_tcp6() {
        for entry in tcp6().unwrap() {
            println!("{:?}", entry);
            assert_eq!(entry.state, TcpState::from_u8(entry.state.to_u8()).unwrap());
        }
    }

    #[test]
    fn test_udp() {
        for entry in udp().unwrap() {
            println!("{:?}", entry);
            assert_eq!(entry.state, UdpState::from_u8(entry.state.to_u8()).unwrap());
        }
    }

    #[test]
    fn test_udp6() {
        for entry in udp6().unwrap() {
            println!("{:?}", entry);
        }
    }

    #[test]
    fn test_unix() {
        for entry in unix().unwrap() {
            println!("{:?}", entry);
        }
    }

    #[test]
    fn test_dev_status() {
        let status = dev_status().unwrap();
        println!("{:#?}", status);
    }

    #[test]
    fn test_arp() {
        for entry in arp().unwrap() {
            println!("{:?}", entry);
        }
    }

    #[test]
    fn test_route() {
        for entry in route().unwrap() {
            println!("{:?}", entry);
        }
    }
}
