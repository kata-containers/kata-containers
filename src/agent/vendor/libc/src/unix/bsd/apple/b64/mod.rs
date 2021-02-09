//! 64-bit specific Apple (ios/darwin) definitions

pub type c_long = i64;
pub type c_ulong = u64;
pub type boolean_t = ::c_uint;
pub type mcontext_t = *mut __darwin_mcontext64;

s! {
    pub struct timeval32 {
        pub tv_sec: i32,
        pub tv_usec: i32,
    }

    pub struct if_data {
        pub ifi_type: ::c_uchar,
        pub ifi_typelen: ::c_uchar,
        pub ifi_physical: ::c_uchar,
        pub ifi_addrlen: ::c_uchar,
        pub ifi_hdrlen: ::c_uchar,
        pub ifi_recvquota: ::c_uchar,
        pub ifi_xmitquota: ::c_uchar,
        pub ifi_unused1: ::c_uchar,
        pub ifi_mtu: u32,
        pub ifi_metric: u32,
        pub ifi_baudrate: u32,
        pub ifi_ipackets: u32,
        pub ifi_ierrors: u32,
        pub ifi_opackets: u32,
        pub ifi_oerrors: u32,
        pub ifi_collisions: u32,
        pub ifi_ibytes: u32,
        pub ifi_obytes: u32,
        pub ifi_imcasts: u32,
        pub ifi_omcasts: u32,
        pub ifi_iqdrops: u32,
        pub ifi_noproto: u32,
        pub ifi_recvtiming: u32,
        pub ifi_xmittiming: u32,
        pub ifi_lastchange: timeval32,
        pub ifi_unused2: u32,
        pub ifi_hwassist: u32,
        pub ifi_reserved1: u32,
        pub ifi_reserved2: u32,
    }

    pub struct bpf_hdr {
        pub bh_tstamp: ::timeval32,
        pub bh_caplen: u32,
        pub bh_datalen: u32,
        pub bh_hdrlen: ::c_ushort,
    }

    pub struct ucontext_t {
        pub uc_onstack: ::c_int,
        pub uc_sigmask: ::sigset_t,
        pub uc_stack: ::stack_t,
        pub uc_link: *mut ::ucontext_t,
        pub uc_mcsize: usize,
        pub uc_mcontext: mcontext_t,
    }

    pub struct __darwin_mcontext64 {
        pub __es: __darwin_x86_exception_state64,
        pub __ss: __darwin_x86_thread_state64,
        pub __fs: __darwin_x86_float_state64,
    }

    pub struct __darwin_x86_exception_state64 {
        pub __trapno: u16,
        pub __cpu: u16,
        pub __err: u32,
        pub __faultvaddr: u64,
    }

    pub struct __darwin_x86_thread_state64 {
        pub __rax: u64,
        pub __rbx: u64,
        pub __rcx: u64,
        pub __rdx: u64,
        pub __rdi: u64,
        pub __rsi: u64,
        pub __rbp: u64,
        pub __rsp: u64,
        pub __r8: u64,
        pub __r9: u64,
        pub __r10: u64,
        pub __r11: u64,
        pub __r12: u64,
        pub __r13: u64,
        pub __r14: u64,
        pub __r15: u64,
        pub __rip: u64,
        pub __rflags: u64,
        pub __cs: u64,
        pub __fs: u64,
        pub __gs: u64,
    }

    pub struct __darwin_x86_float_state64 {
        pub __fpu_reserved: [::c_int; 2],
        __fpu_fcw: ::c_short,
        __fpu_fsw: ::c_short,
        pub __fpu_ftw: u8,
        pub __fpu_rsrv1: u8,
        pub __fpu_fop: u16,
        pub __fpu_ip: u32,
        pub __fpu_cs: u16,
        pub __fpu_rsrv2: u16,
        pub __fpu_dp: u32,
        pub __fpu_ds: u16,
        pub __fpu_rsrv3: u16,
        pub __fpu_mxcsr: u32,
        pub __fpu_mxcsrmask: u32,
        pub __fpu_stmm0: __darwin_mmst_reg,
        pub __fpu_stmm1: __darwin_mmst_reg,
        pub __fpu_stmm2: __darwin_mmst_reg,
        pub __fpu_stmm3: __darwin_mmst_reg,
        pub __fpu_stmm4: __darwin_mmst_reg,
        pub __fpu_stmm5: __darwin_mmst_reg,
        pub __fpu_stmm6: __darwin_mmst_reg,
        pub __fpu_stmm7: __darwin_mmst_reg,
        pub __fpu_xmm0: __darwin_xmm_reg,
        pub __fpu_xmm1: __darwin_xmm_reg,
        pub __fpu_xmm2: __darwin_xmm_reg,
        pub __fpu_xmm3: __darwin_xmm_reg,
        pub __fpu_xmm4: __darwin_xmm_reg,
        pub __fpu_xmm5: __darwin_xmm_reg,
        pub __fpu_xmm6: __darwin_xmm_reg,
        pub __fpu_xmm7: __darwin_xmm_reg,
        pub __fpu_xmm8: __darwin_xmm_reg,
        pub __fpu_xmm9: __darwin_xmm_reg,
        pub __fpu_xmm10: __darwin_xmm_reg,
        pub __fpu_xmm11: __darwin_xmm_reg,
        pub __fpu_xmm12: __darwin_xmm_reg,
        pub __fpu_xmm13: __darwin_xmm_reg,
        pub __fpu_xmm14: __darwin_xmm_reg,
        pub __fpu_xmm15: __darwin_xmm_reg,
        // this field is actually [u8; 96], but defining it with a bigger type
        // allows us to auto-implement traits for it since the length of the
        // array is less than 32
        __fpu_rsrv4: [u32; 24],
        pub __fpu_reserved1: ::c_int,
    }

    pub struct __darwin_mmst_reg {
        pub __mmst_reg: [::c_char; 10],
        pub __mmst_rsrv: [::c_char; 6],
    }

    pub struct __darwin_xmm_reg {
        pub __xmm_reg: [::c_char; 16],
    }
}

