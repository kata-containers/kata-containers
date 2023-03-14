# Maintenance manual

This library is translated using c2rust.  This document describes the
translation, and how to redo it if the upstream project changes.

First, merge any changes from upstream into the repository, taking
care of any merge conflicts.

Run `make check` to see if the C implementation still works.

Then, remove `lib/sha1.rs` (`lib/ubc_check.rs` if the
`src/ubc_check.c` was changed).  Run `c2rust transpile --emit-no-std
compile_commands.json`.

Edit `lib/sha1.rs`.  Remove the following lines at the top:

```rust
#![register_tool(c2rust)]
#![feature(register_tool)]
#![no_std]
```

Add the replacement memcpy function
```rust
unsafe fn memcpy<T>(dst: *mut T, src: *const T, count: usize) {
    core::intrinsics::copy_nonoverlapping(src, dst, count)
}
```
And replace `sha1_process_unaligned` and `maybe_bswap32` with the
following functions:

```rust
#[inline]
unsafe extern "C" fn sha1_process_unaligned(mut ctx: *mut SHA1_CTX,
                                            buf: *const libc::c_void) {
    if cfg!(any(target_arch = "x86", target_arch = "x86_64")) {
        sha1_process(ctx, buf as *mut uint32_t as *const uint32_t);
    } else {
        debug_assert_eq!(core::mem::align_of::<u8>(), 1);
        memcpy((*ctx).buffer.as_mut_ptr() as *mut _, buf, 64);
        sha1_process(ctx, (*ctx).buffer.as_mut_ptr() as *const uint32_t);
    }
}
```

```rust
#[inline]
unsafe extern "C" fn maybe_bswap32(mut x: uint32_t) -> uint32_t {
    if cfg!(target_endian = "big") {
        x
    } else if cfg!(target_endian = "little") {
        sha1_bswap32(x)
    } else {
        unimplemented!()
    }
}
```

Apply the following fixes:

  - 11b262c9e60a29bc982ede9c897a5237bbce7a6b
  - 0ec7d52617eb35800e761fd055517522322afa8e
  - 2a62ed1644b688a70ba2ee821fb65de39cfc43df
  - 03c46946816375df13d2c3bd5d649b56177768f9
  - caa5319f05481f12137f4ac128ea1c7e6eb3dd28
  - 5c20cda259ea066da451946ba52b82144156847e
  - 912db580ae5c8de45d0c39f06f7984a7d5124f7e

In lib/sha1.rs and lib/ubc_check.rs, run the following replacements:
  - s/libc::c_int/i32/g
  - s/libc::c_uint/u32/g
  - s/libc::c_char/i8/g
  - s/libc::c_uchar/u8/g
  - s/libc::size_t/usize/g
  - s/libc::ulong/u64/g
  - s/libc::c_void/core::ffi::c_void
These are the correct types on the platform that was initially used to transpile,
x86_64-unknown-linux-gnu, and also on wasm32-wasi, so they are likely correct.

Run `cargo check` to see if the Rust implementation still works.

Finally, commit the changes.
