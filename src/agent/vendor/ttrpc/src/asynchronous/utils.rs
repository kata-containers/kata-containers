// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::common::{MessageHeader, MESSAGE_TYPE_REQUEST, MESSAGE_TYPE_RESPONSE};
use crate::error::{get_status, Result};
use crate::proto::{Code, Request, Status};
use async_trait::async_trait;
use protobuf::{CodedInputStream, Message};
use std::collections::HashMap;
use std::os::unix::io::{FromRawFd, RawFd};
use std::result::Result as StdResult;
use tokio::net::UnixStream;

/// Handle request in async mode.
#[macro_export]
macro_rules! async_request_handler {
    ($class: ident, $ctx: ident, $req: ident, $server: ident, $req_type: ident, $req_fn: ident) => {
        let mut req = super::$server::$req_type::new();
        {
            let mut s = CodedInputStream::from_bytes(&$req.payload);
            req.merge_from(&mut s)
                .map_err(::ttrpc::err_to_others!(e, ""))?;
        }

        let mut res = ::ttrpc::Response::new();
        match $class.service.$req_fn(&$ctx, req).await {
            Ok(rep) => {
                res.set_status(::ttrpc::get_status(::ttrpc::Code::OK, "".to_string()));
                res.payload.reserve(rep.compute_size() as usize);
                let mut s = protobuf::CodedOutputStream::vec(&mut res.payload);
                rep.write_to(&mut s)
                    .map_err(::ttrpc::err_to_others!(e, ""))?;
                s.flush().map_err(::ttrpc::err_to_others!(e, ""))?;
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

        let mut buf = Vec::with_capacity(res.compute_size() as usize);
        let mut s = protobuf::CodedOutputStream::vec(&mut buf);
        res.write_to(&mut s).map_err(ttrpc::err_to_others!(e, ""))?;
        s.flush().map_err(ttrpc::err_to_others!(e, ""))?;

        return Ok(($ctx.mh.stream_id, buf));
    };
}

/// Send request through async client.
#[macro_export]
macro_rules! async_client_request {
    ($self: ident, $ctx: ident, $req: ident, $server: expr, $method: expr, $cres: ident) => {
        let mut creq = ::ttrpc::Request::new();
        creq.set_service($server.to_string());
        creq.set_method($method.to_string());
        creq.set_timeout_nano($ctx.timeout_nano);
        let md = ::ttrpc::context::to_pb($ctx.metadata);
        creq.set_metadata(md);
        creq.payload.reserve($req.compute_size() as usize);
        {
            let mut s = CodedOutputStream::vec(&mut creq.payload);
            $req.write_to(&mut s)
                .map_err(::ttrpc::err_to_others!(e, ""))?;
            s.flush().map_err(::ttrpc::err_to_others!(e, ""))?;
        }

        let res = $self.client.request(creq).await?;
        let mut s = CodedInputStream::from_bytes(&res.payload);
        $cres
            .merge_from(&mut s)
            .map_err(::ttrpc::err_to_others!(e, "Unpack get error "))?;

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
    pub metadata: HashMap<String, Vec<String>>,
    pub timeout_nano: i64,
}

pub(crate) fn get_response_header_from_body(stream_id: u32, body: &[u8]) -> MessageHeader {
    MessageHeader {
        length: body.len() as u32,
        stream_id,
        type_: MESSAGE_TYPE_RESPONSE,
        flags: 0,
    }
}

pub(crate) fn get_request_header_from_body(stream_id: u32, body: &[u8]) -> MessageHeader {
    MessageHeader {
        length: body.len() as u32,
        stream_id,
        type_: MESSAGE_TYPE_REQUEST,
        flags: 0,
    }
}

pub(crate) fn new_unix_stream_from_raw_fd(fd: RawFd) -> UnixStream {
    let std_stream: std::os::unix::net::UnixStream;
    unsafe {
        std_stream = std::os::unix::net::UnixStream::from_raw_fd(fd);
    }
    // Notice: There is a big change between tokio 1.0 and 0.2
    // we must set nonblocking by ourselves in tokio 1.0
    std_stream.set_nonblocking(true).unwrap();
    UnixStream::from_std(std_stream).unwrap()
}

pub(crate) fn body_to_request(body: &[u8]) -> StdResult<Request, Status> {
    let mut req = Request::new();
    let merge_result;
    {
        let mut s = CodedInputStream::from_bytes(body);
        merge_result = req.merge_from(&mut s);
    }

    if merge_result.is_err() {
        return Err(get_status(Code::INVALID_ARGUMENT, "".to_string()));
    }

    trace!("Got Message request {:?}", req);

    Ok(req)
}

pub(crate) fn get_path(service: &str, method: &str) -> String {
    format!("/{}/{}", service, method)
}
