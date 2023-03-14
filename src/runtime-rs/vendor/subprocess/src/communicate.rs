use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{self, ErrorKind};
use std::time::{Duration, Instant};

#[cfg(unix)]
mod raw {
    use crate::posix;
    use std::cmp::min;
    use std::fs::File;
    use std::io::{self, Read, Write};
    use std::time::{Duration, Instant};

    fn as_pollfd<'a>(f: Option<&'a File>, for_read: bool) -> posix::PollFd<'a> {
        let events = if for_read {
            posix::POLLIN
        } else {
            posix::POLLOUT
        };
        posix::PollFd::new(f, events)
    }

    fn maybe_poll(
        fin: Option<&File>,
        fout: Option<&File>,
        ferr: Option<&File>,
        deadline: Option<Instant>,
    ) -> io::Result<(bool, bool, bool)> {
        // Polling is needed to prevent deadlock when interacting with
        // multiple streams, and for timeout.  If we're interacting with a
        // single stream without timeout, we can skip the actual poll()
        // syscall and just tell the caller to go ahead with reading/writing.
        if deadline.is_none() {
            match (&fin, &fout, &ferr) {
                (None, None, Some(..)) => return Ok((false, false, true)),
                (None, Some(..), None) => return Ok((false, true, false)),
                (Some(..), None, None) => return Ok((true, false, false)),
                _ => (),
            }
        }

        let timeout = deadline.map(|deadline| {
            let now = Instant::now();
            if now >= deadline {
                Duration::from_secs(0)
            } else {
                deadline - now
            }
        });

        let mut fds = [
            as_pollfd(fin, false),
            as_pollfd(fout, true),
            as_pollfd(ferr, true),
        ];
        posix::poll(&mut fds, timeout)?;

        Ok((
            fds[0].test(posix::POLLOUT | posix::POLLHUP),
            fds[1].test(posix::POLLIN | posix::POLLHUP),
            fds[2].test(posix::POLLIN | posix::POLLHUP),
        ))
    }

    #[derive(Debug)]
    pub struct RawCommunicator {
        stdin: Option<File>,
        stdout: Option<File>,
        stderr: Option<File>,
        input_data: Vec<u8>,
        input_pos: usize,
    }

    impl RawCommunicator {
        pub fn new(
            stdin: Option<File>,
            stdout: Option<File>,
            stderr: Option<File>,
            input_data: Option<Vec<u8>>,
        ) -> RawCommunicator {
            let input_data = input_data.unwrap_or_else(Vec::new);
            RawCommunicator {
                stdin,
                stdout,
                stderr,
                input_data,
                input_pos: 0,
            }
        }

        fn do_read(
            source_ref: &mut Option<&File>,
            dest: &mut Vec<u8>,
            size_limit: Option<usize>,
            total_read: usize,
        ) -> io::Result<()> {
            let mut buf = &mut [0u8; 4096][..];
            if let Some(size_limit) = size_limit {
                if total_read >= size_limit {
                    return Ok(());
                }
                if size_limit - total_read < buf.len() {
                    buf = &mut buf[0..size_limit - total_read];
                }
            }
            let n = source_ref.unwrap().read(buf)?;
            if n != 0 {
                dest.extend_from_slice(&buf[..n]);
            } else {
                *source_ref = None;
            }
            Ok(())
        }

        fn read_into(
            &mut self,
            deadline: Option<Instant>,
            size_limit: Option<usize>,
            outvec: &mut Vec<u8>,
            errvec: &mut Vec<u8>,
        ) -> io::Result<()> {
            // Note: chunk size for writing must be smaller than the pipe buffer
            // size.  A large enough write to a pipe deadlocks despite polling.
            const WRITE_SIZE: usize = 4096;

            let mut stdout_ref = self.stdout.as_ref();
            let mut stderr_ref = self.stderr.as_ref();

            loop {
                if let Some(size_limit) = size_limit {
                    if outvec.len() + errvec.len() >= size_limit {
                        break;
                    }
                }

                if let (None, None, None) = (self.stdin.as_ref(), stdout_ref, stderr_ref) {
                    // When no stream remains, we are done.
                    break;
                }

                let (in_ready, out_ready, err_ready) =
                    maybe_poll(self.stdin.as_ref(), stdout_ref, stderr_ref, deadline)?;
                if !in_ready && !out_ready && !err_ready {
                    return Err(io::Error::new(io::ErrorKind::TimedOut, "timeout"));
                }
                if in_ready {
                    let input = &self.input_data[self.input_pos..];
                    let chunk = &input[..min(WRITE_SIZE, input.len())];
                    let n = self.stdin.as_ref().unwrap().write(chunk)?;
                    self.input_pos += n;
                    if self.input_pos == self.input_data.len() {
                        // close stdin when done writing, so the child receives EOF
                        self.stdin.take();
                        // deallocate the input data, we don't need it any more
                        self.input_data = Vec::new();
                    }
                }
                if out_ready {
                    RawCommunicator::do_read(
                        &mut stdout_ref,
                        outvec,
                        size_limit,
                        outvec.len() + errvec.len(),
                    )?;
                }
                if err_ready {
                    RawCommunicator::do_read(
                        &mut stderr_ref,
                        errvec,
                        size_limit,
                        outvec.len() + errvec.len(),
                    )?;
                }
            }

            Ok(())
        }

        pub fn read(
            &mut self,
            deadline: Option<Instant>,
            size_limit: Option<usize>,
        ) -> (Option<io::Error>, (Option<Vec<u8>>, Option<Vec<u8>>)) {
            let mut outvec = vec![];
            let mut errvec = vec![];

            let err = self
                .read_into(deadline, size_limit, &mut outvec, &mut errvec)
                .err();
            let output = (
                self.stdout.as_ref().map(|_| outvec),
                self.stderr.as_ref().map(|_| errvec),
            );
            (err, output)
        }
    }
}

