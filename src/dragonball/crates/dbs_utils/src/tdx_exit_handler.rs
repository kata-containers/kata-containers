// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

#![allow(missing_docs)]

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::{Arc, RwLock};

use kvm_ioctls::VmFd;
use log::*;
use tdx::launch::TdxCapabilities;
use threadpool::ThreadPool;
use vm_memory::{ByteValued, Bytes, GuestAddress, GuestMemoryMmap};

pub const TDX_GET_QUOTE_STRUCTURE_VERSION: u64 = 1;
pub const TDX_GET_QUOTE_BUF_ALIGN: u64 = 4096;

pub const TDX_GET_QUOTE_MAX_BUF_LEN: u64 = 128 * 1024;
pub const TDX_GET_QUOTE_MAX_REQUEST: usize = 16;

pub const TDX_GET_QUOTE_HDR_SIZE: u64 = core::mem::size_of::<TdxGetQuoteHeader>() as u64;

pub const TDG_VP_VMCALL_SUCCESS: u64 = 0;
pub const TDG_VP_VMCALL_RETRY: u64 = 1;
pub const TDG_VP_VMCALL_INVALID_OPERAND: u64 = 0x8000000000000000;
pub const TDG_VP_VMCALL_ALIGN_ERROR: u64 = 0x8000000000000002;

pub const TDX_VP_GET_QUOTE_SUCCESS: u64 = 0;
pub const TDX_VP_GET_QUOTE_IN_FLIGHT: u64 = u64::MAX;
pub const TDX_VP_GET_QUOTE_ERROR: u64 = 0x8000000000000000;
pub const TDX_VP_GET_QUOTE_QGS_UNAVAILABLE: u64 = 0x8000000000000001;

pub const TDG_VP_VMCALL_SUBFUNC_SET_EVENT_NOTIFY_INTERRUPT: u64 = 1 << 1;

pub const SUPPORTED_TDVMCALLINFO_1_R11: u64 = TDG_VP_VMCALL_SUBFUNC_SET_EVENT_NOTIFY_INTERRUPT;
pub const SUPPORTED_TDVMCALLINFO_1_R12: u64 = 0;

pub const QGS_MSG_LIB_MAJOR_VER: u16 = 1;
pub const QGS_MSG_LIB_MINOR_VER: u16 = 1;

pub const QGS_MSG_TYPE_GET_QUOTE_REQ: u32 = 0;
pub const QGS_MSG_TYPE_GET_QUOTE_RESP: u32 = 1;

const HEADER_SIZE: usize = 4;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TdxGetQuoteHeader {
    pub structure_version: u64,
    pub error_code: u64,
    pub in_len: u32,
    pub out_len: u32,
}

