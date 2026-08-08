// Copyright (c) 2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
// Description:
// Implementation of entering into guest VM by debug console.
// Ensure that `kata-debug-port` is consistent with the port
// set in the configuration.

use std::{
    io::{self, Read, Write},
    os::unix::{
        io::{AsRawFd, RawFd},
        net::UnixStream,
    },
};

use anyhow::Context;
use slog::{error, o};
use vmm_sys_util::terminal::Terminal;

use crate::args::ExecArguments;
use crate::debug_console::{self, Session};

const EPOLL_EVENTS_LEN: usize = 16;

type Result<T> = std::result::Result<T, Error>;

// Convenience macro to obtain the scope logger
#[macro_export]
macro_rules! sl {
    () => {
        slog_scope::logger().new(o!("subsystem" => "exec_ops"))
    };
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum Error {
    EpollWait(io::Error),
    EpollCreate(io::Error),
    EpollAdd(io::Error),
    SocketWrite(io::Error),
    StdioErr(io::Error),
}

#[derive(Debug, PartialEq)]
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
        let dispatch_table = Vec::new();
        let stdin_index = 0;

        Ok(EpollContext {
            epoll_raw_fd,
            stdin_index,
            dispatch_table,
            stdin_handle: io::stdin(),
            debug_console_sock: None,
        })
    }

    fn init_debug_console_sock(&mut self, sock: UnixStream) -> Result<()> {
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

    fn do_process_handler(&mut self) -> Result<()> {
        let mut events = [epoll::Event::new(epoll::Events::empty(), 0); EPOLL_EVENTS_LEN];

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
                            Err(e) => {
                                error!(sl!(), "errno {:?} while reading stdin", e);
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
                            Err(e) => {
                                error!(sl!(), "errno {:?} while reading server", e);
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

fn do_run_exec(sock_stream: UnixStream) -> anyhow::Result<()> {
    let mut epoll_context = EpollContext::new().expect("create epoll context");
    epoll_context
        .enable_stdin_event()
        .expect("enable stdin event");
    epoll_context
        .init_debug_console_sock(sock_stream)
        .expect("enable debug console sock");

    let stdin_handle = io::stdin();
    stdin_handle.lock().set_raw_mode().expect("set raw mode");

    epoll_context
        .do_process_handler()
        .expect("do process handler");
    epoll_context.do_exit();

    Ok(())
}

// Run a single command in the guest and relay what it printed, rather than
// handing the console shell a terminal. The console carries one stream, so the
// command's stderr arrives interleaved on stdout.
fn do_run_command(sock_stream: UnixStream, argv: &[String]) -> anyhow::Result<i32> {
    let cmd = argv
        .iter()
        .map(|arg| debug_console::shell_quote(arg))
        .collect::<Vec<_>>()
        .join(" ");

    let mut session = Session::new(sock_stream);

    let stdout = io::stdout();
    let mut out = stdout.lock();
    let status = session.run(&cmd, &mut out)?;
    out.flush().context("flush stdout")?;

    session.close();

    Ok(status)
}

// kata-ctl handle exec command starts here.
pub fn handle_exec(exec_args: ExecArguments) -> anyhow::Result<i32> {
    let sock_stream = debug_console::connect(&exec_args.sandbox_id, exec_args.vport)?;

    if exec_args.command.is_empty() {
        do_run_exec(sock_stream)?;
        return Ok(0);
    }

    do_run_command(sock_stream, &exec_args.command)
}

#[cfg(test)]
mod tests {
    use super::*;
    use micro_http::HttpServer;

    #[test]
    fn test_epoll_context_methods() {
        let kata_hybrid_addr = "/tmp/kata_hybrid_vsock01.hvsock";
        std::fs::remove_file(kata_hybrid_addr).unwrap_or_default();
        let mut server = HttpServer::new(kata_hybrid_addr).unwrap();
        server.start_server().unwrap();
        let sock_addr: UnixStream = UnixStream::connect(kata_hybrid_addr).unwrap();
        let mut epoll_ctx = EpollContext::new().expect("epoll context");
        epoll_ctx
            .init_debug_console_sock(sock_addr)
            .expect("enable debug console sock");
        assert_eq!(epoll_ctx.stdin_index, 0);
        assert!(epoll_ctx.debug_console_sock.is_some());
        assert_eq!(epoll_ctx.dispatch_table[0], EpollDispatch::ServerSock);
        assert_eq!(epoll_ctx.dispatch_table.len(), 1);

        epoll_ctx.enable_stdin_event().expect("enable stdin event");
        assert_eq!(epoll_ctx.stdin_index, 1);
        assert_eq!(epoll_ctx.dispatch_table[1], EpollDispatch::Stdin);
        assert_eq!(epoll_ctx.dispatch_table.len(), 2);
        std::fs::remove_file(kata_hybrid_addr).unwrap_or_default();
    }
}