#[cfg(windows)]
mod raw {
    use std::fs::File;
    use std::io::{self, Read, Write};
    use std::sync::mpsc::{self, RecvTimeoutError, SyncSender};
    use std::thread;
    use std::time::Instant;

    #[derive(Debug, Copy, Clone)]
    enum StreamIdent {
        In = 1 << 0,
        Out = 1 << 1,
        Err = 1 << 2,
    }

    enum Payload {
        Data(Vec<u8>),
        EOF,
        Err(io::Error),
    }

    // Messages exchanged between RawCommunicator's helper threads.
    type Message = (StreamIdent, Payload);

    fn read_and_transmit(mut outfile: File, ident: StreamIdent, sink: SyncSender<Message>) {
        let mut chunk = [0u8; 4096];
        // Note: failing to send to the sink means we're done.  Sending will
        // fail if the main thread drops the RawCommunicator (and with it the
        // receiver) prematurely e.g. because a limit was reached or another
        // helper encountered an IO error.
        loop {
            match outfile.read(&mut chunk) {
                Ok(0) => {
                    let _ = sink.send((ident, Payload::EOF));
                    break;
                }
                Ok(nread) => {
                    if let Err(_) = sink.send((ident, Payload::Data(chunk[..nread].to_vec()))) {
                        break;
                    }
                }
                Err(e) => {
                    let _ = sink.send((ident, Payload::Err(e)));
                    break;
                }
            }
        }
    }

