// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    io::{self, BufRead, BufReader, Read, Write},
    os::unix::{
        io::{AsRawFd, RawFd},
        net::UnixStream,
    },
    path::PathBuf,
    time::Duration,
};

use anyhow::anyhow;
use glob::glob;
use serde::{Deserialize, Serialize};
use slog::debug;
use vmm_sys_util::terminal::Terminal;

const KATA_RUN_PATH: &str = "/run/kata";
const KATA_HYBRID_VSOCK: &str = "kata.hvsock";
const CONNECT_CMD: &str = "CONNECT";
const OK_CMD: &str = "OK";

// 5 seconds
const KATA_AGENT_VSOCK_TIMEOUT: u64 = 5;

// Convenience macro to obtain the scope logger
#[macro_export]
macro_rules! sl {
    () => {
        slog_scope::logger()
    };
}

#[derive(Debug)]
pub enum Error {
    EpollWait(io::Error),
    EpollCreate(io::Error),
    EpollAdd(io::Error),
    SocketWrite(io::Error),
    StdioErr(io::Error),
}

type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server_address: String,
    pub hybrid_vsock: bool,
    pub hybrid_vsock_port: u64,
    pub bundle_dir: String,
    pub interactive: bool,
}

enum EpollDispatch {
    Stdin,
    ServerSock,
}

struct EpollContext {
    epoll_raw_fd: RawFd,
    stdin_index: u64,
    dispatch_table: Vec<EpollDispatch>,
    stdin_handle: io::Stdin,
    debug_console_sock: Option<UnixStream>,
}

impl EpollContext {
    fn new() -> Result<Self> {
        let epoll_raw_fd = epoll::create(true).map_err(Error::EpollCreate)?;
        let dispatch_table = Vec::with_capacity(2);
        let stdin_index = 0;

        Ok(EpollContext {
            epoll_raw_fd,
            stdin_index,
            dispatch_table,
            stdin_handle: io::stdin(),
            debug_console_sock: None,
        })
    }

    fn enable_debug_console_sock(&mut self, sock: UnixStream) -> Result<()> {
        let dispatch_index = self.dispatch_table.len() as u64;
        epoll::ctl(
            self.epoll_raw_fd,
            epoll::ControlOptions::EPOLL_CTL_ADD,
            sock.as_raw_fd(),
            epoll::Event::new(epoll::Events::EPOLLIN, dispatch_index),
        )
        .map_err(Error::EpollAdd)?;

        self.dispatch_table.push(EpollDispatch::ServerSock);
        self.debug_console_sock = Some(sock);

        Ok(())
    }

    fn enable_stdin_event(&mut self) -> Result<()> {
        let stdin_index = self.dispatch_table.len() as u64;
        epoll::ctl(
            self.epoll_raw_fd,
            epoll::ControlOptions::EPOLL_CTL_ADD,
            libc::STDIN_FILENO,
            epoll::Event::new(epoll::Events::EPOLLIN, stdin_index),
        )
        .map_err(Error::EpollAdd)?;

        self.stdin_index = stdin_index;
        self.dispatch_table.push(EpollDispatch::Stdin);

        Ok(())
    }

    fn do_exit(&self) {
        self.stdin_handle
            .lock()
            .set_canon_mode()
            .expect("Fail to set stdin to RAW mode");
    }

    fn process_handler(&mut self) -> Result<()> {
        const EPOLL_EVENTS_LEN: usize = 16;
        let mut events = vec![epoll::Event::new(epoll::Events::empty(), 0); EPOLL_EVENTS_LEN];

        let epoll_raw_fd = self.epoll_raw_fd;
        let debug_console_sock = self.debug_console_sock.as_mut().unwrap();

        loop {
            let num_events =
                epoll::wait(epoll_raw_fd, -1, &mut events[..]).map_err(Error::EpollWait)?;

            for event in events.iter().take(num_events) {
                let dispatch_index = event.data as usize;
                match self.dispatch_table[dispatch_index] {
                    EpollDispatch::Stdin => {
                        let mut out = [0u8; 128];
                        let stdin_lock = self.stdin_handle.lock();
                        match stdin_lock.read_raw(&mut out[..]) {
                            Ok(0) => {
                                return Ok(());
                            }
                            Err(_e) => {
                                return Ok(());
                            }
                            Ok(count) => {
                                debug_console_sock
                                    .write(&out[..count])
                                    .map_err(Error::SocketWrite)?;
                            }
                        }
                    }
                    EpollDispatch::ServerSock => {
                        let mut out = [0u8; 128];
                        match debug_console_sock.read(&mut out[..]) {
                            Ok(0) => {
                                return Ok(());
                            }
                            Err(_e) => {
                                println!("error while reading server");
                                return Ok(());
                            }
                            Ok(count) => {
                                io::stdout()
                                    .write_all(&out[..count])
                                    .map_err(Error::StdioErr)?;
                                io::stdout().flush().map_err(Error::StdioErr)?;
                            }
                        }
                    }
                }
            }
        }
    }
}

