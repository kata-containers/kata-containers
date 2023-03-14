use nix::Error;
use nix::libc::{c_void, size_t};
use nix::sys::mman::{mmap, MapFlags, ProtFlags};

#[cfg(target_os = "linux")]
use nix::sys::mman::{mremap, MRemapFlags};

#[test]
fn test_mmap_anonymous() {
    let ref mut byte = unsafe {
        let ptr = mmap(std::ptr::null_mut(), 1,
                       ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
                       MapFlags::MAP_PRIVATE | MapFlags::MAP_ANONYMOUS, -1, 0)
                      .unwrap();
        *(ptr as * mut u8)
    };
    assert_eq !(*byte, 0x00u8);
    *byte = 0xffu8;
    assert_eq !(*byte, 0xffu8);
}

#[test]
#[cfg(target_os = "linux")]
fn test_mremap_grow() {
    const ONE_K : size_t = 1024;
    let slice : &mut[u8] = unsafe {
        let mem = mmap(std::ptr::null_mut(), ONE_K,
                       ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
                       MapFlags::MAP_ANONYMOUS | MapFlags::MAP_PRIVATE, -1, 0)
                      .unwrap();
        std::slice::from_raw_parts_mut(mem as * mut u8, ONE_K)
    };
    assert_eq !(slice[ONE_K - 1], 0x00);
    slice[ONE_K - 1] = 0xFF;
    assert_eq !(slice[ONE_K - 1], 0xFF);

    let slice : &mut[u8] = unsafe {
        let mem = mremap(slice.as_mut_ptr() as * mut c_void, ONE_K, 10 * ONE_K,
                         MRemapFlags::MREMAP_MAYMOVE, None)
                      .unwrap();
        std::slice::from_raw_parts_mut(mem as * mut u8, 10 * ONE_K)
    };

    // The first KB should still have the old data in it.
    assert_eq !(slice[ONE_K - 1], 0xFF);

    // The additional range should be zero-init'd and accessible.
    assert_eq !(slice[10 * ONE_K - 1], 0x00);
    slice[10 * ONE_K - 1] = 0xFF;
    assert_eq !(slice[10 * ONE_K - 1], 0xFF);
}

#[test]
#[cfg(target_os = "linux")]
fn test_mremap_shrink() {
    const ONE_K : size_t = 1024;
    let slice : &mut[u8] = unsafe {
        let mem = mmap(std::ptr::null_mut(), 10 * ONE_K,
                       ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
                       MapFlags::MAP_ANONYMOUS | MapFlags::MAP_PRIVATE, -1, 0)
                      .unwrap();
        std::slice::from_raw_parts_mut(mem as * mut u8, ONE_K)
    };
    assert_eq !(slice[ONE_K - 1], 0x00);
    slice[ONE_K - 1] = 0xFF;
    assert_eq !(slice[ONE_K - 1], 0xFF);

    let slice : &mut[u8] = unsafe {
        let mem = mremap(slice.as_mut_ptr() as * mut c_void, 10 * ONE_K, ONE_K,
                         MRemapFlags::empty(), None)
                      .unwrap();
        // Since we didn't supply MREMAP_MAYMOVE, the address should be the
        // same.
        assert_eq !(mem, slice.as_mut_ptr() as * mut c_void);
        std::slice::from_raw_parts_mut(mem as * mut u8, ONE_K)
    };

    // The first KB should still be accessible and have the old data in it.
    assert_eq !(slice[ONE_K - 1], 0xFF);
}
