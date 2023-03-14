#![no_std]
#![cfg_attr(all(not(feature = "std"), feature = "unstable"),
            feature(core_intrinsics, core_panic, raw, unicode_internals))]
#![cfg_attr(all(not(feature = "std"), feature = "alloc", feature = "unstable"),
            feature(alloc_prelude, raw_vec_internals, wake_trait))]

// Can't use cfg_if! because it does not allow nesting :(

// Actually, can't even generate #[cfg]s any other way because of
// https://github.com/rust-lang/rust/pull/52234#issuecomment-486810130

// if #[cfg(feature = "std")] {
    #[cfg(feature = "std")]
    extern crate std;
    #[cfg(feature = "std")]
    pub mod prelude {
        pub mod v1 {
            pub use std::prelude::v1::*;
            // Macros aren't included in the prelude for some reason
            pub use std::{
                format, vec,
                print, println, eprint, eprintln, dbg
            };
        }
    }
    #[cfg(feature = "std")]
    pub use std::*;
// } else {
    // The 2 underscores in the crate names are used to avoid
    // ambiguity between whether the user wants to use the public
    // module std::alloc or the private crate no_std_compat::alloc
    // (see https://gitlab.com/jD91mZM2/no-std-compat/issues/1)

    // if #[cfg(feature = "alloc")] {
        #[cfg(all(not(feature = "std"), feature = "alloc"))]
        extern crate alloc as __alloc;
    // }

    #[cfg(not(feature = "std"))]
    extern crate core as __core;

    #[cfg(not(feature = "std"))]
    mod generated;

    #[cfg(not(feature = "std"))]
    pub use self::generated::*;

    // if #[cfg(feature = "compat_macros")] {
        #[cfg(all(not(feature = "std"), feature = "compat_macros"))]
        #[macro_export]
        macro_rules! print {
            () => {{}};
            ($($arg:tt)+) => {{
                // Avoid unused arguments complaint. This surely must get
                // optimized away? TODO: Verify that
                let _ = format_args!($($arg)+);
            }};
        }
        #[cfg(all(not(feature = "std"), feature = "compat_macros"))]
        #[macro_export]
        macro_rules! println {
            ($($arg:tt)*) => { print!($($arg)*) }
        }
        #[cfg(all(not(feature = "std"), feature = "compat_macros"))]
        #[macro_export]
        macro_rules! eprint {
            ($($arg:tt)*) => { print!($($arg)*) }
        }
        #[cfg(all(not(feature = "std"), feature = "compat_macros"))]
        #[macro_export]
        macro_rules! eprintln {
            ($($arg:tt)*) => { print!($($arg)*) }
        }

        #[cfg(all(not(feature = "std"), feature = "compat_macros"))]
        #[macro_export]
        macro_rules! dbg {
            () => {};
            ($($val:expr),+) => { ($($val),+) }
        }
    // }
// }
