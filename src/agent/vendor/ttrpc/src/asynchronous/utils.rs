// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::common::{MessageHeader, MESSAGE_TYPE_REQUEST, MESSAGE_TYPE_RESPONSE};
use crate::error::{Error, Result};
use crate::ttrpc::{Request, Response};
use async_trait::async_trait;
use protobuf::Message;
use std::os::unix::io::{FromRawFd, RawFd};
use tokio::net::UnixStream;

/// Handle request in async mode.
#[macro_export]
macro_rules! async_request_handler {
    ($class: ident, $ctx: ident, $req: ident, $server: ident, $req_type: ident, $req_fn: ident) => {
        let mut req = super::$server::$req_type::new();
        {
            let mut s = CodedInputStream::from_bytes(&$req.payload);
            req.merge_from(&mut s)
                .map_err(::ttrpc::Err_to_Others!(e, ""))?;
        }

        let mut res = ::ttrpc::Response::new();
        match $class.service.$req_fn(&$ctx, req).await {
            Ok(rep) => {
                res.set_status(::ttrpc::get_status(::ttrpc::Code::OK, "".to_string()));
                res.payload.reserve(rep.compute_size() as usize);
                let mut s = protobuf::CodedOutputStream::vec(&mut res.payload);
                rep.write_to(&mut s)
                    .map_err(::ttrpc::Err_to_Others!(e, ""))?;
                s.flush().map_err(::ttrpc::Err_to_Others!(e, ""))?;
            }
            Err(x) => match x {
                ::ttrpc::Error::RpcStatus(s) => {
                    res.set_status(s);
                }
                _ => {
                    res.set_status(::ttrpc::get_status(
                        ::ttrpc::Code::UNKNOWN,
                        format!("{:?}", x),
                    ));
                }
            },
        }

        let buf = ::ttrpc::r#async::convert_response_to_buf(res)?;
        return Ok(($ctx.mh.stream_id, buf));
    };
}

/// Send request through async client.
#[macro_export]
macro_rules! async_client_request {
    ($self: ident, $req: ident, $timeout_nano: ident, $server: expr, $method: expr, $cres: ident) => {
        let mut creq = ::ttrpc::Request::new();
        creq.set_service($server.to_string());
        creq.set_method($method.to_string());
        creq.set_timeout_nano($timeout_nano);
        creq.payload.reserve($req.compute_size() as usize);
        {
            let mut s = CodedOutputStream::vec(&mut creq.payload);
            $req.write_to(&mut s)
                .map_err(::ttrpc::Err_to_Others!(e, ""))?;
            s.flush().map_err(::ttrpc::Err_to_Others!(e, ""))?;
        }

        let res = $self.client.request(creq).await?;
        let mut s = CodedInputStream::from_bytes(&res.payload);
        $cres
            .merge_from(&mut s)
            .map_err(::ttrpc::Err_to_Others!(e, "Unpack get error "))?;

        return Ok($cres);
    };
}

/// Trait that implements handler which is a proxy to the desired method (async).
#[async_trait]
pub trait MethodHandler {
    async fn handler(&self, ctx: TtrpcContext, req: Request) -> Result<(u32, Vec<u8>)>;
}

/// The context of ttrpc (async).
#[derive(Debug)]
pub struct TtrpcContext {
    pub fd: std::os::unix::io::RawFd,
    pub mh: MessageHeader,
}

pub fn convert_response_to_buf(res: Response) -> Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(res.compute_size() as usize);
    let mut s = protobuf::CodedOutputStream::vec(&mut buf);
    res.write_to(&mut s).map_err(err_to_others_err!(e, ""))?;
    s.flush().map_err(err_to_others_err!(e, ""))?;

    Ok(buf)
}

pub fn get_response_header_from_body(stream_id: u32, body: &[u8]) -> MessageHeader {
    MessageHeader {
        length: body.len() as u32,
        stream_id,
        type_: MESSAGE_TYPE_RESPONSE,
        flags: 0,
    }
}

pub fn get_request_header_from_body(stream_id: u32, body: &[u8]) -> MessageHeader {
    MessageHeader {
        length: body.len() as u32,
        stream_id,
        type_: MESSAGE_TYPE_REQUEST,
        flags: 0,
    }
}

pub fn new_unix_stream_from_raw_fd(fd: RawFd) -> UnixStream {
    let std_stream: std::os::unix::net::UnixStream;
    unsafe {
        std_stream = std::os::unix::net::UnixStream::from_raw_fd(fd);
    }
    UnixStream::from_std(std_stream).unwrap()
}
