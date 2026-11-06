// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use libc::pid_t;
use std::fs::File;
use std::os::unix::io::{IntoRawFd, RawFd};
use tokio::sync::mpsc::Sender;
use tokio_vsock::VsockStream;

use nix::errno::Errno;
use nix::fcntl::{fcntl, FcntlArg, OFlag};
use nix::sys::wait::{self, WaitStatus};
use nix::unistd::{self, Pid};
use nix::Result;

use oci::Process as OCIProcess;
use oci_spec::runtime as oci;
use slog::Logger;

use crate::pipestream::PipeStream;
use awaitgroup::WaitGroup;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{split, ReadHalf, WriteHalf};
use tokio::sync::Mutex;
use tokio::sync::Notify;

#[cfg(feature = "cc-op")]
use kata_agent_cc_op::{processors::StreamProcessor, CryptoRunData as OtherCryptoRunData};

macro_rules! close_process_stream {
    ($self: ident, $stream:ident, $stream_type: ident) => {
        if $self.$stream.is_some() {
            $self.close_stream(StreamType::$stream_type);
            $self.$stream = None;
        }
    };
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum StreamType {
    Stdin,
    Stdout,
    Stderr,
    TermMaster,
    ParentStdin,
    ParentStdout,
    ParentStderr,
}

type Reader = Arc<Mutex<ReadHalf<PipeStream>>>;
type Writer = Arc<Mutex<WriteHalf<PipeStream>>>;

#[derive(Debug)]
pub struct ProcessIo {
    pub stdin: Option<VsockStream>,
    pub stdout: Option<VsockStream>,
    pub stderr: Option<VsockStream>,
    // used to wait for all process outputs to be copied to the vsock streams
    // only used when tty is used.
    pub wg_output: WaitGroup,
}

impl ProcessIo {
    pub fn new(
        stdin: Option<VsockStream>,
        stdout: Option<VsockStream>,
        stderr: Option<VsockStream>,
    ) -> Self {
        ProcessIo {
            stdin,
            stdout,
            stderr,
            wg_output: WaitGroup::new(),
        }
    }
}

#[derive(Debug)]
pub struct Process {
    pub exec_id: String,
    pub stdin: Option<RawFd>,
    pub stdout: Option<RawFd>,
    pub stderr: Option<RawFd>,
    pub exit_tx: Option<tokio::sync::watch::Sender<bool>>,
    pub exit_rx: Option<tokio::sync::watch::Receiver<bool>>,
    pub extra_files: Vec<File>,
    pub term_master: Option<RawFd>,
    pub tty: bool,
    pub parent_stdin: Option<RawFd>,
    pub parent_stdout: Option<RawFd>,
    pub parent_stderr: Option<RawFd>,
    pub init: bool,
    // pid of the init/exec process. since we have no command
    // struct to store pid, we must store pid here.
    pub pid: pid_t,

    pub exit_code: i32,
    pub exit_watchers: Vec<Sender<i32>>,
    pub oci: OCIProcess,
    pub logger: Logger,
    pub term_exit_notifier: Arc<Notify>,

    readers: HashMap<StreamType, Reader>,
    writers: HashMap<StreamType, Writer>,

    pub proc_io: Option<ProcessIo>,
    pub crypto_run_data: Option<CryptoRunData>,

    #[cfg(feature = "cc-op")]
    pub stream_processor: Option<Arc<Mutex<Box<dyn StreamProcessor>>>>,
}

pub trait ProcessOperations {
    fn pid(&self) -> Pid;
    fn wait(&self) -> Result<WaitStatus>;
    fn signal(&self, sig: libc::c_int) -> Result<()>;
}

impl ProcessOperations for Process {
    fn pid(&self) -> Pid {
        Pid::from_raw(self.pid)
    }

    fn wait(&self) -> Result<WaitStatus> {
        wait::waitpid(Some(self.pid()), None)
    }

    fn signal(&self, sig: libc::c_int) -> Result<()> {
        let res = unsafe { libc::kill(self.pid().into(), sig) };

        Errno::result(res).map(drop)
    }
}

#[derive(Debug, Default)]
pub struct StreamCrypto {
    pub args: Vec<String>,
    pub enabled: bool,
    pub key: Vec<u8>,
    pub iv_in: Vec<u8>,
    pub iv_out: Vec<u8>,
    pub nonce: Vec<u8>,
    pub has_sandbox_id: bool,
}

#[derive(Clone, Debug)]
pub enum CryptoRunData {
    AttachLogsMode {
        enabled: bool,
        has_sandbox_id: bool,
        in_key: Vec<u8>,
        out_key: Vec<u8>,
    },
    ExecMode {
        args: Vec<String>,
        enabled: bool,
        in_key: Vec<u8>,
        out_key: Vec<u8>,
    },
}

#[cfg(feature = "cc-op")]
impl From<&OtherCryptoRunData> for CryptoRunData {
    fn from(external: &OtherCryptoRunData) -> Self {
        match external {
            OtherCryptoRunData::AttachLogsMode {
                enabled,
                has_sandbox_id,
                in_key,
                out_key,
            } => CryptoRunData::AttachLogsMode {
                enabled: enabled.clone(),
                has_sandbox_id: has_sandbox_id.clone(),
                in_key: in_key.clone(),
                out_key: out_key.clone(),
            },
            OtherCryptoRunData::ExecMode {
                args,
                enabled,
                in_key,
                out_key,
            } => CryptoRunData::ExecMode {
                args: args.clone(),
                enabled: enabled.clone(),
                in_key: in_key.clone(),
                out_key: out_key.clone(),
            },
        }
    }
}
impl CryptoRunData {
    pub fn enabled(&self) -> bool {
        match self {
            CryptoRunData::AttachLogsMode { enabled, .. } => *enabled,
            CryptoRunData::ExecMode { enabled, .. } => *enabled,
        }
    }

    pub fn has_sandbox_id(&self) -> bool {
        match self {
            CryptoRunData::AttachLogsMode { has_sandbox_id, .. } => *has_sandbox_id,
            CryptoRunData::ExecMode { .. } => false,
        }
    }

    pub fn set_has_sandbox_id(&mut self, value: bool) {
        match self {
            CryptoRunData::AttachLogsMode { has_sandbox_id, .. } => {
                *has_sandbox_id = value;
            }
            _ => {
                // optionally ignore or panic
            }
        }
    }

    pub fn as_attachlogs_mode(&self) -> Option<(&bool, &bool, &Vec<u8>, &Vec<u8>)> {
        match self {
            CryptoRunData::AttachLogsMode {
                enabled,
                has_sandbox_id,
                in_key,
                out_key,
            } => Some((enabled, has_sandbox_id, in_key, out_key)),
            _ => None,
        }
    }

    pub fn as_exec_mode(&self) -> Option<(&Vec<String>, &bool, &Vec<u8>, &Vec<u8>)> {
        match self {
            CryptoRunData::ExecMode {
                args,
                enabled,
                in_key,
                out_key,
            } => Some((args, enabled, in_key, out_key)),
            _ => None,
        }
    }
}

impl Process {
    pub fn new(
        logger: &Logger,
        ocip: &OCIProcess,
        id: &str,
        init: bool,
        pipe_size: i32,
        proc_io: Option<ProcessIo>,
    ) -> Result<Self> {
        let logger = logger.new(o!("subsystem" => "process"));
        let (exit_tx, exit_rx) = tokio::sync::watch::channel(false);

        let mut p = Process {
            exec_id: String::from(id),
            stdin: None,
            stdout: None,
            stderr: None,
            exit_tx: Some(exit_tx),
            exit_rx: Some(exit_rx),
            extra_files: Vec::new(),
            tty: ocip.terminal().unwrap_or_default(),
            term_master: None,
            parent_stdin: None,
            parent_stdout: None,
            parent_stderr: None,
            init,
            pid: -1,
            exit_code: 0,
            exit_watchers: Vec::new(),
            oci: ocip.clone(),
            logger: logger.clone(),
            term_exit_notifier: Arc::new(Notify::new()),
            readers: HashMap::new(),
            writers: HashMap::new(),
            proc_io,
            crypto_run_data: None,
            #[cfg(feature = "cc-op")]
            stream_processor: None,
        };

        info!(logger, "before create console socket!");

        if !p.tty {
            info!(logger, "created console socket!");

            let (stdin, pstdin) = unistd::pipe2(OFlag::O_CLOEXEC)?;
            p.parent_stdin = Some(pstdin.into_raw_fd());
            p.stdin = Some(stdin.into_raw_fd());

            // Make sure the parent stdin writer be inserted into
            // p.writes hashmap, thus the cleanup_process_stream can
            // cleanup and close the parent stdin fd.
            let _ = p.get_writer(StreamType::ParentStdin);

            // These pipes are necessary as the stdout/stderr of the child process
            // cannot be a socket. Otherwise, some images relying on the /dev/stdout(stderr)
            // and /proc/self/fd/1(2) will fail to boot as opening an existing socket
            // is forbidden by the Linux kernel.
            let (pstdout, stdout) = create_extended_pipe(OFlag::O_CLOEXEC, pipe_size)?;
            p.parent_stdout = Some(pstdout);
            p.stdout = Some(stdout);

            let (pstderr, stderr) = create_extended_pipe(OFlag::O_CLOEXEC, pipe_size)?;
            p.parent_stderr = Some(pstderr);
            p.stderr = Some(stderr);
        }
        Ok(p)
    }

    pub fn notify_term_close(&mut self) {
        let notify = self.term_exit_notifier.clone();
        notify.notify_waiters();
    }

    pub async fn close_stdin(&mut self) {
        close_process_stream!(self, term_master, TermMaster);
        close_process_stream!(self, parent_stdin, ParentStdin);
    }

    pub fn cleanup_process_stream(&mut self) {
        if let Some(proc_io) = self.proc_io.take() {
            drop(proc_io);

            return;
        }

        // legacy io mode
        close_process_stream!(self, parent_stdin, ParentStdin);
        close_process_stream!(self, parent_stdout, ParentStdout);
        close_process_stream!(self, parent_stderr, ParentStderr);
        close_process_stream!(self, term_master, TermMaster);

        self.notify_term_close();
    }

    fn get_fd(&self, stream_type: &StreamType) -> Option<RawFd> {
        match stream_type {
            StreamType::Stdin => self.stdin,
            StreamType::Stdout => self.stdout,
            StreamType::Stderr => self.stderr,
            StreamType::TermMaster => self.term_master,
            StreamType::ParentStdin => self.parent_stdin,
            StreamType::ParentStdout => self.parent_stdout,
            StreamType::ParentStderr => self.parent_stderr,
        }
    }

    fn get_stream_and_store(&mut self, stream_type: StreamType) -> Option<(Reader, Writer)> {
        let fd = self.get_fd(&stream_type)?;
        let stream = PipeStream::from_fd(fd);

        let (reader, writer) = split(stream);
        let reader = Arc::new(Mutex::new(reader));
        let writer = Arc::new(Mutex::new(writer));

        self.readers.insert(stream_type.clone(), reader.clone());
        self.writers.insert(stream_type, writer.clone());

        Some((reader, writer))
    }

    pub fn get_reader(&mut self, stream_type: StreamType) -> Option<Reader> {
        if let Some(reader) = self.readers.get(&stream_type) {
            return Some(reader.clone());
        }

        let (reader, _) = self.get_stream_and_store(stream_type)?;
        Some(reader)
    }

    pub fn get_writer(&mut self, stream_type: StreamType) -> Option<Writer> {
        if let Some(writer) = self.writers.get(&stream_type) {
            return Some(writer.clone());
        }

        let (_, writer) = self.get_stream_and_store(stream_type)?;
        Some(writer)
    }

    fn close_stream(&mut self, stream_type: StreamType) {
        let _ = self.readers.remove(&stream_type);
        let _ = self.writers.remove(&stream_type);
    }
}

fn create_extended_pipe(flags: OFlag, pipe_size: i32) -> Result<(RawFd, RawFd)> {
    let (r, w) = unistd::pipe2(flags)?;
    if pipe_size > 0 {
        fcntl(&w, FcntlArg::F_SETPIPE_SZ(pipe_size))?;
    }
    Ok((r.into_raw_fd(), w.into_raw_fd()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn get_pipe_max_size() -> i32 {
        fs::read_to_string("/proc/sys/fs/pipe-max-size")
            .unwrap()
            .trim()
            .parse::<i32>()
            .unwrap()
    }

    fn get_pipe_size(fd: RawFd) -> i32 {
        // Adapt the raw fd returned by production code without taking ownership in this test.
        let fd = unsafe { std::os::fd::BorrowedFd::borrow_raw(fd) };
        fcntl(fd, FcntlArg::F_GETPIPE_SZ).unwrap()
    }

    #[test]
    fn test_create_extended_pipe() {
        // Test the default
        let (_r, _w) = create_extended_pipe(OFlag::O_CLOEXEC, 0).unwrap();

        // Test setting to the max size
        let max_size = get_pipe_max_size();
        let (_, w) = create_extended_pipe(OFlag::O_CLOEXEC, max_size).unwrap();
        let actual_size = get_pipe_size(w);
        assert_eq!(max_size, actual_size);
    }

    #[tokio::test]
    async fn test_process() {
        let id = "abc123rgb";
        let init = true;
        let process = Process::new(
            &Logger::root(slog::Discard, o!("source" => "unit-test")),
            &OCIProcess::default(),
            id,
            init,
            32,
            None,
        );

        let mut process = process.unwrap();
        assert_eq!(process.exec_id, id);
        assert_eq!(process.init, init);

        // -1 by default
        assert_eq!(process.pid, -1);
        // signal to every process in the process
        // group of the calling process.
        process.pid = 0;
        assert!(process.signal(libc::SIGCONT).is_ok());
    }
}
