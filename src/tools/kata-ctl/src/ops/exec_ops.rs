// Copyright (c) 2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
// Description:
// Implementation of entering into guest VM by debug console.
// Ensure that `kata-debug-port` is consistent with the port
// set in the configuration.

use std::{
    io::{self, BufRead, BufReader, Read, Write},
    os::unix::{
        io::{AsRawFd, FromRawFd, RawFd},
        net::UnixStream,
    },
    time::Duration,
};

use anyhow::{anyhow, Context};
use nix::sys::socket::{connect, socket, AddressFamily, SockFlag, SockType, VsockAddr};
use reqwest::StatusCode;
use slog::{debug, error, o};
use vmm_sys_util::terminal::Terminal;

use crate::args::ExecArguments;
use shim_interface::shim_mgmt::{client::MgmtClient, AGENT_URL};

use crate::utils::TIMEOUT;

const CMD_CONNECT: &str = "CONNECT";
const CMD_OK: &str = "OK";
const SCHEME_VSOCK: &str = "VSOCK";
const SCHEME_HYBRID_VSOCK: &str = "HVSOCK";

const EPOLL_EVENTS_LEN: usize = 16;
const KATA_AGENT_VSOCK_TIMEOUT: u64 = 5;

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

trait SockHandler {
    fn setup_sock(&self) -> anyhow::Result<UnixStream>;
}

struct VsockConfig {
    sock_cid: u32,
    sock_port: u32,
}

impl VsockConfig {
    fn new(sock_cid: u32, sock_port: u32) -> VsockConfig {
        VsockConfig {
            sock_cid,
            sock_port,
        }
    }
}

impl SockHandler for VsockConfig {
    fn setup_sock(&self) -> anyhow::Result<UnixStream> {
        let sock_addr = VsockAddr::new(self.sock_cid, self.sock_port);

        // Create socket fd
        let vsock_fd = socket(
            AddressFamily::Vsock,
            SockType::Stream,
            SockFlag::SOCK_CLOEXEC,
            None,
        )
        .context("create vsock socket")?;

        // Wrap the socket fd in UnixStream, so that it is closed
        // when anything fails.
        let stream = unsafe { UnixStream::from_raw_fd(vsock_fd) };
        // Connect the socket to vsock server.
        connect(stream.as_raw_fd(), &sock_addr)
            .with_context(|| format!("failed to connect to server {:?}", &sock_addr))?;

        Ok(stream)
    }
}

struct HvsockConfig {
    sock_addr: String,
    sock_port: u32,
}

impl HvsockConfig {
    fn new(sock_addr: String, sock_port: u32) -> Self {
        HvsockConfig {
            sock_addr,
            sock_port,
        }
    }
}

impl SockHandler for HvsockConfig {
    fn setup_sock(&self) -> anyhow::Result<UnixStream> {
        let mut stream = match UnixStream::connect(self.sock_addr.clone()) {
            Ok(s) => s,
            Err(e) => return Err(anyhow!(e).context("failed to create UNIX Stream socket")),
        };

        // Ensure the Unix Stream directly connects to the real VSOCK server which
        // the Kata agent is listening to in the VM.
        {
            let test_msg = format!("{} {}\n", CMD_CONNECT, self.sock_port);

            stream.set_read_timeout(Some(Duration::new(KATA_AGENT_VSOCK_TIMEOUT, 0)))?;
            stream.set_write_timeout(Some(Duration::new(KATA_AGENT_VSOCK_TIMEOUT, 0)))?;

            stream.write_all(test_msg.as_bytes())?;
            // Now, see if we get the expected response
            let stream_reader = stream.try_clone()?;
            let mut reader = BufReader::new(&stream_reader);
            let mut msg = String::new();

            reader.read_line(&mut msg)?;
            if msg.is_empty() {
                return Err(anyhow!(
                    "stream reader get message is empty with port: {:?}",
                    self.sock_port
                ));
            }

            // Expected response message returned was successful.
            if msg.starts_with(CMD_OK) {
                let response = msg
                    .strip_prefix(CMD_OK)
                    .ok_or(format!("invalid response: {:?}", msg))
                    .map_err(|e| anyhow!(e))?
                    .trim();
                debug!(sl!(), "Hybrid Vsock host-side port: {:?}", response);
                // Unset the timeout in order to turn the sokect to bloking mode.
                stream.set_read_timeout(None)?;
                stream.set_write_timeout(None)?;
            } else {
                return Err(anyhow!(
                    "failed to setup Hybrid Vsock connection: {:?}",
                    msg
                ));
            }
        }

        Ok(stream)
    }
}

