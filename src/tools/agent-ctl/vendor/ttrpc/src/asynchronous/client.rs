// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use nix::unistd::close;
use protobuf::{CodedInputStream, CodedOutputStream, Message};
use std::collections::HashMap;
use std::os::unix::io::RawFd;
use std::sync::{Arc, Mutex};

use crate::common::{client_connect, MESSAGE_TYPE_RESPONSE};
use crate::error::{Error, Result};
use crate::proto::{Code, Request, Response};

use crate::asynchronous::stream::{receive, to_req_buf};
use crate::r#async::utils;
use tokio::{
    self,
    io::{split, AsyncWriteExt},
    sync::mpsc::{channel, Receiver, Sender},
    sync::Notify,
};

type RequestSender = Sender<(Vec<u8>, Sender<Result<Vec<u8>>>)>;
type RequestReceiver = Receiver<(Vec<u8>, Sender<Result<Vec<u8>>>)>;

type ResponseSender = Sender<Result<Vec<u8>>>;
type ResponseReceiver = Receiver<Result<Vec<u8>>>;

/// A ttrpc Client (async).
#[derive(Clone)]
pub struct Client {
    req_tx: RequestSender,
}

impl Client {
    pub fn connect(sockaddr: &str) -> Result<Client> {
        let fd = unsafe { client_connect(sockaddr)? };
        Ok(Self::new(fd))
    }

    /// Initialize a new [`Client`].
    pub fn new(fd: RawFd) -> Client {
        let stream = utils::new_unix_stream_from_raw_fd(fd);

        let (mut reader, mut writer) = split(stream);
        let (req_tx, mut rx): (RequestSender, RequestReceiver) = channel(100);

        let req_map = Arc::new(Mutex::new(HashMap::new()));
        let req_map2 = req_map.clone();

        let notify = Arc::new(Notify::new());
        let notify2 = notify.clone();

        // Request sender
        tokio::spawn(async move {
            let mut stream_id: u32 = 1;

            while let Some((body, resp_tx)) = rx.recv().await {
                let current_stream_id = stream_id;
                stream_id += 2;

                {
                    let mut map = req_map2.lock().unwrap();
                    map.insert(current_stream_id, resp_tx.clone());
                }

                let buf = to_req_buf(current_stream_id, body);
                if let Err(e) = writer.write_all(&buf).await {
                    error!("write_message got error: {:?}", e);

                    {
                        let mut map = req_map2.lock().unwrap();
                        map.remove(&current_stream_id);
                    }

                    let e = Error::Socket(format!("{:?}", e));
                    resp_tx
                        .send(Err(e))
                        .await
                        .unwrap_or_else(|_e| error!("The request has returned"));

                    break; // The stream is dead, exit the loop.
                }
            }

            // rx.recv will abort when client.req_tx and client is dropped.
            // notify the response-receiver to quit at this time.
            notify.notify_one();
        });

        // Response receiver
        tokio::spawn(async move {
            loop {
                let req_map = req_map.clone();
                tokio::select! {
                    _ = notify2.notified() => {
                        break;
                    }
                    res = receive(&mut reader) => {
                        match res {
                            Ok((header, body)) => {
                                tokio::spawn(async move {
                                    let resp_tx2;
                                    {
                                        let mut map = req_map.lock().unwrap();
                                        let resp_tx = match map.get(&header.stream_id) {
                                            Some(tx) => tx,
                                            None => {
                                                debug!(
                                                    "Receiver got unknown packet {:?} {:?}",
                                                    header, body
                                                );
                                                return;
                                            }
                                        };

                                        resp_tx2 = resp_tx.clone();
                                        map.remove(&header.stream_id); // Forget the result, just remove.
                                    }

                                    if header.type_ != MESSAGE_TYPE_RESPONSE {
                                        resp_tx2
                                            .send(Err(Error::Others(format!(
                                                "Recver got malformed packet {:?} {:?}",
                                                header, body
                                            ))))
                                            .await
                                            .unwrap_or_else(|_e| error!("The request has returned"));
                                        return;
                                    }

                                    resp_tx2.send(Ok(body)).await.unwrap_or_else(|_e| error!("The request has returned"));
                                });
                            }
                            Err(e) => {
                                trace!("error {:?}", e);
                                break;
                            }
                        }
                    }
                };
            }
        });

        Client { req_tx }
    }

    pub async fn request(&self, req: Request) -> Result<Response> {
        let mut buf = Vec::with_capacity(req.compute_size() as usize);
        {
            let mut s = CodedOutputStream::vec(&mut buf);
            req.write_to(&mut s).map_err(err_to_others_err!(e, ""))?;
            s.flush().map_err(err_to_others_err!(e, ""))?;
        }

        let (tx, mut rx): (ResponseSender, ResponseReceiver) = channel(100);
        self.req_tx
            .send((buf, tx))
            .await
            .map_err(|e| Error::Others(format!("Send packet to sender error {:?}", e)))?;

        let result = rx
            .recv()
            .await
            .ok_or_else(|| Error::Others("Recive packet from recver error".to_string()))?;

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