    fn spawn_with_arg<T: Send + 'static>(f: impl FnOnce(T) + Send + 'static, arg: T) {
        thread::spawn(move || f(arg));
    }

    #[derive(Debug)]
    pub struct RawCommunicator {
        rx: mpsc::Receiver<Message>,
        helper_set: u8,
        requested_streams: u8,
        leftover: Option<(StreamIdent, Vec<u8>)>,
    }

    struct Timeout;

    impl RawCommunicator {
        pub fn new(
            stdin: Option<File>,
            stdout: Option<File>,
            stderr: Option<File>,
            input_data: Option<Vec<u8>>,
        ) -> RawCommunicator {
            let mut helper_set = 0u8;
            let mut requested_streams = 0u8;

            let read_stdout = stdout.map(|stdout| {
                helper_set |= StreamIdent::Out as u8;
                requested_streams |= StreamIdent::Out as u8;
                |tx| read_and_transmit(stdout, StreamIdent::Out, tx)
            });
            let read_stderr = stderr.map(|stderr| {
                helper_set |= StreamIdent::Err as u8;
                requested_streams |= StreamIdent::Err as u8;
                |tx| read_and_transmit(stderr, StreamIdent::Err, tx)
            });
            let write_stdin = stdin.map(|mut stdin| {
                let input_data = input_data.expect("must provide input to redirected stdin");
                helper_set |= StreamIdent::In as u8;
                move |tx: SyncSender<_>| match stdin.write_all(&input_data) {
                    Ok(()) => drop(tx.send((StreamIdent::In, Payload::EOF))),
                    Err(e) => drop(tx.send((StreamIdent::In, Payload::Err(e)))),
                }
            });

            let (tx, rx) = mpsc::sync_channel(0);

            read_stdout.map(|f| spawn_with_arg(f, tx.clone()));
            read_stderr.map(|f| spawn_with_arg(f, tx.clone()));
            write_stdin.map(|f| spawn_with_arg(f, tx.clone()));

            RawCommunicator {
                rx,
                helper_set,
                requested_streams,
                leftover: None,
            }
        }

        fn recv_until(&self, deadline: Option<Instant>) -> Result<Message, Timeout> {
            if let Some(deadline) = deadline {
                match self
                    .rx
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                {
                    Ok(message) => Ok(message),
                    Err(RecvTimeoutError::Timeout) => Err(Timeout),
                    // should never be disconnected, the helper threads always
                    // announce their exit beforehand
                    Err(RecvTimeoutError::Disconnected) => unreachable!(),
                }
            } else {
                Ok(self.rx.recv().unwrap())
            }
        }

        fn read_into(
            &mut self,
            deadline: Option<Instant>,
            size_limit: Option<usize>,
            outvec: &mut Vec<u8>,
            errvec: &mut Vec<u8>,
        ) -> io::Result<()> {
            let mut grow_result =
                |ident, mut data: &[u8], leftover: &mut Option<(StreamIdent, Vec<u8>)>| {
                    if let Some(size_limit) = size_limit {
                        let total_read = outvec.len() + errvec.len();
                        if total_read >= size_limit {
                            return false;
                        }
                        let remaining = size_limit - total_read;
                        if data.len() > remaining {
                            *leftover = Some((ident, data[remaining..].to_vec()));
                            data = &data[..remaining];
                        }
                    }
                    match ident {
                        StreamIdent::Out => outvec.extend_from_slice(data),
                        StreamIdent::Err => errvec.extend_from_slice(data),
                        StreamIdent::In => unreachable!(),
                    }
                    if let Some(size_limit) = size_limit {
                        if outvec.len() + errvec.len() >= size_limit {
                            return false;
                        }
                    }
                    return true;
                };

            if let Some((ident, data)) = self.leftover.take() {
                if !grow_result(ident, &data, &mut self.leftover) {
                    return Ok(());
                }
            }

            while self.helper_set != 0 {
                match self.recv_until(deadline) {
                    Ok((ident, Payload::EOF)) => {
                        self.helper_set &= !(ident as u8);
                        continue;
                    }
                    Ok((ident, Payload::Data(data))) => {
                        assert!(data.len() != 0);
                        if !grow_result(ident, &data, &mut self.leftover) {
                            break;
                        }
                    }
                    Ok((_ident, Payload::Err(e))) => {
                        return Err(e);
                    }
                    Err(Timeout) => {
                        return Err(io::Error::new(io::ErrorKind::TimedOut, "timeout"));
                    }
                }
            }
            Ok(())
        }

        pub fn read(
            &mut self,
            deadline: Option<Instant>,
            size_limit: Option<usize>,
        ) -> (Option<io::Error>, (Option<Vec<u8>>, Option<Vec<u8>>)) {
            // Create both vectors immediately.  This doesn't allocate, and if
            // one of those is not needed, it just won't get resized.
            let mut outvec = vec![];
            let mut errvec = vec![];

            let err = self
                .read_into(deadline, size_limit, &mut outvec, &mut errvec)
                .err();
            let output = {
                let (mut o, mut e) = (None, None);
                if self.requested_streams & StreamIdent::Out as u8 != 0 {
                    o = Some(outvec);
                } else {
                    assert!(outvec.len() == 0);
                }
                if self.requested_streams & StreamIdent::Err as u8 != 0 {
                    e = Some(errvec);
                } else {
                    assert!(errvec.len() == 0);
                }
                (o, e)
            };
            (err, output)
        }
    }
}

use raw::RawCommunicator;

/// Unattended data exchange with the subprocess.
///
/// When a subprocess both expects input and provides output, care must be
/// taken to avoid deadlock.  The issue arises when the subprocess responds to
/// part of the input data by providing some output which must be read for the
/// subprocess to accept further input.  If the parent process is blocked on
/// writing the input, it cannot read the output and a deadlock occurs.  This
/// implementation avoids this issue by by reading from and writing to the
/// subprocess in parallel.  On Unix-like systems this is achieved using
/// `poll()`, and on Windows using threads.
#[must_use]
#[derive(Debug)]
pub struct Communicator {
    inner: RawCommunicator,
    size_limit: Option<usize>,
    time_limit: Option<Duration>,
}

impl Communicator {
    fn new(
        stdin: Option<File>,
        stdout: Option<File>,
        stderr: Option<File>,
        input_data: Option<Vec<u8>>,
    ) -> Communicator {
        Communicator {
            inner: RawCommunicator::new(stdin, stdout, stderr, input_data),
            size_limit: None,
            time_limit: None,
        }
    }

