use nix::ifaddrs::InterfaceAddress;
use nix::sys::socket::{AddressFamily, InetAddr, UnixAddr, getsockname};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::net::{self, Ipv6Addr, SocketAddr, SocketAddrV6};
use std::os::unix::io::RawFd;
use std::path::Path;
use std::slice;
use std::str::FromStr;
use libc::c_char;
use tempfile;

#[test]
pub fn test_inetv4_addr_to_sock_addr() {
    let actual: net::SocketAddr = FromStr::from_str("127.0.0.1:3000").unwrap();
    let addr = InetAddr::from_std(&actual);

    match addr {
        InetAddr::V4(addr) => {
            let ip: u32 = 0x7f00_0001;
            let port: u16 = 3000;
            let saddr = addr.sin_addr.s_addr;

            assert_eq!(saddr, ip.to_be());
            assert_eq!(addr.sin_port, port.to_be());
        }
        _ => panic!("nope"),
    }

    assert_eq!(addr.to_str(), "127.0.0.1:3000");

    let inet = addr.to_std();
    assert_eq!(actual, inet);
}

#[test]
pub fn test_inetv6_addr_to_sock_addr() {
    let port: u16 = 3000;
    let flowinfo: u32 = 1;
    let scope_id: u32 = 2;
    let ip: Ipv6Addr = "fe80::1".parse().unwrap();

    let actual = SocketAddr::V6(SocketAddrV6::new(ip, port, flowinfo, scope_id));
    let addr = InetAddr::from_std(&actual);

    match addr {
        InetAddr::V6(addr) => {
            assert_eq!(addr.sin6_port, port.to_be());
            assert_eq!(addr.sin6_flowinfo, flowinfo);
            assert_eq!(addr.sin6_scope_id, scope_id);
        }
        _ => panic!("nope"),
    }

    assert_eq!(actual, addr.to_std());
}

