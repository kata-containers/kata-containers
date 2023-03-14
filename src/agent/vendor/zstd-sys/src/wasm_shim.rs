use std::alloc::{alloc, dealloc, Layout};
use std::os::raw::{c_int, c_void};

#[no_mangle]
pub extern "C" fn rust_zstd_wasm_shim_malloc(size: usize) -> *mut c_void {
    unsafe {
        let layout = Layout::from_size_align_unchecked(size, 1);
        alloc(layout).cast()
    }
}

#[no_mangle]
pub extern "C" fn rust_zstd_wasm_shim_calloc(
    nmemb: usize,
    size: usize,
) -> *mut c_void {
    unsafe {
        let layout = Layout::from_size_align_unchecked(size * nmemb, 1);
        alloc(layout).cast()
    }
}

#[no_mangle]
pub unsafe extern "C" fn rust_zstd_wasm_shim_free(ptr: *mut c_void) {
    // layout is not actually used
    let layout = Layout::from_size_align_unchecked(1, 1);
    dealloc(ptr.cast(), layout);
}

#[no_mangle]
pub unsafe extern "C" fn rust_zstd_wasm_shim_memcpy(
    dest: *mut c_void,
    src: *const c_void,
    n: usize,
) -> *mut c_void {
    std::ptr::copy_nonoverlapping(src as *const u8, dest as *mut u8, n);
    dest
}

#[no_mangle]
pub unsafe extern "C" fn rust_zstd_wasm_shim_memmove(
    dest: *mut c_void,
    src: *const c_void,
    n: usize,
) -> *mut c_void {
    std::ptr::copy(src as *const u8, dest as *mut u8, n);
    dest
}

#[no_mangle]
pub unsafe extern "C" fn rust_zstd_wasm_shim_memset(
    dest: *mut c_void,
    c: c_int,
    n: usize,
) -> *mut c_void {
    std::ptr::write_bytes(dest as *mut u8, c as u8, n);
    dest
}
