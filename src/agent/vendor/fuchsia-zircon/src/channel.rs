// Copyright 2017 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Type-safe bindings for Zircon channel objects.

use {AsHandleRef, HandleBased, Handle, HandleRef, Peered, Status, Time, usize_into_u32, size_to_u32_sat};
use {sys, ok};
use std::mem;

/// An object representing a Zircon
/// [channel](https://fuchsia.googlesource.com/zircon/+/master/docs/objects/channel.md).
///
/// As essentially a subtype of `Handle`, it can be freely interconverted.
#[derive(Debug, Eq, PartialEq, Hash)]
pub struct Channel(Handle);
impl_handle_based!(Channel);
impl Peered for Channel {}

impl Channel {
    /// Create a channel, resulting an a pair of `Channel` objects representing both
    /// sides of the channel. Messages written into one maybe read from the opposite.
    ///
    /// Wraps the
    /// [zx_channel_create](https://fuchsia.googlesource.com/zircon/+/master/docs/syscalls/channel_create.md)
    /// syscall.
    pub fn create() -> Result<(Channel, Channel), Status> {
        unsafe {
            let mut handle0 = 0;
            let mut handle1 = 0;
            let opts = 0;
            ok(sys::zx_channel_create(opts, &mut handle0, &mut handle1))?;
            Ok((
                Self::from(Handle::from_raw(handle0)),
                Self::from(Handle::from_raw(handle1))
                ))
        }
    }

    /// Read a message from a channel. Wraps the
    /// [zx_channel_read](https://fuchsia.googlesource.com/zircon/+/master/docs/syscalls/channel_read.md)
    /// syscall.
    ///
    /// If the `MessageBuf` lacks the capacity to hold the pending message,
    /// returns an `Err` with the number of bytes and number of handles needed.
    /// Otherwise returns an `Ok` with the result as usual.
    pub fn read_raw(&self, buf: &mut MessageBuf)
        -> Result<Result<(), Status>, (usize, usize)>
    {
        let opts = 0;
        unsafe {
            buf.clear();
            let raw_handle = self.raw_handle();
            let mut num_bytes: u32 = size_to_u32_sat(buf.bytes.capacity());
            let mut num_handles: u32 = size_to_u32_sat(buf.handles.capacity());
            let status = ok(sys::zx_channel_read(raw_handle, opts,
                buf.bytes.as_mut_ptr(), buf.handles.as_mut_ptr() as *mut _,
                num_bytes, num_handles, &mut num_bytes, &mut num_handles));
            if status == Err(Status::BUFFER_TOO_SMALL) {
                Err((num_bytes as usize, num_handles as usize))
            } else {
                Ok(status.map(|()| {
                    buf.bytes.set_len(num_bytes as usize);
                    buf.handles.set_len(num_handles as usize);
                }))
            }
        }
    }

    /// Read a message from a channel.
    ///
    /// Note that this method can cause internal reallocations in the `MessageBuf`
    /// if it is lacks capacity to hold the full message. If such reallocations
    /// are not desirable, use `read_raw` instead.
    pub fn read(&self, buf: &mut MessageBuf) -> Result<(), Status> {
        loop {
            match self.read_raw(buf) {
                Ok(result) => return result,
                Err((num_bytes, num_handles)) => {
                    buf.ensure_capacity_bytes(num_bytes);
                    buf.ensure_capacity_handles(num_handles);
                }
            }
        }
    }

    /// Write a message to a channel. Wraps the
    /// [zx_channel_write](https://fuchsia.googlesource.com/zircon/+/master/docs/syscalls/channel_write.md)
    /// syscall.
    pub fn write(&self, bytes: &[u8], handles: &mut Vec<Handle>)
            -> Result<(), Status>
    {
        let opts = 0;
        let n_bytes = try!(usize_into_u32(bytes.len()).map_err(|_| Status::OUT_OF_RANGE));
        let n_handles = try!(usize_into_u32(handles.len()).map_err(|_| Status::OUT_OF_RANGE));
        unsafe {
            let status = sys::zx_channel_write(self.raw_handle(), opts, bytes.as_ptr(), n_bytes,
                handles.as_ptr() as *const sys::zx_handle_t, n_handles);
            ok(status)?;
            // Handles were successfully transferred, forget them on sender side
            handles.set_len(0);
            Ok(())
        }
    }