#[test]
pub fn test_path_to_sock_addr() {
    let path = "/foo/bar";
    let actual = Path::new(path);
    let addr = UnixAddr::new(actual).unwrap();

    let expect: &[c_char] = unsafe {
        slice::from_raw_parts(path.as_bytes().as_ptr() as *const c_char, path.len())
    };
    assert_eq!(&addr.0.sun_path[..8], expect);

    assert_eq!(addr.path(), Some(actual));
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

#[test]
pub fn test_addr_equality_path() {
    let path = "/foo/bar";
    let actual = Path::new(path);
    let addr1 = UnixAddr::new(actual).unwrap();
    let mut addr2 = addr1.clone();

    addr2.0.sun_path[10] = 127;

    assert_eq!(addr1, addr2);
    assert_eq!(calculate_hash(&addr1), calculate_hash(&addr2));
}

#[cfg(any(target_os = "android", target_os = "linux"))]
#[test]
pub fn test_abstract_sun_path_too_long() {
    let name = String::from("nix\0abstract\0tesnix\0abstract\0tesnix\0abstract\0tesnix\0abstract\0tesnix\0abstract\0testttttnix\0abstract\0test\0make\0sure\0this\0is\0long\0enough");
    let addr = UnixAddr::new_abstract(name.as_bytes());
    assert!(addr.is_err());
}

#[cfg(any(target_os = "android", target_os = "linux"))]
#[test]
pub fn test_addr_equality_abstract() {
    let name = String::from("nix\0abstract\0test");
    let addr1 = UnixAddr::new_abstract(name.as_bytes()).unwrap();
    let mut addr2 = addr1.clone();

    assert_eq!(addr1, addr2);
    assert_eq!(calculate_hash(&addr1), calculate_hash(&addr2));

    addr2.0.sun_path[17] = 127;
    assert_ne!(addr1, addr2);
    assert_ne!(calculate_hash(&addr1), calculate_hash(&addr2));
}

// Test getting/setting abstract addresses (without unix socket creation)
#[cfg(target_os = "linux")]
#[test]
pub fn test_abstract_uds_addr() {
    let empty = String::new();
    let addr = UnixAddr::new_abstract(empty.as_bytes()).unwrap();
    let sun_path: [u8; 0] = [];
    assert_eq!(addr.as_abstract(), Some(&sun_path[..]));

    let name = String::from("nix\0abstract\0test");
    let addr = UnixAddr::new_abstract(name.as_bytes()).unwrap();
    let sun_path = [
        110u8, 105, 120, 0, 97, 98, 115, 116, 114, 97, 99, 116, 0, 116, 101, 115, 116
    ];
    assert_eq!(addr.as_abstract(), Some(&sun_path[..]));
    assert_eq!(addr.path(), None);

    // Internally, name is null-prefixed (abstract namespace)
    assert_eq!(addr.0.sun_path[0], 0);
}

#[test]
pub fn test_getsockname() {
    use nix::sys::socket::{socket, AddressFamily, SockType, SockFlag};
    use nix::sys::socket::{bind, SockAddr};

    let tempdir = tempfile::tempdir().unwrap();
    let sockname = tempdir.path().join("sock");
    let sock = socket(AddressFamily::Unix, SockType::Stream, SockFlag::empty(), None)
               .expect("socket failed");
    let sockaddr = SockAddr::new_unix(&sockname).unwrap();
    bind(sock, &sockaddr).expect("bind failed");
    assert_eq!(sockaddr.to_str(),
               getsockname(sock).expect("getsockname failed").to_str());
}

#[test]
pub fn test_socketpair() {
    use nix::unistd::{read, write};
    use nix::sys::socket::{socketpair, AddressFamily, SockType, SockFlag};

    let (fd1, fd2) = socketpair(AddressFamily::Unix, SockType::Stream, None, SockFlag::empty())
                     .unwrap();
    write(fd1, b"hello").unwrap();
    let mut buf = [0;5];
    read(fd2, &mut buf).unwrap();

    assert_eq!(&buf[..], b"hello");
}

mod recvfrom {
    use nix::Result;
    use nix::sys::socket::*;
    use std::thread;
    use super::*;

    const MSG: &'static [u8] = b"Hello, World!";

    fn sendrecv<F>(rsock: RawFd, ssock: RawFd, f: F) -> Option<SockAddr>
        where F: Fn(RawFd, &[u8], MsgFlags) -> Result<usize> + Send + 'static
    {
        let mut buf: [u8; 13] = [0u8; 13];
        let mut l = 0;
        let mut from = None;

        let send_thread = thread::spawn(move || {
            let mut l = 0;
            while l < std::mem::size_of_val(MSG) {
                l += f(ssock, &MSG[l..], MsgFlags::empty()).unwrap();
            }
        });

        while l < std::mem::size_of_val(MSG) {
            let (len, from_) = recvfrom(rsock, &mut buf[l..]).unwrap();
            from = from_;
            l += len;
        }
        assert_eq!(&buf, MSG);
        send_thread.join().unwrap();
        from
    }

    #[test]
    pub fn stream() {
        let (fd2, fd1) = socketpair(AddressFamily::Unix, SockType::Stream,
                                    None, SockFlag::empty()).unwrap();
        // Ignore from for stream sockets
        let _ = sendrecv(fd1, fd2, |s, m, flags| {
            send(s, m, flags)
        });
    }

    #[test]
    pub fn udp() {
        let std_sa = SocketAddr::from_str("127.0.0.1:6789").unwrap();
        let inet_addr = InetAddr::from_std(&std_sa);
        let sock_addr = SockAddr::new_inet(inet_addr);
        let rsock = socket(AddressFamily::Inet,
            SockType::Datagram,
            SockFlag::empty(),
            None
        ).unwrap();
        bind(rsock, &sock_addr).unwrap();
        let ssock = socket(
            AddressFamily::Inet,
            SockType::Datagram,
            SockFlag::empty(),
            None,
        ).expect("send socket failed");
        let from = sendrecv(rsock, ssock, move |s, m, flags| {
            sendto(s, m, &sock_addr, flags)
        });
        // UDP sockets should set the from address
        assert_eq!(AddressFamily::Inet, from.unwrap().family());
    }
}

// Test error handling of our recvmsg wrapper
#[test]
pub fn test_recvmsg_ebadf() {
    use nix::Error;
    use nix::errno::Errno;
    use nix::sys::socket::{MsgFlags, recvmsg};
    use nix::sys::uio::IoVec;

    let mut buf = [0u8; 5];
    let iov = [IoVec::from_mut_slice(&mut buf[..])];
    let fd = -1;    // Bad file descriptor
    let r = recvmsg(fd, &iov, None, MsgFlags::empty());
    assert_eq!(r.err().unwrap(), Error::Sys(Errno::EBADF));
}

// Disable the test on emulated platforms due to a bug in QEMU versions <
// 2.12.0.  https://bugs.launchpad.net/qemu/+bug/1701808
#[cfg_attr(not(any(target_arch = "x86_64", target_arch="i686")), ignore)]
#[test]
pub fn test_scm_rights() {
    use nix::sys::uio::IoVec;
    use nix::unistd::{pipe, read, write, close};
    use nix::sys::socket::{socketpair, sendmsg, recvmsg,
                           AddressFamily, SockType, SockFlag,
                           ControlMessage, ControlMessageOwned, MsgFlags};

    let (fd1, fd2) = socketpair(AddressFamily::Unix, SockType::Stream, None, SockFlag::empty())
                     .unwrap();
    let (r, w) = pipe().unwrap();
    let mut received_r: Option<RawFd> = None;

    {
        let iov = [IoVec::from_slice(b"hello")];
        let fds = [r];
        let cmsg = ControlMessage::ScmRights(&fds);
        assert_eq!(sendmsg(fd1, &iov, &[cmsg], MsgFlags::empty(), None).unwrap(), 5);
        close(r).unwrap();
        close(fd1).unwrap();
    }

    {
        let mut buf = [0u8; 5];
        let iov = [IoVec::from_mut_slice(&mut buf[..])];
        let mut cmsgspace = cmsg_space!([RawFd; 1]);
        let msg = recvmsg(fd2, &iov, Some(&mut cmsgspace), MsgFlags::empty()).unwrap();

        for cmsg in msg.cmsgs() {
            if let ControlMessageOwned::ScmRights(fd) = cmsg {
                assert_eq!(received_r, None);
                assert_eq!(fd.len(), 1);
                received_r = Some(fd[0]);
            } else {
                panic!("unexpected cmsg");
            }
        }
        assert_eq!(msg.bytes, 5);
        assert!(!msg.flags.intersects(MsgFlags::MSG_TRUNC | MsgFlags::MSG_CTRUNC));
        close(fd2).unwrap();
    }

    let received_r = received_r.expect("Did not receive passed fd");
    // Ensure that the received file descriptor works
    write(w, b"world").unwrap();
    let mut buf = [0u8; 5];
    read(received_r, &mut buf).unwrap();
    assert_eq!(&buf[..], b"world");
    close(received_r).unwrap();
    close(w).unwrap();
}

// Disable the test on emulated platforms due to not enabled support of AF_ALG in QEMU from rust cross
#[cfg_attr(not(any(target_arch = "x86_64", target_arch = "i686")), ignore)]
#[cfg(any(target_os = "linux", target_os= "android"))]
#[test]
pub fn test_af_alg_cipher() {
    use libc;
    use nix::sys::uio::IoVec;
    use nix::unistd::read;
    use nix::sys::socket::{socket, sendmsg, bind, accept, setsockopt,
                           AddressFamily, SockType, SockFlag, SockAddr,
                           ControlMessage, MsgFlags};
    use nix::sys::socket::sockopt::AlgSetKey;

    // Travis's seccomp profile blocks AF_ALG
    // https://docs.docker.com/engine/security/seccomp/
    skip_if_seccomp!(test_af_alg_cipher);

    let alg_type = "skcipher";
    let alg_name = "ctr(aes)";
    // 256-bits secret key
    let key = vec![0u8; 32];
    // 16-bytes IV
    let iv_len = 16;
    let iv = vec![1u8; iv_len];
    // 256-bytes plain payload
    let payload_len = 256;
    let payload = vec![2u8; payload_len];

    let sock = socket(AddressFamily::Alg, SockType::SeqPacket, SockFlag::empty(), None)
        .expect("socket failed");

    let sockaddr = SockAddr::new_alg(alg_type, alg_name);
    bind(sock, &sockaddr).expect("bind failed");

    if let SockAddr::Alg(alg) = sockaddr {
        assert_eq!(alg.alg_name().to_string_lossy(), alg_name);
        assert_eq!(alg.alg_type().to_string_lossy(), alg_type);
    } else {
        panic!("unexpected SockAddr");
    }

    setsockopt(sock, AlgSetKey::default(), &key).expect("setsockopt");
    let session_socket = accept(sock).expect("accept failed");

    let msgs = [ControlMessage::AlgSetOp(&libc::ALG_OP_ENCRYPT), ControlMessage::AlgSetIv(iv.as_slice())];
    let iov = IoVec::from_slice(&payload);
    sendmsg(session_socket, &[iov], &msgs, MsgFlags::empty(), None).expect("sendmsg encrypt");

    // allocate buffer for encrypted data
    let mut encrypted = vec![0u8; payload_len];
    let num_bytes = read(session_socket, &mut encrypted).expect("read encrypt");
    assert_eq!(num_bytes, payload_len);

    let iov = IoVec::from_slice(&encrypted);

    let iv = vec![1u8; iv_len];

    let msgs = [ControlMessage::AlgSetOp(&libc::ALG_OP_DECRYPT), ControlMessage::AlgSetIv(iv.as_slice())];
    sendmsg(session_socket, &[iov], &msgs, MsgFlags::empty(), None).expect("sendmsg decrypt");

    // allocate buffer for decrypted data
    let mut decrypted = vec![0u8; payload_len];
    let num_bytes = read(session_socket, &mut decrypted).expect("read decrypt");

    assert_eq!(num_bytes, payload_len);
    assert_eq!(decrypted, payload);
}

// Disable the test on emulated platforms due to not enabled support of AF_ALG in QEMU from rust cross
#[cfg_attr(not(any(target_arch = "x86_64", target_arch = "i686")), ignore)]
#[cfg(any(target_os = "linux", target_os= "android"))]
#[test]
pub fn test_af_alg_aead() {
    use libc::{ALG_OP_DECRYPT, ALG_OP_ENCRYPT};
    use nix::sys::uio::IoVec;
    use nix::unistd::{read, close};
    use nix::sys::socket::{socket, sendmsg, bind, accept, setsockopt,
                           AddressFamily, SockType, SockFlag, SockAddr,
                           ControlMessage, MsgFlags};
    use nix::sys::socket::sockopt::{AlgSetKey, AlgSetAeadAuthSize};

    // Travis's seccomp profile blocks AF_ALG
    // https://docs.docker.com/engine/security/seccomp/
    skip_if_seccomp!(test_af_alg_aead);

    let auth_size = 4usize;
    let assoc_size = 16u32;

    let alg_type = "aead";
    let alg_name = "gcm(aes)";
    // 256-bits secret key
    let key = vec![0u8; 32];
    // 12-bytes IV
    let iv_len = 12;
    let iv = vec![1u8; iv_len];
    // 256-bytes plain payload
    let payload_len = 256;
    let mut payload = vec![2u8; payload_len + (assoc_size as usize) + auth_size];

    for i in 0..assoc_size {
        payload[i as usize] = 10;
    }

    let len = payload.len();

    for i in 0..auth_size {
        payload[len - 1 - i] = 0;
    }

    let sock = socket(AddressFamily::Alg, SockType::SeqPacket, SockFlag::empty(), None)
        .expect("socket failed");

    let sockaddr = SockAddr::new_alg(alg_type, alg_name);
    bind(sock, &sockaddr).expect("bind failed");

    setsockopt(sock, AlgSetAeadAuthSize, &auth_size).expect("setsockopt AlgSetAeadAuthSize");
    setsockopt(sock, AlgSetKey::default(), &key).expect("setsockopt AlgSetKey");
    let session_socket = accept(sock).expect("accept failed");

    let msgs = [
        ControlMessage::AlgSetOp(&ALG_OP_ENCRYPT),
        ControlMessage::AlgSetIv(iv.as_slice()),
        ControlMessage::AlgSetAeadAssoclen(&assoc_size)];
    let iov = IoVec::from_slice(&payload);
    sendmsg(session_socket, &[iov], &msgs, MsgFlags::empty(), None).expect("sendmsg encrypt");

    // allocate buffer for encrypted data
    let mut encrypted = vec![0u8; (assoc_size as usize) + payload_len + auth_size];
    let num_bytes = read(session_socket, &mut encrypted).expect("read encrypt");
    assert_eq!(num_bytes, payload_len + auth_size + (assoc_size as usize));
    close(session_socket).expect("close");

    for i in 0..assoc_size {
        encrypted[i as usize] = 10;
    }

    let iov = IoVec::from_slice(&encrypted);

    let iv = vec![1u8; iv_len];

    let session_socket = accept(sock).expect("accept failed");

    let msgs = [
        ControlMessage::AlgSetOp(&ALG_OP_DECRYPT),
        ControlMessage::AlgSetIv(iv.as_slice()),
        ControlMessage::AlgSetAeadAssoclen(&assoc_size),
    ];
    sendmsg(session_socket, &[iov], &msgs, MsgFlags::empty(), None).expect("sendmsg decrypt");

    // allocate buffer for decrypted data
    let mut decrypted = vec![0u8; payload_len + (assoc_size as usize) + auth_size];
    let num_bytes = read(session_socket, &mut decrypted).expect("read decrypt");

    assert!(num_bytes >= payload_len + (assoc_size as usize));
    assert_eq!(decrypted[(assoc_size as usize)..(payload_len + (assoc_size as usize))], payload[(assoc_size as usize)..payload_len + (assoc_size as usize)]);
}

/// Tests that passing multiple fds using a single `ControlMessage` works.
// Disable the test on emulated platforms due to a bug in QEMU versions <
// 2.12.0.  https://bugs.launchpad.net/qemu/+bug/1701808
#[cfg_attr(not(any(target_arch = "x86_64", target_arch="i686")), ignore)]
#[test]
fn test_scm_rights_single_cmsg_multiple_fds() {
    use std::os::unix::net::UnixDatagram;
    use std::os::unix::io::{RawFd, AsRawFd};
    use std::thread;
    use nix::sys::socket::{ControlMessage, ControlMessageOwned, MsgFlags,
        sendmsg, recvmsg};
    use nix::sys::uio::IoVec;
    use libc;

    let (send, receive) = UnixDatagram::pair().unwrap();
    let thread = thread::spawn(move || {
        let mut buf = [0u8; 8];
        let iovec = [IoVec::from_mut_slice(&mut buf)];
        let mut space = cmsg_space!([RawFd; 2]);
        let msg = recvmsg(
            receive.as_raw_fd(),
            &iovec,
            Some(&mut space),
            MsgFlags::empty()
        ).unwrap();
        assert!(!msg.flags.intersects(MsgFlags::MSG_TRUNC | MsgFlags::MSG_CTRUNC));

        let mut cmsgs = msg.cmsgs();
        match cmsgs.next() {
            Some(ControlMessageOwned::ScmRights(fds)) => {
                assert_eq!(fds.len(), 2,
                           "unexpected fd count (expected 2 fds, got {})",
                           fds.len());
            },
            _ => panic!(),
        }
        assert!(cmsgs.next().is_none(), "unexpected control msg");

        assert_eq!(msg.bytes, 8);
        assert_eq!(iovec[0].as_slice(), [1u8, 2, 3, 4, 5, 6, 7, 8]);
    });

    let slice = [1u8, 2, 3, 4, 5, 6, 7, 8];
    let iov = [IoVec::from_slice(&slice)];
    let fds = [libc::STDIN_FILENO, libc::STDOUT_FILENO];    // pass stdin and stdout
    let cmsg = [ControlMessage::ScmRights(&fds)];
    sendmsg(send.as_raw_fd(), &iov, &cmsg, MsgFlags::empty(), None).unwrap();
    thread.join().unwrap();
}

// Verify `sendmsg` builds a valid `msghdr` when passing an empty
// `cmsgs` argument.  This should result in a msghdr with a nullptr
// msg_control field and a msg_controllen of 0 when calling into the
// raw `sendmsg`.
#[test]
pub fn test_sendmsg_empty_cmsgs() {
    use nix::sys::uio::IoVec;
    use nix::unistd::close;
    use nix::sys::socket::{socketpair, sendmsg, recvmsg,
                           AddressFamily, SockType, SockFlag, MsgFlags};

    let (fd1, fd2) = socketpair(AddressFamily::Unix, SockType::Stream, None, SockFlag::empty())
                     .unwrap();

    {
        let iov = [IoVec::from_slice(b"hello")];
        assert_eq!(sendmsg(fd1, &iov, &[], MsgFlags::empty(), None).unwrap(), 5);
        close(fd1).unwrap();
    }

    {
        let mut buf = [0u8; 5];
        let iov = [IoVec::from_mut_slice(&mut buf[..])];
        let mut cmsgspace = cmsg_space!([RawFd; 1]);
        let msg = recvmsg(fd2, &iov, Some(&mut cmsgspace), MsgFlags::empty()).unwrap();

        for _ in msg.cmsgs() {
            panic!("unexpected cmsg");
        }
        assert!(!msg.flags.intersects(MsgFlags::MSG_TRUNC | MsgFlags::MSG_CTRUNC));
        assert_eq!(msg.bytes, 5);
        close(fd2).unwrap();
    }
}

#[cfg(any(target_os = "android", target_os = "linux"))]
#[test]
fn test_scm_credentials() {
    use libc;
    use nix::sys::uio::IoVec;
    use nix::unistd::{close, getpid, getuid, getgid};
    use nix::sys::socket::{socketpair, sendmsg, recvmsg, setsockopt,
                           AddressFamily, SockType, SockFlag,
                           ControlMessage, ControlMessageOwned, MsgFlags};
    use nix::sys::socket::sockopt::PassCred;

    let (send, recv) = socketpair(AddressFamily::Unix, SockType::Stream, None, SockFlag::empty())
        .unwrap();
    setsockopt(recv, PassCred, &true).unwrap();

    {
        let iov = [IoVec::from_slice(b"hello")];
        let cred = libc::ucred {
            pid: getpid().as_raw(),
            uid: getuid().as_raw(),
            gid: getgid().as_raw(),
        }.into();
        let cmsg = ControlMessage::ScmCredentials(&cred);
        assert_eq!(sendmsg(send, &iov, &[cmsg], MsgFlags::empty(), None).unwrap(), 5);
        close(send).unwrap();
    }

    {
        let mut buf = [0u8; 5];
        let iov = [IoVec::from_mut_slice(&mut buf[..])];
        let mut cmsgspace = cmsg_space!(libc::ucred);
        let msg = recvmsg(recv, &iov, Some(&mut cmsgspace), MsgFlags::empty()).unwrap();
        let mut received_cred = None;

        for cmsg in msg.cmsgs() {
            if let ControlMessageOwned::ScmCredentials(cred) = cmsg {
                assert!(received_cred.is_none());
                assert_eq!(cred.pid(), getpid().as_raw());
                assert_eq!(cred.uid(), getuid().as_raw());
                assert_eq!(cred.gid(), getgid().as_raw());
                received_cred = Some(cred);
            } else {
                panic!("unexpected cmsg");
            }
        }
        received_cred.expect("no creds received");
        assert_eq!(msg.bytes, 5);
        assert!(!msg.flags.intersects(MsgFlags::MSG_TRUNC | MsgFlags::MSG_CTRUNC));
        close(recv).unwrap();
    }
}

/// Ensure that we can send `SCM_CREDENTIALS` and `SCM_RIGHTS` with a single
/// `sendmsg` call.
#[cfg(any(target_os = "android", target_os = "linux"))]
// qemu's handling of multiple cmsgs is bugged, ignore tests on non-x86
// see https://bugs.launchpad.net/qemu/+bug/1781280
#[cfg_attr(not(any(target_arch = "x86_64", target_arch = "x86")), ignore)]
#[test]
fn test_scm_credentials_and_rights() {
    use libc;

    let space = cmsg_space!(libc::ucred, RawFd);
    test_impl_scm_credentials_and_rights(space);
}

/// Ensure that passing a an oversized control message buffer to recvmsg
/// still works.
#[cfg(any(target_os = "android", target_os = "linux"))]
// qemu's handling of multiple cmsgs is bugged, ignore tests on non-x86
// see https://bugs.launchpad.net/qemu/+bug/1781280
#[cfg_attr(not(any(target_arch = "x86_64", target_arch = "x86")), ignore)]
#[test]
fn test_too_large_cmsgspace() {
    let space = vec![0u8; 1024];
    test_impl_scm_credentials_and_rights(space);
}

#[cfg(any(target_os = "android", target_os = "linux"))]
fn test_impl_scm_credentials_and_rights(mut space: Vec<u8>) {
    use libc::ucred;
    use nix::sys::uio::IoVec;
    use nix::unistd::{pipe, read, write, close, getpid, getuid, getgid};
    use nix::sys::socket::{socketpair, sendmsg, recvmsg, setsockopt,
                           SockType, SockFlag,
                           ControlMessage, ControlMessageOwned, MsgFlags};
    use nix::sys::socket::sockopt::PassCred;

    let (send, recv) = socketpair(AddressFamily::Unix, SockType::Stream, None, SockFlag::empty())
        .unwrap();
    setsockopt(recv, PassCred, &true).unwrap();

    let (r, w) = pipe().unwrap();
    let mut received_r: Option<RawFd> = None;

    {
        let iov = [IoVec::from_slice(b"hello")];
        let cred = ucred {
            pid: getpid().as_raw(),
            uid: getuid().as_raw(),
            gid: getgid().as_raw(),
        }.into();
        let fds = [r];
        let cmsgs = [
            ControlMessage::ScmCredentials(&cred),
            ControlMessage::ScmRights(&fds),
        ];
        assert_eq!(sendmsg(send, &iov, &cmsgs, MsgFlags::empty(), None).unwrap(), 5);
        close(r).unwrap();
        close(send).unwrap();
    }

    {
        let mut buf = [0u8; 5];
        let iov = [IoVec::from_mut_slice(&mut buf[..])];
        let msg = recvmsg(recv, &iov, Some(&mut space), MsgFlags::empty()).unwrap();
        let mut received_cred = None;

        assert_eq!(msg.cmsgs().count(), 2, "expected 2 cmsgs");

        for cmsg in msg.cmsgs() {
            match cmsg {
                ControlMessageOwned::ScmRights(fds) => {
                    assert_eq!(received_r, None, "already received fd");
                    assert_eq!(fds.len(), 1);
                    received_r = Some(fds[0]);
                }
                ControlMessageOwned::ScmCredentials(cred) => {
                    assert!(received_cred.is_none());
                    assert_eq!(cred.pid(), getpid().as_raw());
                    assert_eq!(cred.uid(), getuid().as_raw());
                    assert_eq!(cred.gid(), getgid().as_raw());
                    received_cred = Some(cred);
                }
                _ => panic!("unexpected cmsg"),
            }
        }
        received_cred.expect("no creds received");
        assert_eq!(msg.bytes, 5);
        assert!(!msg.flags.intersects(MsgFlags::MSG_TRUNC | MsgFlags::MSG_CTRUNC));
        close(recv).unwrap();
    }

    let received_r = received_r.expect("Did not receive passed fd");
    // Ensure that the received file descriptor works
    write(w, b"world").unwrap();
    let mut buf = [0u8; 5];
    read(received_r, &mut buf).unwrap();
    assert_eq!(&buf[..], b"world");
    close(received_r).unwrap();
    close(w).unwrap();
}

// Test creating and using named unix domain sockets
#[test]
pub fn test_unixdomain() {
    use nix::sys::socket::{SockType, SockFlag};
    use nix::sys::socket::{bind, socket, connect, listen, accept, SockAddr};
    use nix::unistd::{read, write, close};
    use std::thread;

    let tempdir = tempfile::tempdir().unwrap();
    let sockname = tempdir.path().join("sock");
    let s1 = socket(AddressFamily::Unix, SockType::Stream,
                    SockFlag::empty(), None).expect("socket failed");
    let sockaddr = SockAddr::new_unix(&sockname).unwrap();
    bind(s1, &sockaddr).expect("bind failed");
    listen(s1, 10).expect("listen failed");

    let thr = thread::spawn(move || {
        let s2 = socket(AddressFamily::Unix, SockType::Stream, SockFlag::empty(), None)
                 .expect("socket failed");
        connect(s2, &sockaddr).expect("connect failed");
        write(s2, b"hello").expect("write failed");
        close(s2).unwrap();
    });

    let s3 = accept(s1).expect("accept failed");

    let mut buf = [0;5];
    read(s3, &mut buf).unwrap();
    close(s3).unwrap();
    close(s1).unwrap();
    thr.join().unwrap();

    assert_eq!(&buf[..], b"hello");
}

// Test creating and using named system control sockets
#[cfg(any(target_os = "macos", target_os = "ios"))]
#[test]
pub fn test_syscontrol() {
    use nix::Error;
    use nix::errno::Errno;
    use nix::sys::socket::{socket, SockAddr, SockType, SockFlag, SockProtocol};

    let fd = socket(AddressFamily::System, SockType::Datagram,
                    SockFlag::empty(), SockProtocol::KextControl)
             .expect("socket failed");
    let _sockaddr = SockAddr::new_sys_control(fd, "com.apple.net.utun_control", 0).expect("resolving sys_control name failed");
    assert_eq!(SockAddr::new_sys_control(fd, "foo.bar.lol", 0).err(), Some(Error::Sys(Errno::ENOENT)));

    // requires root privileges
    // connect(fd, &sockaddr).expect("connect failed");
}

#[cfg(any(
    target_os = "android",
    target_os = "freebsd",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos",
    target_os = "netbsd",
    target_os = "openbsd",
))]
fn loopback_address(family: AddressFamily) -> Option<InterfaceAddress> {
    use std::io;
    use std::io::Write;
    use nix::ifaddrs::getifaddrs;
    use nix::sys::socket::SockAddr;
    use nix::net::if_::*;

    let addrs = match getifaddrs() {
        Ok(iter) => iter,
        Err(e) => {
            let stdioerr = io::stderr();
            let mut handle = stdioerr.lock();
            writeln!(handle, "getifaddrs: {:?}", e).unwrap();
            return None;
        },
    };
    // return first address matching family
    for ifaddr in addrs {
        if ifaddr.flags.contains(InterfaceFlags::IFF_LOOPBACK) {
            match ifaddr.address {
                Some(SockAddr::Inet(InetAddr::V4(..))) => {
                    match family {
                        AddressFamily::Inet => return Some(ifaddr),
                        _ => continue
                    }
                },
                Some(SockAddr::Inet(InetAddr::V6(..))) => {
                    match family {
                        AddressFamily::Inet6 => return Some(ifaddr),
                        _ => continue
                    }
                },
                _ => continue,
            }
        }
    }
    None
}

