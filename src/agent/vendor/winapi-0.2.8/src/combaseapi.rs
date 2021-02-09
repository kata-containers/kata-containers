// Copyright © 2016, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
use super::*;
pub const CLSCTX_INPROC_SERVER: DWORD = 0x1;
pub const CLSCTX_INPROC_HANDLER: DWORD = 0x2;
pub const CLSCTX_LOCAL_SERVER: DWORD = 0x4;
pub const CLSCTX_REMOTE_SERVER: DWORD = 0x10;
pub const CLSCTX_SERVER: DWORD = CLSCTX_INPROC_SERVER | CLSCTX_LOCAL_SERVER |
                                 CLSCTX_REMOTE_SERVER;
pub const CLSCTX_ALL: DWORD = CLSCTX_INPROC_HANDLER | CLSCTX_SERVER;
STRUCT!{struct ServerInformation {
    dwServerPid: DWORD,
    dwServerTid: DWORD,
    ui64ServerAddress: UINT64,
}}
pub type PServerInformation = *mut ServerInformation;
DECLARE_HANDLE!(CO_MTA_USAGE_COOKIE, CO_MTA_USAGE_COOKIE__);