    /// Send a message consisting of the given bytes and handles to a channel and await a reply. The
    /// bytes should start with a four byte 'txid' which is used to identify the matching reply.
    ///
    /// Wraps the
    /// [zx_channel_call](https://fuchsia.googlesource.com/zircon/+/master/docs/syscalls/channel_call.md)
    /// syscall.
    ///
    /// Note that unlike [`read`][read], the caller must ensure that the MessageBuf has enough
    /// capacity for the bytes and handles which will be received, as replies which are too large
    /// are discarded.
    ///
    /// On failure returns the both the main and read status.
    ///
    /// [read]: struct.Channel.html#method.read
    pub fn call(&self, timeout: Time, bytes: &[u8], handles: &mut Vec<Handle>,
        buf: &mut MessageBuf) -> Result<(), (Status, Status)>
    {
        let write_num_bytes = try!(usize_into_u32(bytes.len()).map_err(
            |_| (Status::OUT_OF_RANGE, Status::OK)));
        let write_num_handles = try!(usize_into_u32(handles.len()).map_err(
            |_| (Status::OUT_OF_RANGE, Status::OK)));
        buf.clear();
        let read_num_bytes: u32 = size_to_u32_sat(buf.bytes.capacity());
        let read_num_handles: u32 = size_to_u32_sat(buf.handles.capacity());
        let args = sys::zx_channel_call_args_t {
            wr_bytes: bytes.as_ptr(),
            wr_handles: handles.as_ptr() as *const sys::zx_handle_t,
            rd_bytes: buf.bytes.as_mut_ptr(),
            rd_handles: buf.handles.as_mut_ptr() as *mut _,
            wr_num_bytes: write_num_bytes,
            wr_num_handles: write_num_handles,
            rd_num_bytes: read_num_bytes,
            rd_num_handles: read_num_handles,
        };
        let mut actual_read_bytes: u32 = 0;
        let mut actual_read_handles: u32 = 0;
        let mut read_status = Status::OK.into_raw();
        let options = 0;
        let status = unsafe {
            Status::from_raw(
                sys::zx_channel_call(
                    self.raw_handle(), options, timeout.nanos(), &args, &mut actual_read_bytes,
                    &mut actual_read_handles, &mut read_status))
        };

        match status {
            Status::OK |
            Status::TIMED_OUT |
            Status::CALL_FAILED => {
                // Handles were successfully transferred,
                // even if we didn't get a response, so forget
                // them on the sender side.
                unsafe { handles.set_len(0); }
            }
            _ => {}
        }

        unsafe {
            buf.bytes.set_len(actual_read_bytes as usize);
            buf.handles.set_len(actual_read_handles as usize);
        }
        if Status::OK == status {
            Ok(())
        } else {
            Err((status, Status::from_raw(read_status)))
        }
    }
}

#[test]
pub fn test_handle_repr() {
    assert_eq!(::std::mem::size_of::<sys::zx_handle_t>(), 4);
    assert_eq!(::std::mem::size_of::<Handle>(), 4);
    assert_eq!(::std::mem::align_of::<sys::zx_handle_t>(), ::std::mem::align_of::<Handle>());

    // This test asserts that repr(transparent) still works for Handle -> zx_handle_t

    let n: Vec<sys::zx_handle_t> = vec![0, 100, 2<<32-1];
    let v: Vec<Handle> = n.iter().map(|h| unsafe { Handle::from_raw(*h) } ).collect();

    for (handle, raw) in v.iter().zip(n.iter()) {
        unsafe {
            assert_eq!(*(handle as *const _ as *const [u8; 4]), *(raw as *const _ as *const [u8; 4]));
        }
    }

    for h in v.into_iter() {
        ::std::mem::forget(h);
    }
}