#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos",
    target_os = "netbsd",
))]
// qemu doesn't seem to be emulating this correctly in these architectures
#[cfg_attr(any(
    target_arch = "mips",
    target_arch = "mips64",
    target_arch = "powerpc64",
), ignore)]
#[test]
pub fn test_recv_ipv4pktinfo() {
    use libc;
    use nix::sys::socket::sockopt::Ipv4PacketInfo;
    use nix::sys::socket::{bind, SockFlag, SockType};
    use nix::sys::socket::{getsockname, setsockopt, socket};
    use nix::sys::socket::{recvmsg, sendmsg, ControlMessageOwned, MsgFlags};
    use nix::sys::uio::IoVec;
    use nix::net::if_::*;

    let lo_ifaddr = loopback_address(AddressFamily::Inet);
    let (lo_name, lo) = match lo_ifaddr {
        Some(ifaddr) => (ifaddr.interface_name,
                         ifaddr.address.expect("Expect IPv4 address on interface")),
        None => return,
    };
    let receive = socket(
            AddressFamily::Inet,
            SockType::Datagram,
            SockFlag::empty(),
            None,
        ).expect("receive socket failed");
    bind(receive, &lo).expect("bind failed");
    let sa = getsockname(receive).expect("getsockname failed");
    setsockopt(receive, Ipv4PacketInfo, &true).expect("setsockopt failed");

    {
        let slice = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let iov = [IoVec::from_slice(&slice)];

        let send = socket(
            AddressFamily::Inet,
            SockType::Datagram,
            SockFlag::empty(),
            None,
        ).expect("send socket failed");
        sendmsg(send, &iov, &[], MsgFlags::empty(), Some(&sa)).expect("sendmsg failed");
    }

    {
        let mut buf = [0u8; 8];
        let iovec = [IoVec::from_mut_slice(&mut buf)];
        let mut space = cmsg_space!(libc::in_pktinfo);
        let msg = recvmsg(
            receive,
            &iovec,
            Some(&mut space),
            MsgFlags::empty(),
        ).expect("recvmsg failed");
        assert!(
            !msg.flags
                .intersects(MsgFlags::MSG_TRUNC | MsgFlags::MSG_CTRUNC)
        );

        let mut cmsgs = msg.cmsgs();
        match cmsgs.next() {
            Some(ControlMessageOwned::Ipv4PacketInfo(pktinfo)) => {
                let i = if_nametoindex(lo_name.as_bytes()).expect("if_nametoindex");
                assert_eq!(
                    pktinfo.ipi_ifindex as libc::c_uint,
                    i,
                    "unexpected ifindex (expected {}, got {})",
                    i,
                    pktinfo.ipi_ifindex
                );
            }
            _ => (),
        }
        assert!(cmsgs.next().is_none(), "unexpected additional control msg");
        assert_eq!(msg.bytes, 8);
        assert_eq!(
            iovec[0].as_slice(),
            [1u8, 2, 3, 4, 5, 6, 7, 8]
        );
    }
}

