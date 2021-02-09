//! UDP for IOCP
//!
//! Note that most of this module is quite similar to the TCP module, so if
//! something seems odd you may also want to try the docs over there.

use std::fmt;
use std::io::prelude::*;
use std::io;
use std::mem;
use std::net::{self, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::{Mutex, MutexGuard};

#[allow(unused_imports)]
use net2::{UdpBuilder, UdpSocketExt};
use winapi::*;
use miow::iocp::CompletionStatus;
use miow::net::SocketAddrBuf;
use miow::net::UdpSocketExt as MiowUdpSocketExt;

use {poll, Ready, Poll, PollOpt, Token};
use event::Evented;
use sys::windows::from_raw_arc::FromRawArc;
use sys::windows::selector::{Overlapped, ReadyBinding};

pub struct UdpSocket {
    imp: Imp,
    registration: Mutex<Option<poll::Registration>>,
}

#[derive(Clone)]
struct Imp {
    inner: FromRawArc<Io>,
}

struct Io {
    read: Overlapped,
    write: Overlapped,
    socket: net::UdpSocket,
    inner: Mutex<Inner>,
}

struct Inner {
    iocp: ReadyBinding,
    read: State<Vec<u8>, Vec<u8>>,
    write: State<Vec<u8>, (Vec<u8>, usize)>,
    read_buf: SocketAddrBuf,
}

enum State<T, U> {
    Empty,
    Pending(T),
    Ready(U),
    Error(io::Error),
}

impl UdpSocket {
    pub fn new(socket: net::UdpSocket) -> io::Result<UdpSocket> {
        Ok(UdpSocket {
            registration: Mutex::new(None),
            imp: Imp {
                inner: FromRawArc::new(Io {
                    read: Overlapped::new(recv_done),
                    write: Overlapped::new(send_done),
                    socket: socket,
                    inner: Mutex::new(Inner {
                        iocp: ReadyBinding::new(),
                        read: State::Empty,
                        write: State::Empty,
                        read_buf: SocketAddrBuf::new(),
                    }),
                }),
            },
        })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.imp.inner.socket.local_addr()
    }

    pub fn try_clone(&self) -> io::Result<UdpSocket> {
        self.imp.inner.socket.try_clone().and_then(UdpSocket::new)
    }

    /// Note that unlike `TcpStream::write` this function will not attempt to
    /// continue writing `buf` until its entirely written.
    ///
    /// TODO: This... may be wrong in the long run. We're reporting that we
    ///       successfully wrote all of the bytes in `buf` but it's possible
    ///       that we don't actually end up writing all of them!
    pub fn send_to(&self, buf: &[u8], target: &SocketAddr)
                   -> io::Result<usize> {
        let mut me = self.inner();
        let me = &mut *me;

        match me.write {
            State::Empty => {}
            _ => return Err(io::ErrorKind::WouldBlock.into()),
        }

        if !me.iocp.registered() {
            return Err(io::ErrorKind::WouldBlock.into())
        }

        let interest = me.iocp.readiness();
        me.iocp.set_readiness(interest - Ready::writable());

        let mut owned_buf = me.iocp.get_buffer(64 * 1024);
        let amt = owned_buf.write(buf)?;
        unsafe {
            trace!("scheduling a send");
            self.imp.inner.socket.send_to_overlapped(&owned_buf, target,
                                                     self.imp.inner.write.as_mut_ptr())
        }?;
        me.write = State::Pending(owned_buf);
        mem::forget(self.imp.clone());
        Ok(amt)
    }

    /// Note that unlike `TcpStream::write` this function will not attempt to
    /// continue writing `buf` until its entirely written.
    ///
    /// TODO: This... may be wrong in the long run. We're reporting that we
    ///       successfully wrote all of the bytes in `buf` but it's possible
    ///       that we don't actually end up writing all of them!
    pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        let mut me = self.inner();
        let me = &mut *me;

        match me.write {
            State::Empty => {}
            _ => return Err(io::ErrorKind::WouldBlock.into()),
        }

        if !me.iocp.registered() {
            return Err(io::ErrorKind::WouldBlock.into())
        }

        let interest = me.iocp.readiness();
        me.iocp.set_readiness(interest - Ready::writable());

        let mut owned_buf = me.iocp.get_buffer(64 * 1024);
        let amt = owned_buf.write(buf)?;
        unsafe {
            trace!("scheduling a send");
            self.imp.inner.socket.send_overlapped(&owned_buf, self.imp.inner.write.as_mut_ptr())

        }?;
        me.write = State::Pending(owned_buf);
        mem::forget(self.imp.clone());
        Ok(amt)
    }

    pub fn recv_from(&self, mut buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        let mut me = self.inner();
        match mem::replace(&mut me.read, State::Empty) {
            State::Empty => Err(io::ErrorKind::WouldBlock.into()),
            State::Pending(b) => { me.read = State::Pending(b); Err(io::ErrorKind::WouldBlock.into()) }
            State::Ready(data) => {
                // If we weren't provided enough space to receive the message
                // then don't actually read any data, just return an error.
                if buf.len() < data.len() {
                    me.read = State::Ready(data);
                    Err(io::Error::from_raw_os_error(WSAEMSGSIZE as i32))
                } else {
                    let r = if let Some(addr) = me.read_buf.to_socket_addr() {
                        buf.write(&data).unwrap();
                        Ok((data.len(), addr))
                    } else {
                        Err(io::Error::new(io::ErrorKind::Other,
                                           "failed to parse socket address"))
                    };
                    me.iocp.put_buffer(data);
                    self.imp.schedule_read_from(&mut me);
                    r
                }
            }
            State::Error(e) => {
                self.imp.schedule_read_from(&mut me);
                Err(e)
            }
        }
    }

    pub fn recv(&self, buf: &mut [u8])
                     -> io::Result<usize> {
        //Since recv_from can be used on connected sockets just call it and drop the address.
        self.recv_from(buf).map(|(size,_)| size)
    }

    pub fn connect(&self, addr: SocketAddr) -> io::Result<()> {
        self.imp.inner.socket.connect(addr)
    }

    pub fn broadcast(&self) -> io::Result<bool> {
        self.imp.inner.socket.broadcast()
    }

    pub fn set_broadcast(&self, on: bool) -> io::Result<()> {
        self.imp.inner.socket.set_broadcast(on)
    }

    pub fn multicast_loop_v4(&self) -> io::Result<bool> {
        self.imp.inner.socket.multicast_loop_v4()
    }

    pub fn set_multicast_loop_v4(&self, on: bool) -> io::Result<()> {
        self.imp.inner.socket.set_multicast_loop_v4(on)
    }

    pub fn multicast_ttl_v4(&self) -> io::Result<u32> {
        self.imp.inner.socket.multicast_ttl_v4()
    }

    pub fn set_multicast_ttl_v4(&self, ttl: u32) -> io::Result<()> {
        self.imp.inner.socket.set_multicast_ttl_v4(ttl)
    }

    pub fn multicast_loop_v6(&self) -> io::Result<bool> {
        self.imp.inner.socket.multicast_loop_v6()
    }

    pub fn set_multicast_loop_v6(&self, on: bool) -> io::Result<()> {
        self.imp.inner.socket.set_multicast_loop_v6(on)
    }

    pub fn ttl(&self) -> io::Result<u32> {
        self.imp.inner.socket.ttl()
    }

    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.imp.inner.socket.set_ttl(ttl)
    }

    pub fn join_multicast_v4(&self,
                             multiaddr: &Ipv4Addr,
                             interface: &Ipv4Addr) -> io::Result<()> {
        self.imp.inner.socket.join_multicast_v4(multiaddr, interface)
    }

    pub fn join_multicast_v6(&self,
                             multiaddr: &Ipv6Addr,
                             interface: u32) -> io::Result<()> {
        self.imp.inner.socket.join_multicast_v6(multiaddr, interface)
    }

    pub fn leave_multicast_v4(&self,
                              multiaddr: &Ipv4Addr,
                              interface: &Ipv4Addr) -> io::Result<()> {
        self.imp.inner.socket.leave_multicast_v4(multiaddr, interface)
    }

    pub fn leave_multicast_v6(&self,
                              multiaddr: &Ipv6Addr,
                              interface: u32) -> io::Result<()> {
        self.imp.inner.socket.leave_multicast_v6(multiaddr, interface)
    }

    pub fn set_only_v6(&self, only_v6: bool) -> io::Result<()> {
        self.imp.inner.socket.set_only_v6(only_v6)
    }

    pub fn only_v6(&self) -> io::Result<bool> {
        self.imp.inner.socket.only_v6()
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.imp.inner.socket.take_error()
    }

    fn inner(&self) -> MutexGuard<Inner> {
        self.imp.inner()
    }

    fn post_register(&self, interest: Ready, me: &mut Inner) {
        if interest.is_readable() {
            //We use recv_from here since it is well specified for both
            //connected and non-connected sockets and we can discard the address
            //when calling recv().
            self.imp.schedule_read_from(me);
        }
        // See comments in TcpSocket::post_register for what's going on here
        if interest.is_writable() {
            if let State::Empty = me.write {
                self.imp.add_readiness(me, Ready::writable());
            }
        }
    }
}