fn setup_hybrid_vsock(
    stdin_handle: io::Stdin,
    mut stream: &UnixStream,
    hybrid_vsock_port: u64,
) -> anyhow::Result<()> {
    let msg = format!("{} {}\n", CONNECT_CMD, hybrid_vsock_port);

    stream.set_read_timeout(Some(Duration::new(KATA_AGENT_VSOCK_TIMEOUT, 0)))?;
    stream.set_write_timeout(Some(Duration::new(KATA_AGENT_VSOCK_TIMEOUT, 0)))?;

    stream.write_all(msg.as_bytes())?;

    // Now, see if we get the expected response
    let stream_reader = stream.try_clone()?;
    let mut reader = BufReader::new(&stream_reader);

    let mut msg = String::new();
    reader.read_line(&mut msg)?;
    if msg.is_empty() {
        stdin_handle
            .lock()
            .set_canon_mode()
            .expect("Fail to set stdin to Cannon mode in setup_hybrid_vsock.");

        return Err(anyhow!("stream reader get message is empty"));
    }

    if msg.starts_with(OK_CMD) {
        let response = msg
            .strip_prefix(OK_CMD)
            .ok_or(format!("invalid response: {:?}", msg))
            .map_err(|e| anyhow!(e))?
            .trim();
        debug!(sl!(), "Hybrid Vsock host-side port: {:?}", response);
        // Unset the timeout in order to turn the sokect to bloking mode.
        stream.set_read_timeout(None)?;
        stream.set_write_timeout(None)?;
    } else {
        stdin_handle
            .lock()
            .set_canon_mode()
            .expect("Fail to set stdin to Canon mode in starts_with");

        return Err(anyhow!(
            "failed to setup Hybrid Vsock connection: response was: {:?}",
            msg
        ));
    }

    Ok(())
}

fn create_ttrpc_socket(
    stdin_handle: io::Stdin,
    server_addr: String,
    hybrid_vsock_port: u64,
) -> anyhow::Result<UnixStream> {
    let stream = match UnixStream::connect(server_addr) {
        Ok(s) => s,
        Err(e) => {
            return Err(anyhow!(e).context("failed to create named UNIX Domain stream socket"))
        }
    };

    // when vport > 0, setup hvsock.
    if hybrid_vsock_port > 0 {
        setup_hybrid_vsock(stdin_handle, &stream, hybrid_vsock_port)?
    }

    Ok(stream)
}

fn debug_console_uds(sid: &str) -> PathBuf {
    // we use pattern to get the full sandbox run path,
    // eg. pattern: `/run/kata/02abcd*`. As sandbox_id is unique,
    // so iterator will do once, and the entry will be the path we want.
    let sandbox_id = format!("{}{}", sid, "*");

    // ${KATA_RUN_PATH}/root/{SANDBOX_ID}/kata.hvsock will be the path.
    let sandbox_path: String = [
        KATA_RUN_PATH,
        sandbox_id.as_str(),
        "root",
        KATA_HYBRID_VSOCK,
    ]
    .join("/");

    glob(sandbox_path.as_str())
        .unwrap()
        .filter_map(std::result::Result::ok)
        .last()
        .unwrap_or_else(|| PathBuf::from(sandbox_path.as_str()))
}

pub fn do_run_exec(sandbox_id: &str, debug_port: u64) -> anyhow::Result<()> {
    let server_addr = debug_console_uds(sandbox_id)
        .into_os_string()
        .into_string()
        .unwrap();

    if !PathBuf::from(server_addr.clone()).exists() {
        return Err(anyhow!("sandbox with run path {:?} not found", server_addr));
    }

    let mut epoll_context = EpollContext::new().expect("Fail to create epoll");
    let stdin_handle = io::stdin();
    epoll_context
        .enable_stdin_event()
        .expect("Fail to enable stdin");

    let sock_addr: UnixStream = {
        println!("Hybrid Vsock Mode");
        stdin_handle
            .lock()
            .set_raw_mode()
            .expect("Fail to set stdin to RAW mode");

        // by kata agent hvsock
        create_ttrpc_socket(stdin_handle, server_addr, debug_port)?
    };

    epoll_context
        .enable_debug_console_sock(sock_addr)
        .expect("Fail to connect to the server");

    epoll_context.process_handler().unwrap_or_default();
    epoll_context.do_exit();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::os::unix::io::AsRawFd;
    use std::path::PathBuf;

    #[test]
    fn test_debug_console_uds() {
        let sandbox_id = "0x01-test-kata-exec-001";
        let sid = "0x01";

        let sandbox01 = [KATA_RUN_PATH, sandbox_id, "root", KATA_HYBRID_VSOCK].join("/");
        let _f = File::create(sandbox01)?;

        let target_path = debug_console_uds(sid);
        assert_eq!(target_path, PathBuf::from(sandbox01));

        fs::remove_file(sandbox01)?;
    }
}
