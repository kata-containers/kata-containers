#[cfg(unix)]
mod os {
    pub const NULL_DEVICE: &str = "/dev/null";
    pub const SHELL: [&str; 2] = ["sh", "-c"];
}

#[cfg(windows)]
mod os {
    pub const NULL_DEVICE: &str = "nul";
    pub const SHELL: [&str; 2] = ["cmd.exe", "/c"];
}

pub use self::exec::{CaptureData, Exec, NullFile};
pub use self::os::*;
pub use self::pipeline::Pipeline;

#[cfg(unix)]
pub use exec::unix;

mod exec {
    use std::borrow::Cow;
    use std::collections::HashMap;
    use std::env;
    use std::ffi::{OsStr, OsString};
    use std::fmt;
    use std::fs::{File, OpenOptions};
    use std::io::{self, Read, Write};
    use std::ops::BitOr;
    use std::path::Path;

    use crate::communicate::Communicator;
    use crate::os_common::ExitStatus;
    use crate::popen::{Popen, PopenConfig, Redirection, Result as PopenResult};

    use super::os::*;
    use super::Pipeline;

    /// A builder for [`Popen`] instances, providing control and
    /// convenience methods.
    ///
    /// `Exec` provides a builder API for [`Popen::create`], and
    /// includes convenience methods for capturing the output, and for
    /// connecting subprocesses into pipelines.
    ///
    /// # Examples
    ///
    /// Execute an external command and wait for it to complete:
    ///
    /// ```no_run
    /// # use subprocess::*;
    /// # fn dummy() -> Result<()> {
    /// # let dirname = "some_dir";
    /// let exit_status = Exec::cmd("umount").arg(dirname).join()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Execute the command using the OS shell, like C's `system`:
    ///
    /// ```no_run
    /// # use subprocess::*;
    /// # fn dummy() -> Result<()> {
    /// Exec::shell("shutdown -h now").join()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Start a subprocess and obtain its output as a `Read` trait object,
    /// like C's `popen`:
    ///
    /// ```
    /// # use subprocess::*;
    /// # fn dummy() -> Result<()> {
    /// let stream = Exec::cmd("ls").stream_stdout()?;
    /// // call stream.read_to_string, construct io::BufReader(stream), etc.
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Capture the output of a command:
    ///
    /// ```
    /// # use subprocess::*;
    /// # fn dummy() -> Result<()> {
    /// let out = Exec::cmd("ls")
    ///   .stdout(Redirection::Pipe)
    ///   .capture()?
    ///   .stdout_str();
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Redirect errors to standard output, and capture both in a single stream:
    ///
    /// ```
    /// # use subprocess::*;
    /// # fn dummy() -> Result<()> {
    /// let out_and_err = Exec::cmd("ls")
    ///   .stdout(Redirection::Pipe)
    ///   .stderr(Redirection::Merge)
    ///   .capture()?
    ///   .stdout_str();
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Provide input to the command and read its output:
    ///
    /// ```
    /// # use subprocess::*;
    /// # fn dummy() -> Result<()> {
    /// let out = Exec::cmd("sort")
    ///   .stdin("b\nc\na\n")
    ///   .stdout(Redirection::Pipe)
    ///   .capture()?
    ///   .stdout_str();
    /// assert!(out == "a\nb\nc\n");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`Popen`]: struct.Popen.html
    /// [`Popen::create`]: struct.Popen.html#method.create

    #[must_use]
    pub struct Exec {
        command: OsString,
        args: Vec<OsString>,
        config: PopenConfig,
        stdin_data: Option<Vec<u8>>,
    }

    impl Exec {
        /// Constructs a new `Exec`, configured to run `command`.
        ///
        /// The command will be run directly in the OS, without an
        /// intervening shell.  To run it through a shell, use
        /// [`Exec::shell`] instead.
        ///
        /// By default, the command will be run without arguments, and
        /// none of the standard streams will be modified.
        ///
        /// [`Exec::shell`]: struct.Exec.html#method.shell
        pub fn cmd(command: impl AsRef<OsStr>) -> Exec {
            Exec {
                command: command.as_ref().to_owned(),
                args: vec![],
                config: PopenConfig::default(),
                stdin_data: None,
            }
        }

        /// Constructs a new `Exec`, configured to run `cmdstr` with
        /// the system shell.
        ///
        /// `subprocess` never spawns shells without an explicit
        /// request.  This command requests the shell to be used; on
        /// Unix-like systems, this is equivalent to
        /// `Exec::cmd("sh").arg("-c").arg(cmdstr)`.  On Windows, it
        /// runs `Exec::cmd("cmd.exe").arg("/c")`.
        ///
        /// `shell` is useful for porting code that uses the C
        /// `system` function, which also spawns a shell.
        ///
        /// When invoking this function, be careful not to interpolate
        /// arguments into the string run by the shell, such as
        /// `Exec::shell(format!("sort {}", filename))`.  Such code is
        /// prone to errors and, if `filename` comes from an untrusted
        /// source, to shell injection attacks.  Instead, use
        /// `Exec::cmd("sort").arg(filename)`.
        pub fn shell(cmdstr: impl AsRef<OsStr>) -> Exec {
            Exec::cmd(SHELL[0]).args(&SHELL[1..]).arg(cmdstr)
        }