unsafe impl ByteValued for TdxGetQuoteHeader {}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct QgsMessageHeader {
    pub major_version: u16,
    pub minor_version: u16,
    pub r#type: u32,
    pub size: u32,
    pub error_code: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct QgsMessageGetQuoteReq {
    pub header: QgsMessageHeader,
    pub report_size: u32,
    pub id_list_size: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct QgsMessageGetQuoteResp {
    pub header: QgsMessageHeader,
    pub selected_id_size: u32,
    pub quote_size: u32,
}

unsafe impl ByteValued for QgsMessageHeader {}
unsafe impl ByteValued for QgsMessageGetQuoteReq {}
unsafe impl ByteValued for QgsMessageGetQuoteResp {}
pub struct TdxExitHandler {
    _vm_fd: Arc<VmFd>,
    quote_generation_socket: Option<String>,
    thread_pool: ThreadPool,
    event_notify_vector: Arc<RwLock<Option<u8>>>,
    _tdx_capabilities: Arc<TdxCapabilities>,
    mem: GuestMemoryMmap,
}

impl TdxExitHandler {
    pub fn new(
        vm_fd: Arc<VmFd>,
        quote_generation_socket: Option<String>,
        tdx_capabilities: Arc<TdxCapabilities>,
        mem: &GuestMemoryMmap,
    ) -> Self {
        Self {
            _vm_fd: vm_fd,
            quote_generation_socket,
            _tdx_capabilities: tdx_capabilities,
            mem: mem.clone(),
            thread_pool: ThreadPool::with_name(
                "tdxquote-thread".to_string(),
                TDX_GET_QUOTE_MAX_REQUEST,
            ),
            event_notify_vector: Arc::new(RwLock::new(None)),
        }
    }

    pub fn handle_get_tdvmcall_info(
        &self,
        ret: &mut u64,
        leaf: u64,
        _r11: &mut u64,
        _r12: &mut u64,
        _r13: &mut u64,
        _r14: &mut u64,
    ) {
        if leaf != 1 {
            return;
        }

        // *r11 = (self.tdx_capabilities.user_tdvmcallinfo_1_r11 & SUPPORTED_TDVMCALLINFO_1_R11)
        //     | self.tdx_capabilities.kernel_tdvmcallinfo_1_r11;
        // *r12 = (self.tdx_capabilities.user_tdvmcallinfo_1_r12 & SUPPORTED_TDVMCALLINFO_1_R12)
        //     | self.tdx_capabilities.kernel_tdvmcallinfo_1_r12;
        // *r13 = 0;
        // *r14 = 0;

        *ret = TDG_VP_VMCALL_SUCCESS;
    }

    pub fn handle_setup_event_notify_interrupt(&self, ret: &mut u64, vector: u64) {
        if (32..256).contains(&vector) {
            *self.event_notify_vector.write().unwrap() = Some(vector as u8);
            *ret = TDG_VP_VMCALL_SUCCESS;
        } else {
            *ret = TDG_VP_VMCALL_INVALID_OPERAND;
        }
    }

    pub fn handle_get_quote(&self, ret: &mut u64, buf_gpa: u64, buf_len: u64) {
        *ret = TDG_VP_VMCALL_INVALID_OPERAND;

        if buf_len == 0 {
            return;
        }

        if !buf_gpa.is_multiple_of(TDX_GET_QUOTE_BUF_ALIGN)
            || !buf_len.is_multiple_of(TDX_GET_QUOTE_BUF_ALIGN)
        {
            *ret = TDG_VP_VMCALL_ALIGN_ERROR;
            return;
        }

        let mut header: TdxGetQuoteHeader = match self.mem.read_obj(GuestAddress(buf_gpa)) {
            Ok(hdr) => hdr,
            Err(_) => {
                error!("TDX GetQuote: Failed to read GetQuote header");
                return;
            }
        };

        if header.structure_version != TDX_GET_QUOTE_STRUCTURE_VERSION {
            return;
        }

        if buf_len > TDX_GET_QUOTE_MAX_BUF_LEN
            || header.in_len as u64 > buf_len - TDX_GET_QUOTE_HDR_SIZE
        {
            return;
        }

        if self.quote_generation_socket.is_none() {
            header.error_code = TDX_VP_GET_QUOTE_QGS_UNAVAILABLE;
            if self.mem.write_obj(header, GuestAddress(buf_gpa)).is_err() {
                error!("TDX GetQuote: Failed to update GetQuote header");
                return;
            }
            *ret = TDG_VP_VMCALL_SUCCESS;
            return;
        }

        if self.thread_pool.active_count() >= TDX_GET_QUOTE_MAX_REQUEST {
            *ret = TDG_VP_VMCALL_RETRY;
            return;
        }

        let mut report_data = vec![0u8; header.in_len as usize];
        if self
            .mem
            .read_slice(
                report_data.as_mut_slice(),
                GuestAddress(buf_gpa + TDX_GET_QUOTE_HDR_SIZE),
            )
            .is_err()
        {
            error!("TDX GetQuote: Failed to read report data");
            return;
        }

        let quote_generation_socket = self.quote_generation_socket.clone().unwrap();
        let mem = self.mem.clone();

        self.thread_pool.execute(move || {
            Self::threaded_get_quote(
                header,
                buf_gpa,
                buf_len,
                report_data,
                quote_generation_socket,
                mem,
            )
        });

        header.error_code = TDX_VP_GET_QUOTE_IN_FLIGHT;
        if self.mem.write_obj(header, GuestAddress(buf_gpa)).is_err() {
            return;
        }

        *ret = TDG_VP_VMCALL_SUCCESS;
    }

    fn threaded_get_quote(
        mut header: TdxGetQuoteHeader,
        buf_gpa: u64,
        buf_len: u64,
        report_data: Vec<u8>,
        quote_generation_socket: String,
        mem: GuestMemoryMmap,
    ) {
        match Self::generate_quote(buf_len, report_data, quote_generation_socket) {
            Ok(quote) => {
                if mem
                    .write_slice(
                        quote.as_slice(),
                        GuestAddress(buf_gpa + TDX_GET_QUOTE_HDR_SIZE),
                    )
                    .is_err()
                {
                    error!("TDX GetQuote: Failed to write quote data");
                    header.error_code = TDX_VP_GET_QUOTE_ERROR;
                } else {
                    header.out_len = quote.len() as u32;
                    header.error_code = TDX_VP_GET_QUOTE_SUCCESS;
                }
            }
            Err(status_code) => {
                header.error_code = status_code;
            }
        }

        let _ = mem.write_obj(header, GuestAddress(buf_gpa));
    }

    fn generate_quote(
        buf_len: u64,
        report_data: Vec<u8>,
        quote_generation_socket: String,
    ) -> Result<Vec<u8>, u64> {
        let req_size = (core::mem::size_of::<QgsMessageGetQuoteReq>() + report_data.len()) as u32;
        let req_message = QgsMessageGetQuoteReq {
            header: QgsMessageHeader {
                major_version: QGS_MSG_LIB_MAJOR_VER,
                minor_version: QGS_MSG_LIB_MINOR_VER,
                r#type: QGS_MSG_TYPE_GET_QUOTE_REQ,
                size: req_size,
                error_code: 0,
            },
            report_size: report_data.len() as u32,
            id_list_size: 0,
        };

        // Length prefix
        let req_header = encode_header(req_size);

        let mut stream = UnixStream::connect(quote_generation_socket)
            .map_err(|_| TDX_VP_GET_QUOTE_QGS_UNAVAILABLE)?;

        stream
            .write_all(&req_header)
            .map_err(|_| TDX_VP_GET_QUOTE_ERROR)?;
        stream
            .write_all(req_message.as_slice())
            .map_err(|_| TDX_VP_GET_QUOTE_ERROR)?;
        stream
            .write_all(report_data.as_slice())
            .map_err(|_| TDX_VP_GET_QUOTE_ERROR)?;
        stream.flush().map_err(|_| TDX_VP_GET_QUOTE_ERROR)?;

        let resp_message_size = core::mem::size_of::<QgsMessageGetQuoteResp>();

        let mut resp_header = [0u8; HEADER_SIZE];
        stream
            .read_exact(&mut resp_header)
            .map_err(|_| TDX_VP_GET_QUOTE_ERROR)?;
        let resp_size = decode_header(&resp_header);

        if resp_size < resp_message_size as u32 {
            error!("TDX GetQuote: Bad response message size");
            return Err(TDX_VP_GET_QUOTE_ERROR);
        }

        let mut resp_message_buf = vec![0u8; resp_message_size];
        stream
            .read_exact(&mut resp_message_buf)
            .map_err(|_| TDX_VP_GET_QUOTE_ERROR)?;
        let resp_message = match QgsMessageGetQuoteResp::from_slice(resp_message_buf.as_slice()) {
            Some(msg) => msg,
            None => return Err(TDX_VP_GET_QUOTE_ERROR),
        };

        if resp_message.header.major_version != QGS_MSG_LIB_MAJOR_VER
            || resp_message.header.minor_version != QGS_MSG_LIB_MINOR_VER
        {
            return Err(TDX_VP_GET_QUOTE_ERROR);
        }

        if resp_message.header.r#type != QGS_MSG_TYPE_GET_QUOTE_RESP {
            return Err(TDX_VP_GET_QUOTE_ERROR);
        }

        if resp_message.header.size > resp_size {
            return Err(TDX_VP_GET_QUOTE_ERROR);
        }

        if resp_message.header.error_code != 0 {
            return Err(TDX_VP_GET_QUOTE_ERROR);
        }

        if resp_message.selected_id_size != 0 {
            return Err(TDX_VP_GET_QUOTE_ERROR);
        }

        let quote_size = resp_message.quote_size;
        if quote_size != resp_size - resp_message_size as u32
            || quote_size > (buf_len - TDX_GET_QUOTE_HDR_SIZE) as u32
        {
            return Err(TDX_VP_GET_QUOTE_ERROR);
        }

        let mut quote_buf = vec![0u8; quote_size as usize];
        stream
            .read_exact(&mut quote_buf)
            .map_err(|_| TDX_VP_GET_QUOTE_ERROR)?;

        Ok(quote_buf)
    }
}

fn encode_header(size: u32) -> [u8; HEADER_SIZE] {
    size.to_be_bytes()
}

fn decode_header(buf: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*buf)
}
