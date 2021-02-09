// Copyright 2014-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Backtrace support using libunwind/gcc_s/etc APIs.
//!
//! This module contains the ability to unwind the stack using libunwind-style
//! APIs. Note that there's a whole bunch of implementations of the
//! libunwind-like API, and this is just trying to be compatible with most of
//! them all at once instead of being picky.
//!
//! The libunwind API is powered by `_Unwind_Backtrace` and is in practice very
//! reliable at generating a backtrace. It's not entirely clear how it does it
//! (frame pointers? eh_frame info? both?) but it seems to work!
//!
//! Most of the complexity of this module is handling the various platform
//! differences across libunwind implementations. Otherwise this is a pretty
//! straightforward Rust binding to the libunwind APIs.
//!
//! This is the default unwinding API for all non-Windows platforms currently.

use super::super::Bomb;
use core::ffi::c_void;

pub enum Frame {
    Raw(*mut uw::_Unwind_Context),
    Cloned {
        ip: *mut c_void,
        sp: *mut c_void,
        symbol_address: *mut c_void,
    },
}

// With a raw libunwind pointer it should only ever be access in a readonly
// threadsafe fashion, so it's `Sync`. When sending to other threads via `Clone`
// we always switch to a version which doesn't retain interior pointers, so we
// should be `Send` as well.
unsafe impl Send for Frame {}
unsafe impl Sync for Frame {}

impl Frame {
    pub fn ip(&self) -> *mut c_void {
        let ctx = match *self {
            Frame::Raw(ctx) => ctx,
            Frame::Cloned { ip, .. } => return ip,
        };
        unsafe { uw::_Unwind_GetIP(ctx) as *mut c_void }
    }

    pub fn sp(&self) -> *mut c_void {
        match *self {
            Frame::Raw(ctx) => unsafe { uw::get_sp(ctx) as *mut c_void },
            Frame::Cloned { sp, .. } => sp,
        }
    }

    pub fn symbol_address(&self) -> *mut c_void {
        if let Frame::Cloned { symbol_address, .. } = *self {
            return symbol_address;
        }

        // It seems that on OSX `_Unwind_FindEnclosingFunction` returns a
        // pointer to... something that's unclear. It's definitely not always
        // the enclosing function for whatever reason. It's not entirely clear
        // to me what's going on here, so pessimize this for now and just always
        // return the ip.
        //
        // Note the `skip_inner_frames.rs` test is skipped on OSX due to this
        // clause, and if this is fixed that test in theory can be run on OSX!
        if cfg!(target_os = "macos") || cfg!(target_os = "ios") {
            self.ip()
        } else {
            unsafe { uw::_Unwind_FindEnclosingFunction(self.ip()) }
        }
    }

    pub fn module_base_address(&self) -> Option<*mut c_void> {
        None
    }
}

impl Clone for Frame {
    fn clone(&self) -> Frame {
        Frame::Cloned {
            ip: self.ip(),
            sp: self.sp(),
            symbol_address: self.symbol_address(),
        }
    }
}

#[inline(always)]
pub unsafe fn trace(mut cb: &mut dyn FnMut(&super::Frame) -> bool) {
    uw::_Unwind_Backtrace(trace_fn, &mut cb as *mut _ as *mut _);

    extern "C" fn trace_fn(
        ctx: *mut uw::_Unwind_Context,
        arg: *mut c_void,
    ) -> uw::_Unwind_Reason_Code {
        let cb = unsafe { &mut *(arg as *mut &mut dyn FnMut(&super::Frame) -> bool) };
        let cx = super::Frame {
            inner: Frame::Raw(ctx),
        };

        let mut bomb = Bomb { enabled: true };
        let keep_going = cb(&cx);
        bomb.enabled = false;

        if keep_going {
            uw::_URC_NO_REASON
        } else {
            uw::_URC_FAILURE
        }
    }
}

/// Unwind library interface used for backtraces
///
/// Note that dead code is allowed as here are just bindings
/// iOS doesn't use all of them it but adding more
/// platform-specific configs pollutes the code too much
#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[allow(dead_code)]
mod uw {
    pub use self::_Unwind_Reason_Code::*;

    use core::ffi::c_void;

    #[repr(C)]
    pub enum _Unwind_Reason_Code {
        _URC_NO_REASON = 0,
        _URC_FOREIGN_EXCEPTION_CAUGHT = 1,
        _URC_FATAL_PHASE2_ERROR = 2,
        _URC_FATAL_PHASE1_ERROR = 3,
        _URC_NORMAL_STOP = 4,
        _URC_END_OF_STACK = 5,
        _URC_HANDLER_FOUND = 6,
        _URC_INSTALL_CONTEXT = 7,
        _URC_CONTINUE_UNWIND = 8,
        _URC_FAILURE = 9, // used only by ARM EABI
    }

    pub enum _Unwind_Context {}

    pub type _Unwind_Trace_Fn =
        extern "C" fn(ctx: *mut _Unwind_Context, arg: *mut c_void) -> _Unwind_Reason_Code;