        /// Appends `arg` to argument list.
        pub fn arg(mut self, arg: impl AsRef<OsStr>) -> Exec {
            self.args.push(arg.as_ref().to_owned());
            self
        }

        /// Extends the argument list with `args`.
        pub fn args(mut self, args: &[impl AsRef<OsStr>]) -> Exec {
            self.args.extend(args.iter().map(|x| x.as_ref().to_owned()));
            self
        }

        /// Specifies that the process is initially detached.
        ///
        /// A detached process means that we will not wait for the
        /// process to finish when the object that owns it goes out of
        /// scope.
        pub fn detached(mut self) -> Exec {
            self.config.detached = true;
            self
        }

        fn ensure_env(&mut self) {
            if self.config.env.is_none() {
                self.config.env = Some(PopenConfig::current_env());
            }
        }

        /// Clears the environment of the subprocess.
        ///
        /// When this is invoked, the subprocess will not inherit the
        /// environment of this process.
        pub fn env_clear(mut self) -> Exec {
            self.config.env = Some(vec![]);
            self
        }

        /// Sets an environment variable in the child process.
        ///
        /// If the same variable is set more than once, the last value
        /// is used.
        ///
        /// Other environment variables are by default inherited from
        /// the current process.  If this is undesirable, call
        /// `env_clear` first.
        pub fn env(mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Exec {
            self.ensure_env();
            self.config
                .env
                .as_mut()
                .unwrap()
                .push((key.as_ref().to_owned(), value.as_ref().to_owned()));
            self
        }

        /// Sets multiple environment variables in the child process.
        ///
        /// The keys and values of the variables are specified by the
        /// slice.  If the same variable is set more than once, the
        /// last value is used.
        ///
        /// Other environment variables are by default inherited from
        /// the current process.  If this is undesirable, call
        /// `env_clear` first.
        pub fn env_extend(mut self, vars: &[(impl AsRef<OsStr>, impl AsRef<OsStr>)]) -> Exec {
            self.ensure_env();
            {
                let envvec = self.config.env.as_mut().unwrap();
                for &(ref k, ref v) in vars {
                    envvec.push((k.as_ref().to_owned(), v.as_ref().to_owned()));
                }
            }
            self
        }

        /// Removes an environment variable from the child process.
        ///
        /// Other environment variables are inherited by default.
        pub fn env_remove(mut self, key: impl AsRef<OsStr>) -> Exec {
            self.ensure_env();
            self.config
                .env
                .as_mut()
                .unwrap()
                .retain(|&(ref k, ref _v)| k != key.as_ref());
            self
        }

        /// Specifies the current working directory of the child process.
        ///
        /// If unspecified, the current working directory is inherited
        /// from the parent.
        pub fn cwd(mut self, dir: impl AsRef<Path>) -> Exec {
            self.config.cwd = Some(dir.as_ref().as_os_str().to_owned());
            self
        }

        /// Specifies how to set up the standard input of the child process.
        ///
        /// Argument can be:
        ///
        /// * a [`Redirection`];
        /// * a `File`, which is a shorthand for `Redirection::File(file)`;
        /// * a `Vec<u8>` or `&str`, which will set up a `Redirection::Pipe`
        ///   for stdin, making sure that `capture` feeds that data into the
        ///   standard input of the subprocess;
        /// * [`NullFile`], which will redirect the standard input to read from
        ///    `/dev/null`.
        ///
        /// [`Redirection`]: enum.Redirection.html
        /// [`NullFile`]: struct.NullFile.html
        pub fn stdin(mut self, stdin: impl Into<InputRedirection>) -> Exec {
            match (&self.config.stdin, stdin.into()) {
                (&Redirection::None, InputRedirection::AsRedirection(new)) => {
                    self.config.stdin = new
                }
                (&Redirection::Pipe, InputRedirection::AsRedirection(Redirection::Pipe)) => (),
                (&Redirection::None, InputRedirection::FeedData(data)) => {
                    self.config.stdin = Redirection::Pipe;
                    self.stdin_data = Some(data);
                }
                (_, _) => panic!("stdin is already set"),
            }
            self
        }

        /// Specifies how to set up the standard output of the child process.
        ///
        /// Argument can be:
        ///
        /// * a [`Redirection`];
        /// * a `File`, which is a shorthand for `Redirection::File(file)`;
        /// * [`NullFile`], which will redirect the standard output to go to
        ///    `/dev/null`.
        ///
        /// [`Redirection`]: enum.Redirection.html
        /// [`NullFile`]: struct.NullFile.html
        pub fn stdout(mut self, stdout: impl Into<OutputRedirection>) -> Exec {
            match (&self.config.stdout, stdout.into().into_redirection()) {
                (&Redirection::None, new) => self.config.stdout = new,
                (&Redirection::Pipe, Redirection::Pipe) => (),
                (_, _) => panic!("stdout is already set"),
            }
            self
        }

        /// Specifies how to set up the standard error of the child process.
        ///
        /// Argument can be:
        ///
        /// * a [`Redirection`];
        /// * a `File`, which is a shorthand for `Redirection::File(file)`;
        /// * [`NullFile`], which will redirect the standard error to go to
        ///    `/dev/null`.
        ///
        /// [`Redirection`]: enum.Redirection.html
        /// [`NullFile`]: struct.NullFile.html
        pub fn stderr(mut self, stderr: impl Into<OutputRedirection>) -> Exec {
            match (&self.config.stderr, stderr.into().into_redirection()) {
                (&Redirection::None, new) => self.config.stderr = new,
                (&Redirection::Pipe, Redirection::Pipe) => (),
                (_, _) => panic!("stderr is already set"),
            }
            self
        }

        fn check_no_stdin_data(&self, meth: &str) {
            if self.stdin_data.is_some() {
                panic!("{} called with input data specified", meth);
            }
        }

        // Terminators

        /// Starts the process, returning a `Popen` for the running process.
        pub fn popen(mut self) -> PopenResult<Popen> {
            self.check_no_stdin_data("popen");
            self.args.insert(0, self.command);
            let p = Popen::create(&self.args, self.config)?;
            Ok(p)
        }

        /// Starts the process, waits for it to finish, and returns
        /// the exit status.
        ///
        /// This method will wait for as long as necessary for the process to
        /// finish.  If a timeout is needed, use
        /// `<...>.detached().popen()?.wait_timeout(...)` instead.
        pub fn join(self) -> PopenResult<ExitStatus> {
            self.check_no_stdin_data("join");
            self.popen()?.wait()
        }

        /// Starts the process and returns a value implementing the `Read`
        /// trait that reads from the standard output of the child process.
        ///
        /// This will automatically set up
        /// `stdout(Redirection::Pipe)`, so it is not necessary to do
        /// that beforehand.
        ///
        /// When the trait object is dropped, it will wait for the
        /// process to finish.  If this is undesirable, use
        /// `detached()`.
        pub fn stream_stdout(self) -> PopenResult<impl Read> {
            self.check_no_stdin_data("stream_stdout");
            let p = self.stdout(Redirection::Pipe).popen()?;
            Ok(ReadOutAdapter(p))
        }

        /// Starts the process and returns a value implementing the `Read`
        /// trait that reads from the standard error of the child process.
        ///
        /// This will automatically set up
        /// `stderr(Redirection::Pipe)`, so it is not necessary to do
        /// that beforehand.
        ///
        /// When the trait object is dropped, it will wait for the
        /// process to finish.  If this is undesirable, use
        /// `detached()`.
        pub fn stream_stderr(self) -> PopenResult<impl Read> {
            self.check_no_stdin_data("stream_stderr");
            let p = self.stderr(Redirection::Pipe).popen()?;
            Ok(ReadErrAdapter(p))
        }

        /// Starts the process and returns a value implementing the `Write`
        /// trait that writes to the standard input of the child process.
        ///
        /// This will automatically set up `stdin(Redirection::Pipe)`,
        /// so it is not necessary to do that beforehand.
        ///
        /// When the trait object is dropped, it will wait for the
        /// process to finish.  If this is undesirable, use
        /// `detached()`.
        pub fn stream_stdin(self) -> PopenResult<impl Write> {
            self.check_no_stdin_data("stream_stdin");
            let p = self.stdin(Redirection::Pipe).popen()?;
            Ok(WriteAdapter(p))
        }

        fn setup_communicate(mut self) -> PopenResult<(Communicator, Popen)> {
            let stdin_data = self.stdin_data.take();
            if let (&Redirection::None, &Redirection::None) =
                (&self.config.stdout, &self.config.stderr)
            {
                self = self.stdout(Redirection::Pipe);
            }
            let mut p = self.popen()?;

            Ok((p.communicate_start(stdin_data), p))
        }

        /// Starts the process and returns a `Communicator` handle.
        ///
        /// This is a lower-level API that offers more choice in how
        /// communication is performed, such as read size limit and timeout,
        /// equivalent to [`Popen::communicate`].
        ///
        /// Unlike `capture()`, this method doesn't wait for the process to
        /// finish, effectively detaching it.
        ///
        /// [`Popen::communicate`]: struct.Popen.html#method.communicate
        pub fn communicate(self) -> PopenResult<Communicator> {
            let comm = self.detached().setup_communicate()?.0;
            Ok(comm)
        }

        /// Starts the process, collects its output, and waits for it
        /// to finish.
        ///
        /// The return value provides the standard output and standard
        /// error as bytes or optionally strings, as well as the exit
        /// status.
        ///
        /// Unlike `Popen::communicate`, this method actually waits
        /// for the process to finish, rather than simply waiting for
        /// its standard streams to close.  If this is undesirable,
        /// use `detached()`.
        pub fn capture(self) -> PopenResult<CaptureData> {
            let (mut comm, mut p) = self.setup_communicate()?;
            let (maybe_out, maybe_err) = comm.read()?;
            Ok(CaptureData {
                stdout: maybe_out.unwrap_or_else(Vec::new),
                stderr: maybe_err.unwrap_or_else(Vec::new),
                exit_status: p.wait()?,
            })
        }

        // used for Debug impl
        fn display_escape(s: &str) -> Cow<'_, str> {
            fn nice_char(c: char) -> bool {
                match c {
                    '-' | '_' | '.' | ',' | '/' => true,
                    c if c.is_ascii_alphanumeric() => true,
                    _ => false,
                }
            }
            if !s.chars().all(nice_char) {
                Cow::Owned(format!("'{}'", s.replace("'", r#"'\''"#)))
            } else {
                Cow::Borrowed(s)
            }
        }

        /// Show Exec as command-line string quoted in the Unix style.
        pub fn to_cmdline_lossy(&self) -> String {
            let mut out = String::new();
            if let Some(ref cmd_env) = self.config.env {
                let current: Vec<_> = env::vars_os().collect();
                let current_map: HashMap<_, _> = current.iter().map(|(x, y)| (x, y)).collect();
                for (k, v) in cmd_env {
                    if current_map.get(&k) == Some(&&v) {
                        continue;
                    }
                    out.push_str(&Exec::display_escape(&k.to_string_lossy()));
                    out.push('=');
                    out.push_str(&Exec::display_escape(&v.to_string_lossy()));
                    out.push(' ');
                }
                let cmd_env: HashMap<_, _> = cmd_env.iter().map(|(k, v)| (k, v)).collect();
                for (k, _) in current {
                    if !cmd_env.contains_key(&k) {
                        out.push_str(&Exec::display_escape(&k.to_string_lossy()));
                        out.push('=');
                        out.push(' ');
                    }
                }
            }
            out.push_str(&Exec::display_escape(&self.command.to_string_lossy()));
            for arg in &self.args {
                out.push(' ');
                out.push_str(&Exec::display_escape(&arg.to_string_lossy()));
            }
            out
        }
    }

    impl Clone for Exec {
        /// Returns a copy of the value.
        ///
        /// This method is guaranteed not to fail as long as none of
        /// the `Redirection` values contain a `Redirection::File`
        /// variant.  If a redirection to `File` is present, cloning
        /// that field will use `File::try_clone` method, which
        /// duplicates a file descriptor and can (but is not likely
        /// to) fail.  In that scenario, `Exec::clone` panics.
        fn clone(&self) -> Exec {
            Exec {
                command: self.command.clone(),
                args: self.args.clone(),
                config: self.config.try_clone().unwrap(),
                stdin_data: self.stdin_data.as_ref().cloned(),
            }
        }
    }

    impl BitOr for Exec {
        type Output = Pipeline;

        /// Create a `Pipeline` from `self` and `rhs`.
        fn bitor(self, rhs: Exec) -> Pipeline {
            Pipeline::new(self, rhs)
        }
    }

    impl fmt::Debug for Exec {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "Exec {{ {} }}", self.to_cmdline_lossy())
        }
    }