#[cfg(any(
    target_os = "freebsd",
    target_os = "ios",
    target_os = "macos",
    target_os = "netbsd",
    target_os = "openbsd",
))]
// qemu doesn't seem to be emulating this correctly in these architectures
#[cfg_attr(any(
    target_arch = "mips",
    target_arch = "mips64",
    target_arch = "powerpc64",
), ignore)]
#[test]
pub fn test_recvif() {
    use libc;
    use nix::net::if_::*;
    use nix::sys::socket::sockopt::{Ipv4RecvIf, Ipv4RecvDstAddr};
    use nix::sys::socket::{bind, SockFlag, SockType};
    use nix::sys::socket::{getsockname, setsockopt, socket, SockAddr};
    use nix::sys::socket::{recvmsg, sendmsg, ControlMessageOwned, MsgFlags};
    use nix::sys::uio::IoVec;

    let lo_ifaddr = loopback_address(AddressFamily::Inet);
    let (lo_name, lo) = match lo_ifaddr {
        Some(ifaddr) => (ifaddr.interface_name,
                         ifaddr.address.expect("Expect IPv4 address on interface")),
        None => return,
    };
    let receive = socket(
        AddressFamily::Inet,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    ).expect("receive socket failed");
    bind(receive, &lo).expect("bind failed");
    let sa = getsockname(receive).expect("getsockname failed");
    setsockopt(receive, Ipv4RecvIf, &true).expect("setsockopt IP_RECVIF failed");
    setsockopt(receive, Ipv4RecvDstAddr, &true).expect("setsockopt IP_RECVDSTADDR failed");

    {
        let slice = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let iov = [IoVec::from_slice(&slice)];

        let send = socket(
            AddressFamily::Inet,
            SockType::Datagram,
            SockFlag::empty(),
            None,
        ).expect("send socket failed");
        sendmsg(send, &iov, &[], MsgFlags::empty(), Some(&sa)).expect("sendmsg failed");
    }

    {
        let mut buf = [0u8; 8];
        let iovec = [IoVec::from_mut_slice(&mut buf)];
        let mut space = cmsg_space!(libc::sockaddr_dl, libc::in_addr);
        let msg = recvmsg(
            receive,
            &iovec,
            Some(&mut space),
            MsgFlags::empty(),
        ).expect("recvmsg failed");
        assert!(
            !msg.flags
                .intersects(MsgFlags::MSG_TRUNC | MsgFlags::MSG_CTRUNC)
        );
        assert_eq!(msg.cmsgs().count(), 2, "expected 2 cmsgs");

        let mut rx_recvif = false;
        let mut rx_recvdstaddr = false;
        for cmsg in msg.cmsgs() {
            match cmsg {
                ControlMessageOwned::Ipv4RecvIf(dl) => {
                    rx_recvif = true;
                    let i = if_nametoindex(lo_name.as_bytes()).expect("if_nametoindex");
                    assert_eq!(
                        dl.sdl_index as libc::c_uint,
                        i,
                        "unexpected ifindex (expected {}, got {})",
                        i,
                        dl.sdl_index
                    );
                },
                ControlMessageOwned::Ipv4RecvDstAddr(addr) => {
                    rx_recvdstaddr = true;
                    if let SockAddr::Inet(InetAddr::V4(a)) = lo {
                        assert_eq!(a.sin_addr.s_addr,
                                   addr.s_addr,
                                   "unexpected destination address (expected {}, got {})",
                                   a.sin_addr.s_addr,
                                   addr.s_addr);
                    } else {
                        panic!("unexpected Sockaddr");
                    }
                },
                _ => panic!("unexpected additional control msg"),
            }
        }
        assert_eq!(rx_recvif, true);
        assert_eq!(rx_recvdstaddr, true);
        assert_eq!(msg.bytes, 8);
        assert_eq!(
            iovec[0].as_slice(),
            [1u8, 2, 3, 4, 5, 6, 7, 8]
        );
    }
}

