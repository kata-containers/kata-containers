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

use nix::sys::select::*;
use nix::sys::socket::*;
use nix::unistd::close;
use protobuf::{CodedInputStream, CodedOutputStream, Message};
use std::collections::HashMap;
use std::os::unix::io::RawFd;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::channel::{
    read_message, write_message, MessageHeader, MESSAGE_TYPE_REQUEST, MESSAGE_TYPE_RESPONSE,
};
use crate::error::{Error, Result};
use crate::ttrpc::{Code, Request, Response};

#[derive(Clone)]
pub struct Client {
    fd: RawFd,
    sender_tx: mpsc::Sender<(Vec<u8>, mpsc::SyncSender<Result<Vec<u8>>>)>,
    client_close: Arc<ClientClose>,
}

impl Client {
    /// Initialize a new [`Client`].
    pub fn new(fd: RawFd) -> Client {
        let (sender_tx, rx): (
            mpsc::Sender<(Vec<u8>, mpsc::SyncSender<Result<Vec<u8>>>)>,
            mpsc::Receiver<(Vec<u8>, mpsc::SyncSender<Result<Vec<u8>>>)>,
        ) = mpsc::channel();

        let (recver_fd, close_fd) = socketpair(
            AddressFamily::Unix,
            SockType::Stream,
            None,
            SockFlag::empty(),
        )
        .unwrap();
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
                    recver_tx.send(Err(e)).unwrap();
                }
            }
            trace!("Sender quit");
        });

        //Recver
        let recver_map = recver_map_orig.clone();
        thread::spawn(move || {
            let bigfd = {
                if fd > recver_fd {
                    fd + 1
                } else {
                    recver_fd + 1
                }
            };
            loop {
                let mut rs = FdSet::new();
                rs.insert(recver_fd);
                rs.insert(fd);
                select(bigfd, Some(&mut rs), None, None, None).unwrap();
                if rs.contains(recver_fd) {
                    break;
                } else if !rs.contains(fd) {
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
                            break;
                        }
                        _ => {
                            trace!("Others error {:?}", x);
                            continue;
                        }
                    },
                };
                let mut map = recver_map.lock().unwrap();
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
                        .unwrap();
                    continue;
                }

                recver_tx.send(Ok(buf)).unwrap();

                map.remove(&mh.stream_id);
            }
            trace!("Recver quit");
        });

        Client {
            fd,
            sender_tx,
            client_close,
        }
    }
    pub fn request(&self, req: Request) -> Result<Response> {
        let mut buf = Vec::with_capacity(req.compute_size() as usize);
        let mut s = CodedOutputStream::vec(&mut buf);
        req.write_to(&mut s).map_err(err_to_Others!(e, ""))?;
        s.flush().map_err(err_to_Others!(e, ""))?;

        let (tx, rx) = mpsc::sync_channel(0);

        self.sender_tx
            .send((buf, tx))
            .map_err(err_to_Others!(e, "Send packet to sender error "))?;
        let result = rx
            .recv()
            .map_err(err_to_Others!(e, "Recive packet from recver error "))?;

        let buf = result?;
        let mut s = CodedInputStream::from_bytes(&buf);
        let mut res = Response::new();
        res.merge_from(&mut s)
            .map_err(err_to_Others!(e, "Unpack response error "))?;

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

#[macro_export]
macro_rules! client_request {
    ($self: ident, $req: ident, $timeout_nano: ident, $server: expr, $method: expr, $cres: ident) => {
        let mut creq = ::ttrpc::Request::new();
        creq.set_service($server.to_string());
        creq.set_method($method.to_string());
        creq.set_timeout_nano($timeout_nano);
        creq.payload.reserve($req.compute_size() as usize);
        let mut s = CodedOutputStream::vec(&mut creq.payload);
        $req.write_to(&mut s)
            .map_err(::ttrpc::Err_to_Others!(e, ""))?;
        s.flush().map_err(::ttrpc::Err_to_Others!(e, ""))?;

        let res = $self.client.request(creq)?;
        let mut s = CodedInputStream::from_bytes(&res.payload);
        $cres
            .merge_from(&mut s)
            .map_err(::ttrpc::Err_to_Others!(e, "Unpack get error "))?;
    };
}
