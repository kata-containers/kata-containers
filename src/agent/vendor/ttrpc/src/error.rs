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

use crate::ttrpc::{Code, Status};
use std::result;

#[derive(Debug)]
pub enum Error {
    Socket(String),
    RpcStatus(Status),
    Others(String),
}

pub type Result<T> = result::Result<T, Error>;

pub fn get_status(c: Code, msg: String) -> Status {
    let mut status = Status::new();
    status.set_code(c);
    status.set_message(msg);

    status
}

pub fn get_rpc_status(c: Code, msg: String) -> Error {
    Error::RpcStatus(get_status(c, msg))
}

macro_rules! err_to_RpcStatus {
    ($c: expr, $e: ident, $s: expr) => {
        |$e| get_rpc_status($c, $s.to_string() + &$e.to_string())
    };
}

macro_rules! err_to_Others {
    ($e: ident, $s: expr) => {
        |$e| Error::Others($s.to_string() + &$e.to_string())
    };
}

#[macro_export]
macro_rules! Err_to_Others {
    ($e: ident, $s: expr) => {
        |$e| ::ttrpc::Error::Others($s.to_string() + &$e.to_string())
    };
}