    /// Communicate with the subprocess, return the contents of its standard
    /// output and error.
    ///
    /// This will write input data to the subprocess's standard input and
    /// simultaneously read its standard output and error.  The output and
    /// error contents are returned as a pair of `Option<Vec>`.  The `None`
    /// options correspond to streams not specified as `Redirection::Pipe`
    /// when creating the subprocess.
    ///
    /// By default `read()` will read all data until end-of-file.
    ///
    /// If `limit_time` has been called, the method will read for no more than
    /// the specified duration.  In case of timeout, an error of kind
    /// `io::ErrorKind::TimedOut` is returned.  Communication may be resumed
    /// after the timeout by calling `read()` again.
    ///
    /// If `limit_size` has been called, it will limit the allocation done by
    /// this method.  If the subprocess provides more data than the limit
    /// specifies, `read()` will successfully return as much data as specified
    /// by the limit.  (It might internally read a bit more from the
    /// subprocess, but the data will remain available for future reads.)
    /// Subsequent data can be retrieved by calling `read()` again, which can
    /// be repeated until `read()` returns all-empty data, which marks EOF.
    ///
    /// Note that this method does not wait for the subprocess to finish, only
    /// to close its output/error streams.  It is rare but possible for the
    /// program to continue running after having closed the streams, in which
    /// case `Popen::Drop` will wait for it to finish.  If such a wait is
    /// undesirable, it can be prevented by waiting explicitly using `wait()`,
    /// by detaching the process using `detach()`, or by terminating it with
    /// `terminate()`.
    ///
    /// # Panics
    ///
    /// If `input_data` is provided and `stdin` was not redirected to a pipe.
    /// Also, if `input_data` is not provided and `stdin` was redirected to a
    /// pipe.
    ///
    /// # Errors
    ///
    /// * `Err(CommunicateError)` if a system call fails.  In case of timeout,
    /// the underlying error kind will be `ErrorKind::TimedOut`.
    ///
    /// Regardless of the nature of the error, the content prior to the error
    /// can be retrieved using the [`capture`] attribute of the error.
    ///
    /// [`capture`]: struct.CommunicateError.html#structfield.capture

    pub fn read(&mut self) -> Result<(Option<Vec<u8>>, Option<Vec<u8>>), CommunicateError> {
        let deadline = self.time_limit.map(|timeout| Instant::now() + timeout);
        match self.inner.read(deadline, self.size_limit) {
            (None, capture) => Ok(capture),
            (Some(error), capture) => Err(CommunicateError { error, capture }),
        }
    }

    /// Return the subprocess's output and error contents as strings.
    ///
    /// Like `read()`, but returns strings instead of byte vectors.  Invalid
    /// UTF-8 sequences, if found, are replaced with the the `U+FFFD` Unicode
    /// replacement character.
    pub fn read_string(&mut self) -> Result<(Option<String>, Option<String>), CommunicateError> {
        let (o, e) = self.read()?;
        Ok((o.map(from_utf8_lossy), e.map(from_utf8_lossy)))
    }

    /// Limit the amount of data the next `read()` will read from the
    /// subprocess.
    pub fn limit_size(mut self, size: usize) -> Communicator {
        self.size_limit = Some(size);
        self
    }

    /// Limit the amount of time the next `read()` will spend reading from the
    /// subprocess.
    pub fn limit_time(mut self, time: Duration) -> Communicator {
        self.time_limit = Some(time);
        self
    }
}

/// Like String::from_utf8_lossy(), but takes `Vec<u8>` and reuses its storage if
/// possible.
fn from_utf8_lossy(v: Vec<u8>) -> String {
    match String::from_utf8(v) {
        Ok(s) => s,
        Err(e) => String::from_utf8_lossy(e.as_bytes()).into(),
    }
}

pub fn communicate(
    stdin: Option<File>,
    stdout: Option<File>,
    stderr: Option<File>,
    input_data: Option<Vec<u8>>,
) -> Communicator {
    if stdin.is_some() {
        input_data
            .as_ref()
            .expect("must provide input to redirected stdin");
    } else {
        assert!(
            input_data.as_ref().is_none(),
            "cannot provide input to non-redirected stdin"
        );
    }
    Communicator::new(stdin, stdout, stderr, input_data)
}

/// Error during communication.
///
/// It holds the underlying `io::Error` in the `error` field, and also
/// provides the data captured before the error was encountered in the
/// `capture` field.
///
/// The error description and cause are taken from the underlying IO error.
#[derive(Debug)]
pub struct CommunicateError {
    /// The underlying `io::Error`.
    pub error: io::Error,
    /// The data captured before the error was encountered.
    pub capture: (Option<Vec<u8>>, Option<Vec<u8>>),
}

impl CommunicateError {
    /// Returns the corresponding IO `ErrorKind` for this error.
    ///
    /// Equivalent to `self.error.kind()`.
    pub fn kind(&self) -> ErrorKind {
        self.error.kind()
    }
}

impl Error for CommunicateError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.error.source()
    }
}

impl fmt::Display for CommunicateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(f)
    }
}
