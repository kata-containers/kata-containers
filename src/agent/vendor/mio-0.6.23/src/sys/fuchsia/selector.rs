use {io, Event, PollOpt, Ready, Token};
use sys::fuchsia::{
    assert_fuchsia_ready_repr,
    epoll_event_to_ready,
    poll_opts_to_wait_async,
    EventedFd,
    EventedFdInner,
    FuchsiaReady,
};
use zircon;
use zircon::AsHandleRef;
use zircon_sys::zx_handle_t;
use std::collections::hash_map;
use std::fmt;
use std::mem;
use std::sync::atomic::{AtomicBool, AtomicUsize, ATOMIC_USIZE_INIT, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;
use sys;

/// The kind of registration-- file descriptor or handle.
///
/// The last bit of a token is set to indicate the type of the registration.
#[derive(Copy, Clone, Eq, PartialEq)]
enum RegType {
    Fd,
    Handle,
}

fn key_from_token_and_type(token: Token, reg_type: RegType) -> io::Result<u64> {
    let key = token.0 as u64;
    let msb = 1u64 << 63;
    if (key & msb) != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Most-significant bit of token must remain unset."));
    }

    Ok(match reg_type {
        RegType::Fd => key,
        RegType::Handle => key | msb,
    })
}

fn token_and_type_from_key(key: u64) -> (Token, RegType) {
    let msb = 1u64 << 63;
    (
        Token((key & !msb) as usize),
        if (key & msb) == 0 {
            RegType::Fd
        } else {
            RegType::Handle
        }
    )
}

/// Each Selector has a globally unique(ish) ID associated with it. This ID
/// gets tracked by `TcpStream`, `TcpListener`, etc... when they are first
/// registered with the `Selector`. If a type that is previously associated with
/// a `Selector` attempts to register itself with a different `Selector`, the
/// operation will return with an error. This matches windows behavior.
static NEXT_ID: AtomicUsize = ATOMIC_USIZE_INIT;

pub struct Selector {
    id: usize,

    /// Zircon object on which the handles have been registered, and on which events occur
    port: Arc<zircon::Port>,

    /// Whether or not `tokens_to_rereg` contains any elements. This is a best-effort attempt
    /// used to prevent having to lock `tokens_to_rereg` when it is empty.
    has_tokens_to_rereg: AtomicBool,

    /// List of `Token`s corresponding to registrations that need to be reregistered before the
    /// next `port::wait`. This is necessary to provide level-triggered behavior for
    /// `Async::repeating` registrations.
    ///
    /// When a level-triggered `Async::repeating` event is seen, its token is added to this list so
    /// that it will be reregistered before the next `port::wait` call, making `port::wait` return
    /// immediately if the signal was high during the reregistration.
    ///
    /// Note: when used at the same time, the `tokens_to_rereg` lock should be taken out _before_
    /// `token_to_fd`.
    tokens_to_rereg: Mutex<Vec<Token>>,

    /// Map from tokens to weak references to `EventedFdInner`-- a structure describing a
    /// file handle, its associated `fdio` object, and its current registration.
    token_to_fd: Mutex<hash_map::HashMap<Token, Weak<EventedFdInner>>>,
}

impl Selector {
    pub fn new() -> io::Result<Selector> {
        // Assertion from fuchsia/ready.rs to make sure that FuchsiaReady's representation is
        // compatible with Ready.
        assert_fuchsia_ready_repr();

        let port = Arc::new(
            zircon::Port::create(zircon::PortOpts::Default)?
        );

        // offset by 1 to avoid choosing 0 as the id of a selector
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed) + 1;

        let has_tokens_to_rereg = AtomicBool::new(false);
        let tokens_to_rereg = Mutex::new(Vec::new());
        let token_to_fd = Mutex::new(hash_map::HashMap::new());