#[cfg(any(
    target_os = "android",
    target_os = "freebsd",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos",
    target_os = "netbsd",
    target_os = "openbsd",
))]
// qemu doesn't seem to be emulating this correctly in these architectures
#[cfg_attr(any(
    target_arch = "mips",
    target_arch = "mips64",
    target_arch = "powerpc64",
), ignore)]
#[test]
pub fn test_recv_ipv6pktinfo() {
    use libc;
    use nix::net::if_::*;
    use nix::sys::socket::sockopt::Ipv6RecvPacketInfo;
    use nix::sys::socket::{bind, SockFlag, SockType};
    use nix::sys::socket::{getsockname, setsockopt, socket};
    use nix::sys::socket::{recvmsg, sendmsg, ControlMessageOwned, MsgFlags};
    use nix::sys::uio::IoVec;

    let lo_ifaddr = loopback_address(AddressFamily::Inet6);
    let (lo_name, lo) = match lo_ifaddr {
        Some(ifaddr) => (ifaddr.interface_name,
                         ifaddr.address.expect("Expect IPv4 address on interface")),
        None => return,
    };
    let receive = socket(
        AddressFamily::Inet6,
        SockType::Datagram,
        SockFlag::empty(),
        None,
    ).expect("receive socket failed");
    bind(receive, &lo).expect("bind failed");
    let sa = getsockname(receive).expect("getsockname failed");
    setsockopt(receive, Ipv6RecvPacketInfo, &true).expect("setsockopt failed");

    {
        let slice = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let iov = [IoVec::from_slice(&slice)];

        let send = socket(
            AddressFamily::Inet6,
            SockType::Datagram,
            SockFlag::empty(),
            None,
        ).expect("send socket failed");
        sendmsg(send, &iov, &[], MsgFlags::empty(), Some(&sa)).expect("sendmsg failed");
    }

    {
        let mut buf = [0u8; 8];
        let iovec = [IoVec::from_mut_slice(&mut buf)];
        let mut space = cmsg_space!(libc::in6_pktinfo);
        let msg = recvmsg(
            receive,
            &iovec,
            Some(&mut space),
            MsgFlags::empty(),
        ).expect("recvmsg failed");
        assert!(
            !msg.flags
                .intersects(MsgFlags::MSG_TRUNC | MsgFlags::MSG_CTRUNC)
        );

        let mut cmsgs = msg.cmsgs();
        match cmsgs.next() {
            Some(ControlMessageOwned::Ipv6PacketInfo(pktinfo)) => {
                let i = if_nametoindex(lo_name.as_bytes()).expect("if_nametoindex");
                assert_eq!(
                    pktinfo.ipi6_ifindex,
                    i,
                    "unexpected ifindex (expected {}, got {})",
                    i,
                    pktinfo.ipi6_ifindex
                );
            }
            _ => (),
        }
        assert!(cmsgs.next().is_none(), "unexpected additional control msg");
        assert_eq!(msg.bytes, 8);
        assert_eq!(
            iovec[0].as_slice(),
            [1u8, 2, 3, 4, 5, 6, 7, 8]
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
pub fn test_vsock() {
    use libc;
    use nix::Error;
    use nix::errno::Errno;
    use nix::sys::socket::{AddressFamily, socket, bind, connect, listen,
                           SockAddr, SockType, SockFlag};
    use nix::unistd::{close};
    use std::thread;

    let port: u32 = 3000;

    let s1 = socket(AddressFamily::Vsock,  SockType::Stream,
                    SockFlag::empty(), None)
             .expect("socket failed");

    // VMADDR_CID_HYPERVISOR and VMADDR_CID_RESERVED are reserved, so we expect
    // an EADDRNOTAVAIL error.
    let sockaddr = SockAddr::new_vsock(libc::VMADDR_CID_HYPERVISOR, port);
    assert_eq!(bind(s1, &sockaddr).err(),
               Some(Error::Sys(Errno::EADDRNOTAVAIL)));

    let sockaddr = SockAddr::new_vsock(libc::VMADDR_CID_RESERVED, port);
    assert_eq!(bind(s1, &sockaddr).err(),
               Some(Error::Sys(Errno::EADDRNOTAVAIL)));


    let sockaddr = SockAddr::new_vsock(libc::VMADDR_CID_ANY, port);
    assert_eq!(bind(s1, &sockaddr), Ok(()));
    listen(s1, 10).expect("listen failed");

    let thr = thread::spawn(move || {
        let cid: u32 = libc::VMADDR_CID_HOST;

        let s2 = socket(AddressFamily::Vsock, SockType::Stream,
                        SockFlag::empty(), None)
                 .expect("socket failed");

        let sockaddr = SockAddr::new_vsock(cid, port);

        // The current implementation does not support loopback devices, so,
        // for now, we expect a failure on the connect.
        assert_ne!(connect(s2, &sockaddr), Ok(()));

        close(s2).unwrap();
    });

    close(s1).unwrap();
    thr.join().unwrap();
}