s_no_extra_traits! {
    pub struct pthread_attr_t {
        __sig: c_long,
        __opaque: [::c_char; 56]
    }
}

cfg_if! {
    if #[cfg(feature = "extra_traits")] {
        impl PartialEq for pthread_attr_t {
            fn eq(&self, other: &pthread_attr_t) -> bool {
                self.__sig == other.__sig
                    && self.__opaque
                    .iter()
                    .zip(other.__opaque.iter())
                    .all(|(a,b)| a == b)
            }
        }
        impl Eq for pthread_attr_t {}
        impl ::fmt::Debug for pthread_attr_t {
            fn fmt(&self, f: &mut ::fmt::Formatter) -> ::fmt::Result {
                f.debug_struct("pthread_attr_t")
                    .field("__sig", &self.__sig)
                // FIXME: .field("__opaque", &self.__opaque)
                    .finish()
            }
        }
        impl ::hash::Hash for pthread_attr_t {
            fn hash<H: ::hash::Hasher>(&self, state: &mut H) {
                self.__sig.hash(state);
                self.__opaque.hash(state);
            }
        }
    }
}

#[doc(hidden)]
#[deprecated(since = "0.2.55")]
pub const NET_RT_MAXID: ::c_int = 11;

pub const __PTHREAD_MUTEX_SIZE__: usize = 56;
pub const __PTHREAD_COND_SIZE__: usize = 40;
pub const __PTHREAD_CONDATTR_SIZE__: usize = 8;
pub const __PTHREAD_RWLOCK_SIZE__: usize = 192;
pub const __PTHREAD_RWLOCKATTR_SIZE__: usize = 16;

pub const TIOCTIMESTAMP: ::c_ulong = 0x40107459;
pub const TIOCDCDTIMESTAMP: ::c_ulong = 0x40107458;

pub const BIOCSETF: ::c_ulong = 0x80104267;
pub const BIOCSRTIMEOUT: ::c_ulong = 0x8010426d;
pub const BIOCGRTIMEOUT: ::c_ulong = 0x4010426e;
pub const BIOCSETFNR: ::c_ulong = 0x8010427e;

extern "C" {
    pub fn exchangedata(
        path1: *const ::c_char,
        path2: *const ::c_char,
        options: ::c_uint,
    ) -> ::c_int;
}

cfg_if! {
    if #[cfg(libc_align)] {
        mod align;
        pub use self::align::*;
    }
}
