use std::sync::Mutex;

use miow::iocp::CompletionStatus;
use {io, poll, Ready, Poll, PollOpt, Token};
use event::Evented;
use sys::windows::Selector;

pub struct Awakener {
    inner: Mutex<Option<AwakenerInner>>,
}

struct AwakenerInner {
    token: Token,
    selector: Selector,
}

impl Awakener {
    pub fn new() -> io::Result<Awakener> {
        Ok(Awakener {
            inner: Mutex::new(None),
        })
    }

    pub fn wakeup(&self) -> io::Result<()> {
        // Each wakeup notification has NULL as its `OVERLAPPED` pointer to
        // indicate that it's from this awakener and not part of an I/O
        // operation. This is specially recognized by the selector.
        //
        // If we haven't been registered with an event loop yet just silently
        // succeed.
        if let Some(inner) = self.inner.lock().unwrap().as_ref() {
            let status = CompletionStatus::new(0,
                                               usize::from(inner.token),
                                               0 as *mut _);
            inner.selector.port().post(status)?;
        }
        Ok(())
    }

    pub fn cleanup(&self) {
        // noop
    }
}

impl Evented for Awakener {
    fn register(&self, poll: &Poll, token: Token, events: Ready,
                opts: PollOpt) -> io::Result<()> {
        assert_eq!(opts, PollOpt::edge());
        assert_eq!(events, Ready::readable());
        *self.inner.lock().unwrap() = Some(AwakenerInner {
            selector: poll::selector(poll).clone_ref(),
            token: token,
        });
        Ok(())
    }

    fn reregister(&self, poll: &Poll, token: Token, events: Ready,
                  opts: PollOpt) -> io::Result<()> {
        self.register(poll, token, events, opts)
    }

    fn deregister(&self, _poll: &Poll) -> io::Result<()> {
        *self.inner.lock().unwrap() = None;
        Ok(())
    }
}