    #[derive(Debug)]
    struct ReadOutAdapter(Popen);

    impl Read for ReadOutAdapter {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.0.stdout.as_mut().unwrap().read(buf)
        }
    }

    #[derive(Debug)]
    struct ReadErrAdapter(Popen);

    impl Read for ReadErrAdapter {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.0.stderr.as_mut().unwrap().read(buf)
        }
    }

    #[derive(Debug)]
    struct WriteAdapter(Popen);

    impl Write for WriteAdapter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.stdin.as_mut().unwrap().write(buf)
        }
        fn flush(&mut self) -> io::Result<()> {
            self.0.stdin.as_mut().unwrap().flush()
        }
    }

    // We must implement Drop in order to close the stream.  The typical
    // use case for stream_stdin() is a process that reads something from
    // stdin.  WriteAdapter going out of scope invokes Popen::drop(),
    // which waits for the process to exit.  Without closing stdin, this
    // deadlocks because the child process hangs reading its stdin.

    impl Drop for WriteAdapter {
        fn drop(&mut self) {
            self.0.stdin.take();
        }
    }

    /// Data captured by [`Exec::capture`] and [`Pipeline::capture`].
    ///
    /// [`Exec::capture`]: struct.Exec.html#method.capture
    /// [`Pipeline::capture`]: struct.Pipeline.html#method.capture
    #[derive(Debug)]
    pub struct CaptureData {
        /// Standard output as bytes.
        pub stdout: Vec<u8>,
        /// Standard error as bytes.
        pub stderr: Vec<u8>,
        /// Exit status.
        pub exit_status: ExitStatus,
    }

    impl CaptureData {
        /// Returns the standard output as string, converted from bytes using
        /// `String::from_utf8_lossy`.
        pub fn stdout_str(&self) -> String {
            String::from_utf8_lossy(&self.stdout).into_owned()
        }

        /// Returns the standard error as string, converted from bytes using
        /// `String::from_utf8_lossy`.
        pub fn stderr_str(&self) -> String {
            String::from_utf8_lossy(&self.stderr).into_owned()
        }

        /// True if the exit status of the process or pipeline is 0.
        pub fn success(&self) -> bool {
            self.exit_status.success()
        }
    }

    #[derive(Debug)]
    pub enum InputRedirection {
        AsRedirection(Redirection),
        FeedData(Vec<u8>),
    }

    impl From<Redirection> for InputRedirection {
        fn from(r: Redirection) -> Self {
            if let Redirection::Merge = r {
                panic!("Redirection::Merge is only allowed for output streams");
            }
            InputRedirection::AsRedirection(r)
        }
    }

    impl From<File> for InputRedirection {
        fn from(f: File) -> Self {
            InputRedirection::AsRedirection(Redirection::File(f))
        }
    }

    /// Marker value for [`stdin`], [`stdout`], and [`stderr`] methods
    /// of [`Exec`] and [`Pipeline`].
    ///
    /// Use of this value means that the corresponding stream should
    /// be redirected to the devnull device.
    ///
    /// [`stdin`]: struct.Exec.html#method.stdin
    /// [`stdout`]: struct.Exec.html#method.stdout
    /// [`stderr`]: struct.Exec.html#method.stderr
    /// [`Exec`]: struct.Exec.html
    /// [`Pipeline`]: struct.Pipeline.html
    #[derive(Debug)]
    pub struct NullFile;

    impl From<NullFile> for InputRedirection {
        fn from(_nf: NullFile) -> Self {
            let null_file = OpenOptions::new().read(true).open(NULL_DEVICE).unwrap();
            InputRedirection::AsRedirection(Redirection::File(null_file))
        }
    }

    impl From<Vec<u8>> for InputRedirection {
        fn from(v: Vec<u8>) -> Self {
            InputRedirection::FeedData(v)
        }
    }

    impl<'a> From<&'a str> for InputRedirection {
        fn from(s: &'a str) -> Self {
            InputRedirection::FeedData(s.as_bytes().to_vec())
        }
    }

    #[derive(Debug)]
    pub struct OutputRedirection(Redirection);

    impl OutputRedirection {
        pub fn into_redirection(self) -> Redirection {
            self.0
        }
    }

    impl From<Redirection> for OutputRedirection {
        fn from(r: Redirection) -> Self {
            OutputRedirection(r)
        }
    }

    impl From<File> for OutputRedirection {
        fn from(f: File) -> Self {
            OutputRedirection(Redirection::File(f))
        }
    }

    impl From<NullFile> for OutputRedirection {
        fn from(_nf: NullFile) -> Self {
            let null_file = OpenOptions::new().write(true).open(NULL_DEVICE).unwrap();
            OutputRedirection(Redirection::File(null_file))
        }
    }

    #[cfg(unix)]
    pub mod unix {
        use super::Exec;

        pub trait ExecExt {
            fn setuid(self, uid: u32) -> Self;
            fn setgid(self, gid: u32) -> Self;
        }

        impl ExecExt for Exec {
            fn setuid(mut self, uid: u32) -> Exec {
                self.config.setuid = Some(uid);
                self
            }

            fn setgid(mut self, gid: u32) -> Exec {
                self.config.setgid = Some(gid);
                self
            }
        }
    }
}