fn setup_client(server_url: String, dbg_console_port: u32) -> anyhow::Result<UnixStream> {
    // server address format: scheme://[cid|/x/domain.sock]:port
    let url_fields: Vec<&str> = server_url.split("://").collect();
    if url_fields.len() != 2 {
        return Err(anyhow!("invalid URI"));
    }

    let scheme = url_fields[0].to_uppercase();
    let sock_addr: Vec<&str> = url_fields[1].split(':').collect();
    if sock_addr.len() != 2 {
        return Err(anyhow!("invalid VSOCK server address URI"));
    }

    match scheme.as_str() {
        // Hybrid Vsock: hvsock://<path>:<port>.
        // Example: "hvsock:///x/y/z/kata.hvsock:port"
        // Firecracker/Dragonball/CLH implements the hybrid vsock device model.
        SCHEME_HYBRID_VSOCK => {
            let hvsock_path = sock_addr[0].to_string();
            if hvsock_path.is_empty() {
                return Err(anyhow!("hvsock path cannot be empty"));
            }

            let hvsock = HvsockConfig::new(hvsock_path, dbg_console_port);
            hvsock.setup_sock().context("set up hvsock")
        }
        // Vsock: vsock://<cid>:<port>
        // Example: "vsock://31513974:1024"
        // Qemu using the Vsock device model.
        SCHEME_VSOCK => {
            let sock_cid: u32 = match sock_addr[0] {
                "-1" | "" => libc::VMADDR_CID_ANY,
                _ => match sock_addr[0].parse::<u32>() {
                    Ok(cid) => cid,
                    Err(e) => return Err(anyhow!("vsock addr CID is INVALID: {:?}", e)),
                },
            };

            let vsock = VsockConfig::new(sock_cid, dbg_console_port);
            vsock.setup_sock().context("set up vsock")
        }
        // Others will be INVALID URI.
        _ => Err(anyhow!("invalid URI scheme: {:?}", scheme)),
    }
}

async fn get_agent_socket(sandbox_id: &str) -> anyhow::Result<String> {
    let shim_client = MgmtClient::new(sandbox_id, Some(TIMEOUT))?;

    // get agent sock from body when status code is OK.
    let response = shim_client.get(AGENT_URL).await?;
    let status = response.status();
    if status != StatusCode::OK {
        return Err(anyhow!("shim client get connection failed: {:?} ", status));
    }

    let body = hyper::body::to_bytes(response.into_body()).await?;
    let agent_sock = String::from_utf8(body.to_vec())?;

    Ok(agent_sock)
}

fn get_server_socket(sandbox_id: &str) -> anyhow::Result<String> {
    let server_url = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(get_agent_socket(sandbox_id))
        .context("get connection vsock")?;

    Ok(server_url)
}

fn do_run_exec(sandbox_id: &str, dbg_console_vport: u32) -> anyhow::Result<()> {
    let server_url = get_server_socket(sandbox_id).context("get debug console socket URL")?;
    if server_url.is_empty() {
        return Err(anyhow!("server url is empty."));
    }
    let sock_stream = setup_client(server_url, dbg_console_vport)?;

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

// kata-ctl handle exec command starts here.
pub fn handle_exec(exec_args: ExecArguments) -> anyhow::Result<()> {
    do_run_exec(exec_args.sandbox_id.as_str(), exec_args.vport)?;

    Ok(())
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

    #[test]
    fn test_setup_hvsock_failed() {
        let kata_hybrid_addr = "/tmp/kata_hybrid_vsock02.hvsock";
        let hybrid_sock_addr = "hvsock:///tmp/kata_hybrid_vsock02.hvsock:1024";
        std::fs::remove_file(kata_hybrid_addr).unwrap_or_default();
        let dbg_console_port: u32 = 1026;
        let mut server = HttpServer::new(kata_hybrid_addr).unwrap();
        server.start_server().unwrap();

        let stream = setup_client(hybrid_sock_addr.to_string(), dbg_console_port);
        assert!(stream.is_err());
        std::fs::remove_file(kata_hybrid_addr).unwrap_or_default();
    }

    #[test]
    fn test_setup_vsock_client_failed() {
        let hybrid_sock_addr = "hvsock://8:1024";
        let dbg_console_port: u32 = 1026;
        let stream = setup_client(hybrid_sock_addr.to_string(), dbg_console_port);
        assert!(stream.is_err());
    }
}
