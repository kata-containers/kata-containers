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
use tokio::io::{split, AsyncReadExt, ReadHalf, WriteHalf};
use tokio::sync::Notify;
use tokio::sync::{watch, Mutex};

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

#[derive(Debug)]
struct OutputReplay {
    last_session_id: String,
    last_request_id: u64,
    last_response: Vec<u8>,
}

impl OutputReplay {
    fn response(&self, session_id: &str, request_id: u64) -> Option<Vec<u8>> {
        (!session_id.is_empty()
            && self.last_session_id == session_id
            && self.last_request_id == request_id)
            .then(|| self.last_response.clone())
    }

    fn store(&mut self, session_id: &str, request_id: u64, response: &[u8]) {
        if session_id.is_empty() {
            return;
        }

        self.last_session_id = session_id.to_string();
        self.last_request_id = request_id;
        self.last_response.clear();
        self.last_response.extend_from_slice(response);
    }
}

#[derive(Debug)]
pub struct ProcessOutputReader {
    reader: ReadHalf<PipeStream>,
    replay: OutputReplay,
}

impl ProcessOutputReader {
    fn new(reader: ReadHalf<PipeStream>) -> Self {
        Self {
            reader,
            replay: OutputReplay {
                last_session_id: String::new(),
                last_request_id: 0,
                last_response: Vec::new(),
            },
        }
    }

    pub async fn read_replayable(
        &mut self,
        len: usize,
        session_id: &str,
        request_id: u64,
    ) -> std::io::Result<Vec<u8>> {
        if let Some(response) = self.replay.response(session_id, request_id) {
            return Ok(response);
        }

        let mut response = vec![0_u8; len];
        let count = self.reader.read(&mut response).await?;
        response.truncate(count);

        self.replay.store(session_id, request_id, &response);

        Ok(response)
    }
}

#[cfg(test)]
mod output_replay_tests {
    use super::OutputReplay;

    #[test]
    fn replay_matches_only_the_same_session_and_request() {
        let mut replay = OutputReplay {
            last_session_id: String::new(),
            last_request_id: 0,
            last_response: Vec::new(),
        };

        replay.store("session-a", 1, b"first");

        assert_eq!(replay.response("session-a", 1), Some(b"first".to_vec()));
        assert_eq!(replay.response("session-a", 2), None);
        assert_eq!(replay.response("session-b", 1), None);
    }

    #[test]
    fn empty_session_disables_replay() {
        let mut replay = OutputReplay {
            last_session_id: String::new(),
            last_request_id: 0,
            last_response: Vec::new(),
        };

        replay.store("", 1, b"legacy");

        assert_eq!(replay.response("", 1), None);
        assert!(replay.last_response.is_empty());
    }
}

pub type Reader = Arc<Mutex<ProcessOutputReader>>;
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

    // A whole-VM restore also restores agent RPC tasks that were blocked while
    // reading this process's output. Advancing this generation makes those
    // stale readers stop before the restored process is resumed; the new shim
    // subscribes to the new generation when it reconnects output.
    output_generation_tx: watch::Sender<u64>,
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
        let (output_generation_tx, _) = watch::channel(0);

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
            output_generation_tx,
            readers: HashMap::new(),
            writers: HashMap::new(),
            proc_io,
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

    pub fn subscribe_output_generation(&self) -> watch::Receiver<u64> {
        self.output_generation_tx.subscribe()
    }

    pub fn invalidate_output_readers(&self) {
        self.output_generation_tx
            .send_modify(|generation| *generation = generation.wrapping_add(1));
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
        let reader = Arc::new(Mutex::new(ProcessOutputReader::new(reader)));
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