        Ok(Selector {
            id: id,
            port: port,
            has_tokens_to_rereg: has_tokens_to_rereg,
            tokens_to_rereg: tokens_to_rereg,
            token_to_fd: token_to_fd,
        })
    }

    pub fn id(&self) -> usize {
        self.id
    }

    /// Returns a reference to the underlying port `Arc`.
    pub fn port(&self) -> &Arc<zircon::Port> { &self.port }

    /// Reregisters all registrations pointed to by the `tokens_to_rereg` list
    /// if `has_tokens_to_rereg`.
    fn reregister_handles(&self) -> io::Result<()> {
        // We use `Ordering::Acquire` to make sure that we see all `tokens_to_rereg`
        // written before the store using `Ordering::Release`.
        if self.has_tokens_to_rereg.load(Ordering::Acquire) {
            let mut tokens = self.tokens_to_rereg.lock().unwrap();
            let token_to_fd = self.token_to_fd.lock().unwrap();
            for token in tokens.drain(0..) {
                if let Some(eventedfd) = token_to_fd.get(&token)
                    .and_then(|h| h.upgrade()) {
                    eventedfd.rereg_for_level(&self.port);
                }
            }
            self.has_tokens_to_rereg.store(false, Ordering::Release);
        }
        Ok(())
    }

    pub fn select(&self,
                  evts: &mut Events,
                  _awakener: Token,
                  timeout: Option<Duration>) -> io::Result<bool>
    {
        evts.clear();

        self.reregister_handles()?;

        let deadline = match timeout {
            Some(duration) => {
                let nanos = duration.as_secs().saturating_mul(1_000_000_000)
                                .saturating_add(duration.subsec_nanos() as u64);

                zircon::deadline_after(nanos)
            }
            None => zircon::ZX_TIME_INFINITE,
        };

        let packet = match self.port.wait(deadline) {
            Ok(packet) => packet,
            Err(zircon::Status::ErrTimedOut) => return Ok(false),
            Err(e) => Err(e)?,
        };

        let observed_signals = match packet.contents() {
            zircon::PacketContents::SignalOne(signal_packet) => {
                signal_packet.observed()
            }
            zircon::PacketContents::SignalRep(signal_packet) => {
                signal_packet.observed()
            }
            zircon::PacketContents::User(_user_packet) => {
                // User packets are only ever sent by an Awakener
                return Ok(true);
            }
        };

        let key = packet.key();
        let (token, reg_type) = token_and_type_from_key(key);

        match reg_type {
            RegType::Handle => {
                // We can return immediately-- no lookup or registration necessary.
                evts.events.push(Event::new(Ready::from(observed_signals), token));
                Ok(false)
            },
            RegType::Fd => {
                // Convert the signals to epoll events using __fdio_wait_end,
                // and add to reregistration list if necessary.
                let events: u32;
                {
                    let handle = if let Some(handle) =
                    self.token_to_fd.lock().unwrap()
                        .get(&token)
                        .and_then(|h| h.upgrade()) {
                        handle
                    } else {
                        // This handle is apparently in the process of removal.
                        // It has been removed from the list, but port_cancel has not been called.
                        return Ok(false);
                    };

                    events = unsafe {
                        let mut events: u32 = mem::uninitialized();
                        sys::fuchsia::sys::__fdio_wait_end(handle.fdio(), observed_signals, &mut events);
                        events
                    };

                    // If necessary, queue to be reregistered before next port_await
                    let needs_to_rereg = {
                        let registration_lock = handle.registration().lock().unwrap();

                        registration_lock
                            .as_ref()
                            .and_then(|r| r.rereg_signals())
                            .is_some()
                    };

                    if needs_to_rereg {
                        let mut tokens_to_rereg_lock = self.tokens_to_rereg.lock().unwrap();
                        tokens_to_rereg_lock.push(token);
                        // We use `Ordering::Release` to make sure that we see all `tokens_to_rereg`
                        // written before the store.
                        self.has_tokens_to_rereg.store(true, Ordering::Release);
                    }
                }

                evts.events.push(Event::new(epoll_event_to_ready(events), token));
                Ok(false)
            },
        }
    }

    /// Register event interests for the given IO handle with the OS
    pub fn register_fd(&self,
                       handle: &zircon::Handle,
                       fd: &EventedFd,
                       token: Token,
                       signals: zircon::Signals,
                       poll_opts: PollOpt) -> io::Result<()>
    {
        {
            let mut token_to_fd = self.token_to_fd.lock().unwrap();
            match token_to_fd.entry(token) {
                hash_map::Entry::Occupied(_) =>
                    return Err(io::Error::new(io::ErrorKind::AlreadyExists,
                               "Attempted to register a filedescriptor on an existing token.")),
                hash_map::Entry::Vacant(slot) => slot.insert(Arc::downgrade(&fd.inner)),
            };
        }

        let wait_async_opts = poll_opts_to_wait_async(poll_opts);

        let wait_res = handle.wait_async_handle(&self.port, token.0 as u64, signals, wait_async_opts);

        if wait_res.is_err() {
            self.token_to_fd.lock().unwrap().remove(&token);
        }

        Ok(wait_res?)
    }

    /// Deregister event interests for the given IO handle with the OS
    pub fn deregister_fd(&self, handle: &zircon::Handle, token: Token) -> io::Result<()> {
        self.token_to_fd.lock().unwrap().remove(&token);

        // We ignore NotFound errors since oneshots are automatically deregistered,
        // but mio will attempt to deregister them manually.
        self.port.cancel(&*handle, token.0 as u64)
            .map_err(io::Error::from)
            .or_else(|e| if e.kind() == io::ErrorKind::NotFound {
                Ok(())
            } else {
                Err(e)
            })
    }

    pub fn register_handle(&self,
                           handle: zx_handle_t,
                           token: Token,
                           interests: Ready,
                           poll_opts: PollOpt) -> io::Result<()>
    {
        if poll_opts.is_level() && !poll_opts.is_oneshot() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput,
                      "Repeated level-triggered events are not supported on Fuchsia handles."));
        }

        let temp_handle = unsafe { zircon::Handle::from_raw(handle) };

        let res = temp_handle.wait_async_handle(
                    &self.port,
                    key_from_token_and_type(token, RegType::Handle)?,
                    FuchsiaReady::from(interests).into_zx_signals(),
                    poll_opts_to_wait_async(poll_opts));

        mem::forget(temp_handle);

        Ok(res?)
    }


    pub fn deregister_handle(&self, handle: zx_handle_t, token: Token) -> io::Result<()>
    {
        let temp_handle = unsafe { zircon::Handle::from_raw(handle) };
        let res = self.port.cancel(&temp_handle, key_from_token_and_type(token, RegType::Handle)?);

        mem::forget(temp_handle);

        Ok(res?)
    }
}

pub struct Events {
    events: Vec<Event>
}

impl Events {
    pub fn with_capacity(_u: usize) -> Events {
        // The Fuchsia selector only handles one event at a time,
        // so we ignore the default capacity and set it to one.
        Events { events: Vec::with_capacity(1) }
    }
    pub fn len(&self) -> usize {
        self.events.len()
    }
    pub fn capacity(&self) -> usize {
        self.events.capacity()
    }
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
    pub fn get(&self, idx: usize) -> Option<Event> {
        self.events.get(idx).map(|e| *e)
    }
    pub fn push_event(&mut self, event: Event) {
        self.events.push(event)
    }
    pub fn clear(&mut self) {
        self.events.events.drain(0..);
    }
}

impl fmt::Debug for Events {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Events")
            .field("len", &self.len())
            .finish()
    }
}