impl AsRef<Channel> for Channel {
    fn as_ref(&self) -> &Self {
        &self
    }
}

/// A buffer for _receiving_ messages from a channel.
///
/// A `MessageBuf` is essentially a byte buffer and a vector of
/// handles, but move semantics for "taking" handles requires special handling.
///
/// Note that for sending messages to a channel, the caller manages the buffers,
/// using a plain byte slice and `Vec<Handle>`.
#[derive(Default)]
#[derive(Debug)]
pub struct MessageBuf {
    bytes: Vec<u8>,
    handles: Vec<Handle>,
}

impl MessageBuf {
    /// Create a new, empty, message buffer.
    pub fn new() -> Self {
        Default::default()
    }

    /// Create a new non-empty message buffer.
    pub fn new_with(v: Vec<u8>, h: Vec<Handle>) -> Self {
        Self{
            bytes: v,
            handles: h,
        }
    }

    /// Ensure that the buffer has the capacity to hold at least `n_bytes` bytes.
    pub fn ensure_capacity_bytes(&mut self, n_bytes: usize) {
        ensure_capacity(&mut self.bytes, n_bytes);
    }

    /// Ensure that the buffer has the capacity to hold at least `n_handles` handles.
    pub fn ensure_capacity_handles(&mut self, n_handles: usize) {
        ensure_capacity(&mut self.handles, n_handles);
    }

    /// Ensure that at least n_bytes bytes are initialized (0 fill).
    pub fn ensure_initialized_bytes(&mut self, n_bytes: usize) {
        if n_bytes <= self.bytes.len() {
            return;
        }
        self.bytes.resize(n_bytes, 0);
    }

    /// Get a reference to the bytes of the message buffer, as a `&[u8]` slice.
    pub fn bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }

    /// The number of handles in the message buffer. Note this counts the number
    /// available when the message was received; `take_handle` does not affect
    /// the count.
    pub fn n_handles(&self) -> usize {
        self.handles.len()
    }

    /// Take the handle at the specified index from the message buffer. If the
    /// method is called again with the same index, it will return `None`, as
    /// will happen if the index exceeds the number of handles available.
    pub fn take_handle(&mut self, index: usize) -> Option<Handle> {
        self.handles.get_mut(index).and_then(|handle|
            if handle.is_invalid() {
                None
            } else {
                Some(mem::replace(handle, Handle::invalid()))
            }
        )
    }

    /// Clear the bytes and handles contained in the buf. This will drop any
    /// contained handles, resulting in their resources being freed.
    pub fn clear(&mut self) {
        self.bytes.clear();
        self.handles.clear();
    }
}