impl Imp {
    fn inner(&self) -> MutexGuard<Inner> {
        self.inner.inner.lock().unwrap()
    }

    fn schedule_read_from(&self, me: &mut Inner) {
        match me.read {
            State::Empty => {}
            _ => return,
        }

        let interest = me.iocp.readiness();
        me.iocp.set_readiness(interest - Ready::readable());

        let mut buf = me.iocp.get_buffer(64 * 1024);
        let res = unsafe {
            trace!("scheduling a read");
            let cap = buf.capacity();
            buf.set_len(cap);
            self.inner.socket.recv_from_overlapped(&mut buf, &mut me.read_buf,
                                                   self.inner.read.as_mut_ptr())
        };
        match res {
            Ok(_) => {
                me.read = State::Pending(buf);
                mem::forget(self.clone());
            }
            Err(e) => {
                me.read = State::Error(e);
                self.add_readiness(me, Ready::readable());
                me.iocp.put_buffer(buf);
            }
        }
    }

    // See comments in tcp::StreamImp::push
    fn add_readiness(&self, me: &Inner, set: Ready) {
        me.iocp.set_readiness(set | me.iocp.readiness());
    }
}

impl Evented for UdpSocket {
    fn register(&self, poll: &Poll, token: Token,
                interest: Ready, opts: PollOpt) -> io::Result<()> {
        let mut me = self.inner();
        me.iocp.register_socket(&self.imp.inner.socket,
                                     poll, token, interest, opts,
                                     &self.registration)?;
        self.post_register(interest, &mut me);
        Ok(())
    }

