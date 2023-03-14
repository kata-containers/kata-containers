// Copyright (c) 2019 Ant Financial
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Sync client of ttrpc.

use nix::sys::socket::*;
use nix::unistd::close;
use protobuf::{CodedInputStream, CodedOutputStream, Message};
use std::collections::HashMap;
use std::os::unix::io::RawFd;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::{io, thread};

#[cfg(target_os = "macos")]
use crate::common::set_fd_close_exec;
use crate::common::{client_connect, MESSAGE_TYPE_REQUEST, MESSAGE_TYPE_RESPONSE, SOCK_CLOEXEC};
use crate::error::{Error, Result};
use crate::proto::{Code, Request, Response};
use crate::sync::channel::{read_message, write_message};
use crate::MessageHeader;
use std::time::Duration;

type Sender = mpsc::Sender<(Vec<u8>, mpsc::SyncSender<Result<Vec<u8>>>)>;
type Receiver = mpsc::Receiver<(Vec<u8>, mpsc::SyncSender<Result<Vec<u8>>>)>;

/// A ttrpc Client (sync).
#[derive(Clone)]
pub struct Client {
    _fd: RawFd,
    sender_tx: Sender,
    _client_close: Arc<ClientClose>,
}

impl Client {
    pub fn connect(sockaddr: &str) -> Result<Client> {
        let fd = unsafe { client_connect(sockaddr)? };
        Ok(Self::new(fd))
    }

    /// Initialize a new [`Client`] from raw file descriptor.
    pub fn new(fd: RawFd) -> Client {
        let (sender_tx, rx): (Sender, Receiver) = mpsc::channel();

        let (recver_fd, close_fd) =
            socketpair(AddressFamily::Unix, SockType::Stream, None, SOCK_CLOEXEC).unwrap();

        // MacOS doesn't support descriptor creation with SOCK_CLOEXEC automically,
        // so there is a chance of leak if fork + exec happens in between of these calls.
        #[cfg(target_os = "macos")]
        {
            set_fd_close_exec(recver_fd).unwrap();
            set_fd_close_exec(close_fd).unwrap();
        }

        let client_close = Arc::new(ClientClose { fd, close_fd });

        let recver_map_orig = Arc::new(Mutex::new(HashMap::new()));

        //Sender
        let recver_map = recver_map_orig.clone();
        thread::spawn(move || {
            let mut stream_id: u32 = 1;
            for (buf, recver_tx) in rx.iter() {
                let current_stream_id = stream_id;
                stream_id += 2;
                //Put current_stream_id and recver_tx to recver_map
                {
                    let mut map = recver_map.lock().unwrap();
                    map.insert(current_stream_id, recver_tx.clone());
                }
                let mh = MessageHeader {
                    length: buf.len() as u32,
                    stream_id: current_stream_id,
                    type_: MESSAGE_TYPE_REQUEST,
                    flags: 0,
                };
                if let Err(e) = write_message(fd, mh, buf) {
                    //Remove current_stream_id and recver_tx to recver_map
                    {
                        let mut map = recver_map.lock().unwrap();
                        map.remove(&current_stream_id);
                    }
                    recver_tx
                        .send(Err(e))
                        .unwrap_or_else(|_e| error!("The request has returned"));
                }
            }
            trace!("Sender quit");
        });

        //Recver
        thread::spawn(move || {
            let mut pollers = vec![
                libc::pollfd {
                    fd: recver_fd,
                    events: libc::POLLIN,
                    revents: 0,
                },
                libc::pollfd {
                    fd,
                    events: libc::POLLIN,
                    revents: 0,
                },
            ];

            loop {
                let returned = unsafe {
                    let pollers: &mut [libc::pollfd] = &mut pollers;
                    libc::poll(
                        pollers as *mut _ as *mut libc::pollfd,
                        pollers.len() as _,
                        -1,
                    )
                };

                if returned == -1 {
                    let err = io::Error::last_os_error();
                    if err.raw_os_error() == Some(libc::EINTR) {
                        continue;
                    }

                    error!("fatal error in process reaper:{}", err);
                    break;
                } else if returned < 1 {
                    continue;
                }

                if pollers[0].revents != 0 {
                    break;
                }

                if pollers[pollers.len() - 1].revents == 0 {
                    continue;
                }

                let mh;
                let buf;
                match read_message(fd) {
                    Ok((x, y)) => {
                        mh = x;
                        buf = y;
                    }
                    Err(x) => match x {
                        Error::Socket(y) => {
                            trace!("Socket error {}", y);
                            let mut map = recver_map_orig.lock().unwrap();
                            for (_, recver_tx) in map.iter_mut() {
                                recver_tx
                                    .send(Err(Error::Socket(format!("socket error {}", y))))
                                    .unwrap_or_else(|e| {
                                        error!("The request has returned error {:?}", e)
                                    });
                            }
                            map.clear();
                            break;
                        }
                        _ => {
                            trace!("Others error {:?}", x);
                            continue;
                        }
                    },
                };
                let mut map = recver_map_orig.lock().unwrap();
                let recver_tx = match map.get(&mh.stream_id) {
                    Some(tx) => tx,
                    None => {
                        debug!("Recver got unknown packet {:?} {:?}", mh, buf);
                        continue;
                    }
                };
                if mh.type_ != MESSAGE_TYPE_RESPONSE {
                    recver_tx
                        .send(Err(Error::Others(format!(
                            "Recver got malformed packet {:?} {:?}",
                            mh, buf
                        ))))
                        .unwrap_or_else(|_e| error!("The request has returned"));
                    continue;
                }

                recver_tx
                    .send(Ok(buf))
                    .unwrap_or_else(|_e| error!("The request has returned"));

                map.remove(&mh.stream_id);
            }

            let _ = close(recver_fd).map_err(|e| {
                warn!(
                    "failed to close recver_fd: {} with error: {:?}",
                    recver_fd, e
                )
            });

            trace!("Recver quit");
        });

        Client {
            _fd: fd,
            sender_tx,
            _client_close: client_close,
        }
    }
    pub fn request(&self, req: Request) -> Result<Response> {
        let mut buf = Vec::with_capacity(req.compute_size() as usize);
        let mut s = CodedOutputStream::vec(&mut buf);
        req.write_to(&mut s).map_err(err_to_others_err!(e, ""))?;
        s.flush().map_err(err_to_others_err!(e, ""))?;

        let (tx, rx) = mpsc::sync_channel(0);

        self.sender_tx
            .send((buf, tx))
            .map_err(err_to_others_err!(e, "Send packet to sender error "))?;

        let result: Result<Vec<u8>>;
        if req.timeout_nano == 0 {
            result = rx
                .recv()
                .map_err(err_to_others_err!(e, "Receive packet from recver error: "))?;
        } else {
            result = rx
                .recv_timeout(Duration::from_nanos(req.timeout_nano as u64))
                .map_err(err_to_others_err!(
                    e,
                    "Receive packet from recver timeout: "
                ))?;
        }

        let buf = result?;
        let mut s = CodedInputStream::from_bytes(&buf);
        let mut res = Response::new();
        res.merge_from(&mut s)
            .map_err(err_to_others_err!(e, "Unpack response error "))?;

        let status = res.get_status();
        if status.get_code() != Code::OK {
            return Err(Error::RpcStatus((*status).clone()));
        }

        Ok(res)
    }
}

struct ClientClose {
    fd: RawFd,
    close_fd: RawFd,
}

impl Drop for ClientClose {
    fn drop(&mut self) {
        close(self.close_fd).unwrap();
        close(self.fd).unwrap();
        trace!("All client is droped");
    }
}