    extern "C" {
        // No native _Unwind_Backtrace on iOS
        #[cfg(not(all(target_os = "ios", target_arch = "arm")))]
        pub fn _Unwind_Backtrace(
            trace: _Unwind_Trace_Fn,
            trace_argument: *mut c_void,
        ) -> _Unwind_Reason_Code;

        // available since GCC 4.2.0, should be fine for our purpose
        #[cfg(all(
            not(all(target_os = "android", target_arch = "arm")),
            not(all(target_os = "freebsd", target_arch = "arm")),
            not(all(target_os = "linux", target_arch = "arm"))
        ))]
        pub fn _Unwind_GetIP(ctx: *mut _Unwind_Context) -> libc::uintptr_t;

        #[cfg(all(
            not(all(target_os = "android", target_arch = "arm")),
            not(all(target_os = "freebsd", target_arch = "arm")),
            not(all(target_os = "linux", target_arch = "arm"))
        ))]
        pub fn _Unwind_FindEnclosingFunction(pc: *mut c_void) -> *mut c_void;

        #[cfg(all(
            not(all(target_os = "android", target_arch = "arm")),
            not(all(target_os = "freebsd", target_arch = "arm")),
            not(all(target_os = "linux", target_arch = "arm"))
        ))]
        // This function is a misnomer: rather than getting this frame's
        // Canonical Frame Address (aka the caller frame's SP) it
        // returns this frame's SP.
        //
        // https://github.com/libunwind/libunwind/blob/d32956507cf29d9b1a98a8bce53c78623908f4fe/src/unwind/GetCFA.c#L28-L35
        #[link_name = "_Unwind_GetCFA"]
        pub fn get_sp(ctx: *mut _Unwind_Context) -> libc::uintptr_t;
    }

    // On android and arm, the function `_Unwind_GetIP` and a bunch of others
    // are macros, so we define functions containing the expansion of the
    // macros.
    //
    // TODO: link to the header file that defines these macros, if you can find
    // it. (I, fitzgen, cannot find the header file that some of these macro
    // expansions were originally borrowed from.)
    #[cfg(any(
        all(target_os = "android", target_arch = "arm"),
        all(target_os = "freebsd", target_arch = "arm"),
        all(target_os = "linux", target_arch = "arm")
    ))]
    pub use self::arm::*;
    #[cfg(any(
        all(target_os = "android", target_arch = "arm"),
        all(target_os = "freebsd", target_arch = "arm"),
        all(target_os = "linux", target_arch = "arm")
    ))]
    mod arm {
        pub use super::*;
        #[repr(C)]
        enum _Unwind_VRS_Result {
            _UVRSR_OK = 0,
            _UVRSR_NOT_IMPLEMENTED = 1,
            _UVRSR_FAILED = 2,
        }
        #[repr(C)]
        enum _Unwind_VRS_RegClass {
            _UVRSC_CORE = 0,
            _UVRSC_VFP = 1,
            _UVRSC_FPA = 2,
            _UVRSC_WMMXD = 3,
            _UVRSC_WMMXC = 4,
        }
        #[repr(C)]
        enum _Unwind_VRS_DataRepresentation {
            _UVRSD_UINT32 = 0,
            _UVRSD_VFPX = 1,
            _UVRSD_FPAX = 2,
            _UVRSD_UINT64 = 3,
            _UVRSD_FLOAT = 4,
            _UVRSD_DOUBLE = 5,
        }

        type _Unwind_Word = libc::c_uint;
        extern "C" {
            fn _Unwind_VRS_Get(
                ctx: *mut _Unwind_Context,
                klass: _Unwind_VRS_RegClass,
                word: _Unwind_Word,
                repr: _Unwind_VRS_DataRepresentation,
                data: *mut c_void,
            ) -> _Unwind_VRS_Result;
        }

        pub unsafe fn _Unwind_GetIP(ctx: *mut _Unwind_Context) -> libc::uintptr_t {
            let mut val: _Unwind_Word = 0;
            let ptr = &mut val as *mut _Unwind_Word;
            let _ = _Unwind_VRS_Get(
                ctx,
                _Unwind_VRS_RegClass::_UVRSC_CORE,
                15,
                _Unwind_VRS_DataRepresentation::_UVRSD_UINT32,
                ptr as *mut c_void,
            );
            (val & !1) as libc::uintptr_t
        }

        // R13 is the stack pointer on arm.
        const SP: _Unwind_Word = 13;

        pub unsafe fn get_sp(ctx: *mut _Unwind_Context) -> libc::uintptr_t {
            let mut val: _Unwind_Word = 0;
            let ptr = &mut val as *mut _Unwind_Word;
            let _ = _Unwind_VRS_Get(
                ctx,
                _Unwind_VRS_RegClass::_UVRSC_CORE,
                SP,
                _Unwind_VRS_DataRepresentation::_UVRSD_UINT32,
                ptr as *mut c_void,
            );
            val as libc::uintptr_t
        }

        // This function also doesn't exist on Android or ARM/Linux, so make it
        // a no-op.
        pub unsafe fn _Unwind_FindEnclosingFunction(pc: *mut c_void) -> *mut c_void {
            pc
        }
    }
}