    fn reregister(&self, poll: &Poll, token: Token,
                  interest: Ready, opts: PollOpt) -> io::Result<()> {
        let mut me = self.inner();
        me.iocp.reregister_socket(&self.imp.inner.socket,
                                       poll, token, interest,
                                       opts, &self.registration)?;
        self.post_register(interest, &mut me);
        Ok(())
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        self.inner().iocp.deregister(&self.imp.inner.socket,
                                     poll, &self.registration)
    }
}

impl fmt::Debug for UdpSocket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("UdpSocket")
            .finish()
    }
}

impl Drop for UdpSocket {
    fn drop(&mut self) {
        let inner = self.inner();

        // If we're still internally reading, we're no longer interested. Note
        // though that we don't cancel any writes which may have been issued to
        // preserve the same semantics as Unix.
        unsafe {
            match inner.read {
                State::Pending(_) => {
                    drop(super::cancel(&self.imp.inner.socket,
                                       &self.imp.inner.read));
                }
                State::Empty |
                State::Ready(_) |
                State::Error(_) => {}
            }
        }
    }
}

fn send_done(status: &OVERLAPPED_ENTRY) {
    let status = CompletionStatus::from_entry(status);
    trace!("finished a send {}", status.bytes_transferred());
    let me2 = Imp {
        inner: unsafe { overlapped2arc!(status.overlapped(), Io, write) },
    };
    let mut me = me2.inner();
    me.write = State::Empty;
    me2.add_readiness(&mut me, Ready::writable());
}

fn recv_done(status: &OVERLAPPED_ENTRY) {
    let status = CompletionStatus::from_entry(status);
    trace!("finished a recv {}", status.bytes_transferred());
    let me2 = Imp {
        inner: unsafe { overlapped2arc!(status.overlapped(), Io, read) },
    };
    let mut me = me2.inner();
    let mut buf = match mem::replace(&mut me.read, State::Empty) {
        State::Pending(buf) => buf,
        _ => unreachable!(),
    };
    unsafe {
        buf.set_len(status.bytes_transferred() as usize);
    }
    me.read = State::Ready(buf);
    me2.add_readiness(&mut me, Ready::readable());
}