fn ensure_capacity<T>(vec: &mut Vec<T>, size: usize) {
    let len = vec.len();
    if size > len {
        vec.reserve(size - len);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use {DurationNum, Rights, Signals, Vmo};
    use std::thread;

    #[test]
    fn channel_basic() {
        let (p1, p2) = Channel::create().unwrap();

        let mut empty = vec![];
        assert!(p1.write(b"hello", &mut empty).is_ok());

        let mut buf = MessageBuf::new();
        assert!(p2.read(&mut buf).is_ok());
        assert_eq!(buf.bytes(), b"hello");
    }

    #[test]
    fn channel_read_raw_too_small() {
        let (p1, p2) = Channel::create().unwrap();

        let mut empty = vec![];
        assert!(p1.write(b"hello", &mut empty).is_ok());

        let mut buf = MessageBuf::new();
        let result = p2.read_raw(&mut buf);
        assert_eq!(result, Err((5, 0)));
        assert_eq!(buf.bytes(), b"");
    }

    #[test]
    fn channel_send_handle() {
        let hello_length: usize = 5;

        // Create a pair of channels and a virtual memory object.
        let (p1, p2) = Channel::create().unwrap();
        let vmo = Vmo::create(hello_length as u64).unwrap();

        // Duplicate VMO handle and send it down the channel.
        let duplicate_vmo_handle = vmo.duplicate_handle(Rights::SAME_RIGHTS).unwrap().into();
        let mut handles_to_send: Vec<Handle> = vec![duplicate_vmo_handle];
        assert!(p1.write(b"", &mut handles_to_send).is_ok());
        // Handle should be removed from vector.
        assert!(handles_to_send.is_empty());

        // Read the handle from the receiving channel.
        let mut buf = MessageBuf::new();
        assert!(p2.read(&mut buf).is_ok());
        assert_eq!(buf.n_handles(), 1);
        // Take the handle from the buffer.
        let received_handle = buf.take_handle(0).unwrap();
        // Should not affect number of handles.
        assert_eq!(buf.n_handles(), 1);
        // Trying to take it again should fail.
        assert!(buf.take_handle(0).is_none());

        // Now to test that we got the right handle, try writing something to it...
        let received_vmo = Vmo::from(received_handle);
        assert_eq!(received_vmo.write(b"hello", 0).unwrap(), hello_length);

        // ... and reading it back from the original VMO.
        let mut read_vec = vec![0; hello_length];
        assert_eq!(vmo.read(&mut read_vec, 0).unwrap(), hello_length);
        assert_eq!(read_vec, b"hello");
    }

    #[test]
    fn channel_call_timeout() {
        let ten_ms = 10.millis();

        // Create a pair of channels and a virtual memory object.
        let (p1, p2) = Channel::create().unwrap();
        let vmo = Vmo::create(0 as u64).unwrap();

        // Duplicate VMO handle and send it along with the call.
        let duplicate_vmo_handle = vmo.duplicate_handle(Rights::SAME_RIGHTS).unwrap().into();
        let mut handles_to_send: Vec<Handle> = vec![duplicate_vmo_handle];
        let mut buf = MessageBuf::new();
        assert_eq!(p1.call(ten_ms.after_now(), b"call", &mut handles_to_send, &mut buf),
            Err((Status::TIMED_OUT, Status::OK)));
        // Handle should be removed from vector even though we didn't get a response, as it was
        // still sent over the channel.
        assert!(handles_to_send.is_empty());

        // Should be able to read call even though it timed out waiting for a response.
        let mut buf = MessageBuf::new();
        assert!(p2.read(&mut buf).is_ok());
        assert_eq!(buf.bytes(), b"call");
        assert_eq!(buf.n_handles(), 1);
    }

    #[test]
    fn channel_call() {
        // Create a pair of channels
        let (p1, p2) = Channel::create().unwrap();

        // create an mpsc channel for communicating the call data for later assertion
        let (tx, rx) = ::std::sync::mpsc::channel();

        // Start a new thread to respond to the call.
        thread::spawn(move || {
            let mut buf = MessageBuf::new();
            // if either the read or the write fail, this thread will panic,
            // resulting in tx being dropped, which will be noticed by the rx.
            p2.wait_handle(Signals::CHANNEL_READABLE, 1.seconds().after_now()).expect("callee wait error");
            p2.read(&mut buf).expect("callee read error");
            p2.write(b"txidresponse", &mut vec![]).expect("callee write error");
            tx.send(buf).expect("callee mpsc send error");
        });

        // Make the call.
        let mut buf = MessageBuf::new();
        buf.ensure_capacity_bytes(12);
        // NOTE(raggi): CQ has been seeing some long stalls from channel call,
        // and it's as yet unclear why. The timeout here has been made much
        // larger in order to avoid that, as the issues are not issues with this
        // crate's concerns. The timeout is here just to prevent the tests from
        // stalling forever if a developer makes a mistake locally in this
        // crate. Tests of Zircon behavior or virtualization behavior should be
        // covered elsewhere. See ZX-1324.
        p1.call(30.seconds().after_now(), b"txidcall", &mut vec![], &mut buf).expect("channel call error");
        assert_eq!(buf.bytes(), b"txidresponse");
        assert_eq!(buf.n_handles(), 0);

        let sbuf = rx.recv().expect("mpsc channel recv error");
        assert_eq!(sbuf.bytes(), b"txidcall");
        assert_eq!(sbuf.n_handles(), 0);
    }
}
