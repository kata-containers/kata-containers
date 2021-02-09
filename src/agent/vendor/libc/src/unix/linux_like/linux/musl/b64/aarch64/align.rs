s_no_extra_traits! {
    #[allow(missing_debug_implementations)]
    #[repr(align(16))]
    pub struct max_align_t {
        priv_: [f32; 8]
    }
}

s!{
    pub struct ucontext_t {
        pub uc_flags: ::c_ulong,
        pub uc_link: *mut ucontext_t,
        pub uc_stack: ::stack_t,
        pub uc_sigmask: ::sigset_t,
        pub uc_mcontext: mcontext_t,
    }

    #[repr(align(16))]
    pub struct mcontext_t {
        // What we want here is a single [u64; 36 + 512], but splitting things
        // up allows Debug to be auto-derived.
        __regs1: [[u64; 18]; 2], // 36
        __regs2: [[u64; 32]; 16], // 512
    }
}