mod pipeline {
    use std::fmt;
    use std::fs::File;
    use std::io::{self, Read, Write};
    use std::ops::BitOr;
    use std::rc::Rc;

    use crate::communicate::{self, Communicator};
    use crate::os_common::ExitStatus;
    use crate::popen::{Popen, Redirection, Result as PopenResult};

    use super::exec::{CaptureData, Exec, InputRedirection, OutputRedirection};

    /// A builder for multiple [`Popen`] instances connected via
    /// pipes.
    ///
    /// A pipeline is a sequence of two or more [`Exec`] commands
    /// connected via pipes.  Just like in a Unix shell pipeline, each
    /// command receives standard input from the previous command, and
    /// passes standard output to the next command.  Optionally, the
    /// standard input of the first command can be provided from the
    /// outside, and the output of the last command can be captured.
    ///
    /// In most cases you do not need to create [`Pipeline`] instances
    /// directly; instead, combine [`Exec`] instances using the `|`
    /// operator which produces `Pipeline`.
    ///
    /// # Examples
    ///
    /// Execute a pipeline and return the exit status of the last command:
    ///
    /// ```no_run
    /// # use subprocess::*;
    /// # fn dummy() -> Result<()> {
    /// let exit_status =
    ///   (Exec::shell("ls *.bak") | Exec::cmd("xargs").arg("rm")).join()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Capture the pipeline's output:
    ///
    /// ```no_run
    /// # use subprocess::*;
    /// # fn dummy() -> Result<()> {
    /// let dir_checksum = {
    ///     Exec::cmd("find . -type f") | Exec::cmd("sort") | Exec::cmd("sha1sum")
    /// }.capture()?.stdout_str();
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`Popen`]: struct.Popen.html
    /// [`Exec`]: struct.Exec.html
    /// [`Pipeline`]: struct.Pipeline.html

    #[must_use]
    pub struct Pipeline {
        cmds: Vec<Exec>,
        stdin: Redirection,
        stdout: Redirection,
        stderr_file: Option<File>,
        stdin_data: Option<Vec<u8>>,
    }

    impl Pipeline {
        /// Creates a new pipeline by combining two commands.
        ///
        /// Equivalent to `cmd1 | cmd2`.
        pub fn new(cmd1: Exec, cmd2: Exec) -> Pipeline {
            Pipeline {
                cmds: vec![cmd1, cmd2],
                stdin: Redirection::None,
                stdout: Redirection::None,
                stderr_file: None,
                stdin_data: None,
            }
        }

        /// Creates a new pipeline from a list of commands. Useful if
        /// a pipeline should be created dynamically.
        ///
        /// Example:
        /// ```
        /// use subprocess::Exec;
        ///
        /// let commands = vec![
        ///   Exec::shell("echo tset"),
        ///   Exec::shell("tr '[:lower:]' '[:upper:]'"),
        ///   Exec::shell("rev")
        /// ];
        ///
        /// let pipeline = subprocess::Pipeline::from_exec_iter(commands);
        /// let output = pipeline.capture().unwrap().stdout_str();
        /// assert_eq!(output, "TEST\n");
        /// ```
        /// ```should_panic
        /// use subprocess::Exec;
        ///
        /// let commands = vec![
        ///   Exec::shell("echo tset"),
        /// ];
        ///
        /// // This will panic as the iterator contains less than two (2) items.
        /// let pipeline = subprocess::Pipeline::from_exec_iter(commands);
        /// ```
        /// Errors:
        ///   - Panics when the passed iterator contains less than two (2) items.
        pub fn from_exec_iter<I>(iterable: I) -> Pipeline
        where
            I: IntoIterator<Item = Exec>,
        {
            let cmds: Vec<_> = iterable.into_iter().collect();

            if cmds.len() < 2 {
                panic!("iterator needs to contain at least two (2) elements")
            }

            Pipeline {
                cmds,
                stdin: Redirection::None,
                stdout: Redirection::None,
                stderr_file: None,
                stdin_data: None,
            }
        }

        /// Specifies how to set up the standard input of the first
        /// command in the pipeline.
        ///
        /// Argument can be:
        ///
        /// * a [`Redirection`];
        /// * a `File`, which is a shorthand for `Redirection::File(file)`;
        /// * a `Vec<u8>` or `&str`, which will set up a `Redirection::Pipe`
        ///   for stdin, making sure that `capture` feeds that data into the
        ///   standard input of the subprocess.
        /// * `NullFile`, which will redirect the standard input to read from
        ///    /dev/null.
        ///
        /// [`Redirection`]: enum.Redirection.html
        pub fn stdin(mut self, stdin: impl Into<InputRedirection>) -> Pipeline {
            match stdin.into() {
                InputRedirection::AsRedirection(r) => self.stdin = r,
                InputRedirection::FeedData(data) => {
                    self.stdin = Redirection::Pipe;
                    self.stdin_data = Some(data);
                }
            };
            self
        }

        /// Specifies how to set up the standard output of the last
        /// command in the pipeline.
        ///
        /// Argument can be:
        ///
        /// * a [`Redirection`];
        /// * a `File`, which is a shorthand for `Redirection::File(file)`;
        /// * `NullFile`, which will redirect the standard output to write to
        ///    /dev/null.
        ///
        /// [`Redirection`]: enum.Redirection.html
        pub fn stdout(mut self, stdout: impl Into<OutputRedirection>) -> Pipeline {
            self.stdout = stdout.into().into_redirection();
            self
        }

        /// Specifies a file to which to redirect the standard error of all
        /// the commands in the pipeline.
        ///
        /// It is useful for capturing the standard error of the pipeline as a
        /// whole.  Unlike `stdout()`, which only affects the last command in
        /// the pipeline, this affects all commands.  The difference is
        /// because standard output is piped from one command to the next, so
        /// only the output of the last command is "free".  In contrast, the
        /// standard errors are not connected in any way.  This is also the
        /// reason only a `File` is supported - it allows for efficient
        /// sharing of the same file by all commands.
        pub fn stderr_to(mut self, to: File) -> Pipeline {
            self.stderr_file = Some(to);
            self
        }

        fn check_no_stdin_data(&self, meth: &str) {
            if self.stdin_data.is_some() {
                panic!("{} called with input data specified", meth);
            }
        }

        // Terminators:

        /// Starts all commands in the pipeline, and returns a
        /// `Vec<Popen>` whose members correspond to running commands.
        ///
        /// If some command fails to start, the remaining commands
        /// will not be started, and the appropriate error will be
        /// returned.  The commands that have already started will be
        /// waited to finish (but will probably exit immediately due
        /// to missing output), except for the ones for which
        /// `detached()` was called.  This is equivalent to what the
        /// shell does.
        pub fn popen(mut self) -> PopenResult<Vec<Popen>> {
            self.check_no_stdin_data("popen");
            assert!(self.cmds.len() >= 2);

            if let Some(stderr_to) = self.stderr_file {
                let stderr_to = Rc::new(stderr_to);
                self.cmds = self
                    .cmds
                    .into_iter()
                    .map(|cmd| cmd.stderr(Redirection::RcFile(Rc::clone(&stderr_to))))
                    .collect();
            }

            let first_cmd = self.cmds.drain(..1).next().unwrap();
            self.cmds.insert(0, first_cmd.stdin(self.stdin));

            let last_cmd = self.cmds.drain(self.cmds.len() - 1..).next().unwrap();
            self.cmds.push(last_cmd.stdout(self.stdout));

            let mut ret = Vec::<Popen>::new();
            let cnt = self.cmds.len();

            for (idx, mut runner) in self.cmds.into_iter().enumerate() {
                if idx != 0 {
                    let prev_stdout = ret[idx - 1].stdout.take().unwrap();
                    runner = runner.stdin(prev_stdout);
                }
                if idx != cnt - 1 {
                    runner = runner.stdout(Redirection::Pipe);
                }
                ret.push(runner.popen()?);
            }
            Ok(ret)
        }

        /// Starts the pipeline, waits for it to finish, and returns
        /// the exit status of the last command.
        pub fn join(self) -> PopenResult<ExitStatus> {
            self.check_no_stdin_data("join");
            let mut v = self.popen()?;
            // Waiting on a pipeline waits for all commands, but
            // returns the status of the last one.  This is how the
            // shells do it.  If the caller needs more precise control
            // over which status is returned, they can call popen().
            v.last_mut().unwrap().wait()
        }

        /// Starts the pipeline and returns a value implementing the `Read`
        /// trait that reads from the standard output of the last command.
        ///
        /// This will automatically set up
        /// `stdout(Redirection::Pipe)`, so it is not necessary to do
        /// that beforehand.
        ///
        /// When the trait object is dropped, it will wait for the
        /// pipeline to finish.  If this is undesirable, use
        /// `detached()`.
        pub fn stream_stdout(self) -> PopenResult<impl Read> {
            self.check_no_stdin_data("stream_stdout");
            let v = self.stdout(Redirection::Pipe).popen()?;
            Ok(ReadPipelineAdapter(v))
        }

        /// Starts the pipeline and returns a value implementing the `Write`
        /// trait that writes to the standard input of the last command.
        ///
        /// This will automatically set up `stdin(Redirection::Pipe)`,
        /// so it is not necessary to do that beforehand.
        ///
        /// When the trait object is dropped, it will wait for the
        /// process to finish.  If this is undesirable, use
        /// `detached()`.
        pub fn stream_stdin(self) -> PopenResult<impl Write> {
            self.check_no_stdin_data("stream_stdin");
            let v = self.stdin(Redirection::Pipe).popen()?;
            Ok(WritePipelineAdapter(v))
        }

        fn setup_communicate(mut self) -> PopenResult<(Communicator, Vec<Popen>)> {
            assert!(self.cmds.len() >= 2);

            let (err_read, err_write) = crate::popen::make_pipe()?;
            self = self.stderr_to(err_write);

            let stdin_data = self.stdin_data.take();
            let mut v = self.stdout(Redirection::Pipe).popen()?;
            let vlen = v.len();

            let comm = communicate::communicate(
                v[0].stdin.take(),
                v[vlen - 1].stdout.take(),
                Some(err_read),
                stdin_data,
            );
            Ok((comm, v))
        }

        /// Starts the pipeline and returns a `Communicator` handle.
        ///
        /// This is a lower-level API that offers more choice in how
        /// communication is performed, such as read size limit and timeout,
        /// equivalent to [`Popen::communicate`].
        ///
        /// Unlike `capture()`, this method doesn't wait for the pipeline to
        /// finish, effectively detaching it.
        ///
        /// [`Popen::communicate`]: struct.Popen.html#method.communicate
        pub fn communicate(mut self) -> PopenResult<Communicator> {
            self.cmds = self.cmds.into_iter().map(|cmd| cmd.detached()).collect();
            let comm = self.setup_communicate()?.0;
            Ok(comm)
        }

        /// Starts the pipeline, collects its output, and waits for all
        /// commands to finish.
        ///
        /// The return value provides the standard output of the last command,
        /// the combined standard error of all commands, and the exit status
        /// of the last command.  The captured outputs can be accessed as
        /// bytes or strings.
        ///
        /// Unlike `Popen::communicate`, this method actually waits for the
        /// processes to finish, rather than simply waiting for the output to
        /// close.  If this is undesirable, use `detached()`.
        pub fn capture(self) -> PopenResult<CaptureData> {
            let (mut comm, mut v) = self.setup_communicate()?;
            let (out, err) = comm.read()?;
            let out = out.unwrap_or_else(Vec::new);
            let err = err.unwrap();

            let vlen = v.len();
            let status = v[vlen - 1].wait()?;

            Ok(CaptureData {
                stdout: out,
                stderr: err,
                exit_status: status,
            })
        }
    }

    impl Clone for Pipeline {
        /// Returns a copy of the value.
        ///
        /// This method is guaranteed not to fail as long as none of
        /// the `Redirection` values contain a `Redirection::File`
        /// variant.  If a redirection to `File` is present, cloning
        /// that field will use `File::try_clone` method, which
        /// duplicates a file descriptor and can (but is not likely
        /// to) fail.  In that scenario, `Exec::clone` panics.
        fn clone(&self) -> Pipeline {
            Pipeline {
                cmds: self.cmds.clone(),
                stdin: self.stdin.try_clone().unwrap(),
                stdout: self.stdout.try_clone().unwrap(),
                stderr_file: self.stderr_file.as_ref().map(|f| f.try_clone().unwrap()),
                stdin_data: self.stdin_data.clone(),
            }
        }
    }

    impl BitOr<Exec> for Pipeline {
        type Output = Pipeline;

        /// Append a command to the pipeline and return a new pipeline.
        fn bitor(mut self, rhs: Exec) -> Pipeline {
            self.cmds.push(rhs);
            self
        }
    }

    impl BitOr for Pipeline {
        type Output = Pipeline;

        /// Append a pipeline to the pipeline and return a new pipeline.
        fn bitor(mut self, rhs: Pipeline) -> Pipeline {
            self.cmds.extend(rhs.cmds);
            self.stdout = rhs.stdout;
            self
        }
    }

    impl fmt::Debug for Pipeline {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            let mut args = vec![];
            for cmd in &self.cmds {
                args.push(cmd.to_cmdline_lossy());
            }
            write!(f, "Pipeline {{ {} }}", args.join(" | "))
        }
    }

    #[derive(Debug)]
    struct ReadPipelineAdapter(Vec<Popen>);

    impl Read for ReadPipelineAdapter {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let last = self.0.last_mut().unwrap();
            last.stdout.as_mut().unwrap().read(buf)
        }
    }

    #[derive(Debug)]
    struct WritePipelineAdapter(Vec<Popen>);

    impl WritePipelineAdapter {
        fn stdin(&mut self) -> &mut File {
            let first = self.0.first_mut().unwrap();
            first.stdin.as_mut().unwrap()
        }
    }

    impl Write for WritePipelineAdapter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.stdin().write(buf)
        }
        fn flush(&mut self) -> io::Result<()> {
            self.stdin().flush()
        }
    }

    impl Drop for WritePipelineAdapter {
        // the same rationale as Drop for WriteAdapter
        fn drop(&mut self) {
            let first = &mut self.0[0];
            first.stdin.take();
        }
    }
}
