//! Implementation of mio for Windows using IOCP
//!
//! This module uses I/O Completion Ports (IOCP) on Windows to implement mio's
//! Unix epoll-like interface. Unfortunately these two I/O models are
//! fundamentally incompatible:
//!
//! * IOCP is a completion-based model where work is submitted to the kernel and
//!   a program is notified later when the work finished.
//! * epoll is a readiness-based model where the kernel is queried as to what
//!   work can be done, and afterwards the work is done.
//!
//! As a result, this implementation for Windows is much less "low level" than
//! the Unix implementation of mio. This design decision was intentional,
//! however.
//!
//! ## What is IOCP?
//!
//! The [official docs][docs] have a comprehensive explanation of what IOCP is,
//! but at a high level it requires the following operations to be executed to
//! perform some I/O:
//!
//! 1. A completion port is created
//! 2. An I/O handle and a token is registered with this completion port
//! 3. Some I/O is issued on the handle. This generally means that an API was
//!    invoked with a zeroed `OVERLAPPED` structure. The API will immediately
//!    return.
//! 4. After some time, the application queries the I/O port for completed
//!    events. The port will returned a pointer to the `OVERLAPPED` along with
//!    the token presented at registration time.
//!
//! Many I/O operations can be fired off before waiting on a port, and the port
//! will block execution of the calling thread until an I/O event has completed
//! (or a timeout has elapsed).
//!
//! Currently all of these low-level operations are housed in a separate `miow`
//! crate to provide a 0-cost abstraction over IOCP. This crate uses that to
//! implement all fiddly bits so there's very few actual Windows API calls or
//! `unsafe` blocks as a result.
//!
//! [docs]: https://msdn.microsoft.com/en-us/library/windows/desktop/aa365198%28v=vs.85%29.aspx
//!
//! ## Safety of IOCP
//!
//! Unfortunately for us, IOCP is pretty unsafe in terms of Rust lifetimes and
//! such. When an I/O operation is submitted to the kernel, it involves handing
//! the kernel a few pointers like a buffer to read/write, an `OVERLAPPED`
//! structure pointer, and perhaps some other buffers such as for socket
//! addresses. These pointers all have to remain valid **for the entire I/O
//! operation's duration**.
//!
//! There's no way to define a safe lifetime for these pointers/buffers over
//! the span of an I/O operation, so we're forced to add a layer of abstraction
//! (not 0-cost) to make these APIs safe. Currently this implementation
//! basically just boxes everything up on the heap to give it a stable address
//! and then keys off that most of the time.
//!
//! ## From completion to readiness
//!
//! Translating a completion-based model to a readiness-based model is also no
//! easy task, and a significant portion of this implementation is managing this
//! translation. The basic idea behind this implementation is to issue I/O
//! operations preemptively and then translate their completions to a "I'm
//! ready" event.
//!
//! For example, in the case of reading a `TcpSocket`, as soon as a socket is
//! connected (or registered after an accept) a read operation is executed.
//! While the read is in progress calls to `read` will return `WouldBlock`, and
//! once the read is completed we translate the completion notification into a
//! `readable` event. Once the internal buffer is drained (e.g. all data from it
//! has been read) a read operation is re-issued.
//!
//! Write operations are a little different from reads, and the current
//! implementation is to just schedule a write as soon as `write` is first
//! called. While that write operation is in progress all future calls to
//! `write` will return `WouldBlock`. Completion of the write then translates to
//! a `writable` event. Note that this will probably want to add some layer of
//! internal buffering in the future.
//!
//! ## Buffer Management
//!
//! As there's lots of I/O operations in flight at any one point in time,
//! there's lots of live buffers that need to be juggled around (e.g. this
//! implementation's own internal buffers).
//!
//! Currently all buffers are created for the I/O operation at hand and are then
//! discarded when it completes (this is listed as future work below).
//!
//! ## Callback Management
//!
//! When the main event loop receives a notification that an I/O operation has
//! completed, some work needs to be done to translate that to a set of events
//! or perhaps some more I/O needs to be scheduled. For example after a
//! `TcpStream` is connected it generates a writable event and also schedules a
//! read.
//!
//! To manage all this the `Selector` uses the `OVERLAPPED` pointer from the
//! completion status. The selector assumes that all `OVERLAPPED` pointers are
//! actually pointers to the interior of a `selector::Overlapped` which means
//! that right after the `OVERLAPPED` itself there's a function pointer. This
//! function pointer is given the completion status as well as another callback
//! to push events onto the selector.
//!
//! The callback for each I/O operation doesn't have any environment, so it
//! relies on memory layout and unsafe casting to translate an `OVERLAPPED`
//! pointer (or in this case a `selector::Overlapped` pointer) to a type of
//! `FromRawArc<T>` (see module docs for why this type exists).
//!
//! ## Thread Safety
//!
//! Currently all of the I/O primitives make liberal use of `Arc` and `Mutex`
//! as an implementation detail. The main reason for this is to ensure that the
//! types are `Send` and `Sync`, but the implementations have not been stressed
//! in multithreaded situations yet. As a result, there are bound to be
//! functional surprises in using these concurrently.
//!
//! ## Future Work
//!
//! First up, let's take a look at unimplemented portions of this module:
//!
//! * The `PollOpt::level()` option is currently entirely unimplemented.
//! * Each `EventLoop` currently owns its completion port, but this prevents an
//!   I/O handle from being added to multiple event loops (something that can be
//!   done on Unix). Additionally, it hinders event loops moving across threads.
//!   This should be solved by likely having a global `Selector` which all
//!   others then communicate with.
//! * Although Unix sockets don't exist on Windows, there are named pipes and
//!   those should likely be bound here in a similar fashion to `TcpStream`.
//!
//! Next up, there are a few performance improvements and optimizations that can
//! still be implemented
//!
//! * Buffer management right now is pretty bad, they're all just allocated
//!   right before an I/O operation and discarded right after. There should at
//!   least be some form of buffering buffers.
//! * No calls to `write` are internally buffered before being scheduled, which
//!   means that writing performance is abysmal compared to Unix. There should
//!   be some level of buffering of writes probably.

use std::io;
use std::os::windows::prelude::*;

use kernel32;
use winapi;

mod awakener;
#[macro_use]
mod selector;
mod tcp;
mod udp;
mod from_raw_arc;
mod buffer_pool;

pub use self::awakener::Awakener;
pub use self::selector::{Events, Selector, Overlapped, Binding};
pub use self::tcp::{TcpStream, TcpListener};
pub use self::udp::UdpSocket;

#[derive(Copy, Clone)]
enum Family {
    V4, V6,
}

unsafe fn cancel(socket: &AsRawSocket,
                 overlapped: &Overlapped) -> io::Result<()> {
    let handle = socket.as_raw_socket() as winapi::HANDLE;
    let ret = kernel32::CancelIoEx(handle, overlapped.as_mut_ptr());
    if ret == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

unsafe fn no_notify_on_instant_completion(handle: winapi::HANDLE) -> io::Result<()> {
    // TODO: move those to winapi
    const FILE_SKIP_COMPLETION_PORT_ON_SUCCESS: winapi::UCHAR = 1;
    const FILE_SKIP_SET_EVENT_ON_HANDLE: winapi::UCHAR = 2;

    let flags = FILE_SKIP_COMPLETION_PORT_ON_SUCCESS | FILE_SKIP_SET_EVENT_ON_HANDLE;

    let r = kernel32::SetFileCompletionNotificationModes(handle, flags);
    if r == winapi::TRUE {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
