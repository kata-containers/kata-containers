use std::io;

use super::*;

/// Changes the cookie type without introducing any buffering.
///
/// If you have a `b: BufferedReader<B>` but need a `c:
/// BufferedReader<C>`, then one way to do that is to use `let c =
/// Generic::with_cookie(b, _)`, but that introduces buffering.  This
/// `Adapter` also changes cookie types, but does no buffering of its
/// own.
#[derive(Debug)]
pub struct Adapter<T: BufferedReader<B>, B: fmt::Debug + Send + Sync, C: fmt::Debug + Sync + Send> {
    _ghostly_cookie: std::marker::PhantomData<B>,
    cookie: C,
    reader: T,
}

assert_send_and_sync!(Adapter<T, B, C>
                      where T: BufferedReader<B>,
                            B: fmt::Debug,
                            C: fmt::Debug);

impl<T: BufferedReader<B>, B: fmt::Debug + Send + Sync, C: fmt::Debug + Sync + Send> fmt::Display for Adapter<T, B, C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Adapter").finish()
    }
}

impl<T: BufferedReader<B>, B: fmt::Debug + Sync + Send> Adapter<T, B, ()> {
    /// Instantiates a new adapter.
    ///
    /// `reader` is the source to wrap.
    pub fn new(reader: T) -> Self {
        Self::with_cookie(reader, ())
    }
}

impl<T: BufferedReader<B>, B: fmt::Debug + Send + Sync, C: fmt::Debug + Sync + Send> Adapter<T, B, C> {
    /// Like `new()`, but sets a cookie.
    ///
    /// The cookie can be retrieved using the `cookie_ref` and
    /// `cookie_mut` methods, and set using the `cookie_set` method.
    pub fn with_cookie(reader: T, cookie: C)
            -> Adapter<T, B, C> {
        Adapter {
            reader,
            _ghostly_cookie: Default::default(),
            cookie,
        }
    }
}

impl<T: BufferedReader<B>, B: fmt::Debug + Send + Sync, C: fmt::Debug + Sync + Send> io::Read for Adapter<T, B, C> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        self.reader.read(buf)
    }
}

impl<T: BufferedReader<B>, B: fmt::Debug + Send + Sync, C: fmt::Debug + Sync + Send> BufferedReader<C> for Adapter<T, B, C> {
    fn buffer(&self) -> &[u8] {
        self.reader.buffer()
    }

    fn data(&mut self, amount: usize) -> Result<&[u8], io::Error> {
        self.reader.data(amount)
    }

    fn consume(&mut self, amount: usize) -> &[u8] {
        self.reader.consume(amount)
    }

    fn data_consume(&mut self, amount: usize) -> Result<&[u8], io::Error> {
        self.reader.data_consume(amount)
    }

    fn data_consume_hard(&mut self, amount: usize) -> Result<&[u8], io::Error> {
        self.reader.data_consume_hard(amount)
    }

    fn consummated(&mut self) -> bool {
        self.reader.consummated()
    }

    fn get_mut(&mut self) -> Option<&mut dyn BufferedReader<C>> {
        None
    }

    fn get_ref(&self) -> Option<&dyn BufferedReader<C>> {
        None
    }

    fn into_inner<'b>(self: Box<Self>) -> Option<Box<dyn BufferedReader<C> + 'b>>
        where Self: 'b {
        None
    }

    fn cookie_set(&mut self, cookie: C) -> C {
        std::mem::replace(&mut self.cookie, cookie)
    }

    fn cookie_ref(&self) -> &C {
        &self.cookie
    }

    fn cookie_mut(&mut self) -> &mut C {
        &mut self.cookie
    }
}
