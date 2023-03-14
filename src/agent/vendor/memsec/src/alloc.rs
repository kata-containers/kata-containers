//! alloc

#![cfg(feature = "alloc")]

extern crate std;

use core::mem;
use core::ptr::{ self, NonNull };
use core::slice;
use getrandom::getrandom;
use self::std::sync::Once;
use self::std::process::abort;
use self::raw_alloc::*;


const GARBAGE_VALUE: u8 = 0xd0;
const CANARY_SIZE: usize = 16;
static ALLOC_INIT: Once = Once::new();
static mut PAGE_SIZE: usize = 0;
static mut PAGE_MASK: usize = 0;
static mut CANARY: [u8; CANARY_SIZE] = [0; CANARY_SIZE];


// -- alloc init --

#[inline]
unsafe fn alloc_init() {
    #[cfg(unix)] {
        PAGE_SIZE = libc::sysconf(libc::_SC_PAGESIZE) as usize;
    }

    #[cfg(windows)] {
        let mut si = mem::MaybeUninit::uninit();
        windows_sys::Win32::System::SystemInformation::GetSystemInfo(si.as_mut_ptr());
        PAGE_SIZE = (*si.as_ptr()).dwPageSize as usize;
    }

    if PAGE_SIZE < CANARY_SIZE || PAGE_SIZE < mem::size_of::<usize>() {
        panic!("page size too small");
    }

    PAGE_MASK = PAGE_SIZE - 1;

    getrandom(&mut CANARY).unwrap();
}


// -- aligned alloc / aligned free --

mod raw_alloc {
    use super::std::alloc::{ alloc, dealloc, Layout };
    use super::*;

    #[inline]
    pub unsafe fn alloc_aligned(size: usize) -> Option<NonNull<u8>> {
        let layout = Layout::from_size_align_unchecked(size, PAGE_SIZE);
        NonNull::new(alloc(layout))
    }

    #[inline]
    pub unsafe fn free_aligned(memptr: *mut u8, size: usize) {
        let layout = Layout::from_size_align_unchecked(size, PAGE_SIZE);
        dealloc(memptr, layout);
    }
}


// -- mprotect --

/// Prot enum.
#[cfg(unix)]
#[allow(non_snake_case, non_upper_case_globals)]
pub mod Prot {
    pub use libc::c_int as Ty;

    pub const NoAccess: Ty = libc::PROT_NONE;
    pub const ReadOnly: Ty = libc::PROT_READ;
    pub const WriteOnly: Ty = libc::PROT_WRITE;
    pub const ReadWrite: Ty = libc::PROT_READ | libc::PROT_WRITE;
    pub const Execute: Ty = libc::PROT_EXEC;
    pub const ReadExec: Ty = libc::PROT_READ | libc::PROT_EXEC;
    pub const WriteExec: Ty = libc::PROT_WRITE | libc::PROT_EXEC;
    pub const ReadWriteExec: Ty = libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC;
}

/// Prot enum.
#[cfg(windows)]
#[allow(non_snake_case, non_upper_case_globals)]
pub mod Prot {
    pub use windows_sys::Win32::System::Memory::PAGE_PROTECTION_FLAGS as Ty;

    pub const NoAccess: Ty = windows_sys::Win32::System::Memory::PAGE_NOACCESS;
    pub const ReadOnly: Ty = windows_sys::Win32::System::Memory::PAGE_READONLY;
    pub const ReadWrite: Ty = windows_sys::Win32::System::Memory::PAGE_READWRITE;
    pub const WriteCopy: Ty = windows_sys::Win32::System::Memory::PAGE_WRITECOPY;
    pub const Execute: Ty = windows_sys::Win32::System::Memory::PAGE_EXECUTE;
    pub const ReadExec: Ty = windows_sys::Win32::System::Memory::PAGE_EXECUTE_READ;
    pub const ReadWriteExec: Ty = windows_sys::Win32::System::Memory::PAGE_EXECUTE_READWRITE;
    pub const WriteCopyExec: Ty = windows_sys::Win32::System::Memory::PAGE_EXECUTE_WRITECOPY;
    pub const Guard: Ty = windows_sys::Win32::System::Memory::PAGE_GUARD;
    pub const NoCache: Ty = windows_sys::Win32::System::Memory::PAGE_NOCACHE;
    pub const WriteCombine: Ty = windows_sys::Win32::System::Memory::PAGE_WRITECOMBINE;
    pub const RevertToFileMap: Ty = windows_sys::Win32::System::Memory::PAGE_REVERT_TO_FILE_MAP;
    pub const TargetsInvalid: Ty = windows_sys::Win32::System::Memory::PAGE_TARGETS_INVALID;
    pub const TargetsNoUpdate: Ty = windows_sys::Win32::System::Memory::PAGE_TARGETS_NO_UPDATE;
}


/// Unix `mprotect`.
#[cfg(unix)]
#[inline]
pub unsafe fn _mprotect(ptr: *mut u8, len: usize, prot: Prot::Ty) -> bool {
    libc::mprotect(ptr as *mut libc::c_void, len, prot as libc::c_int) == 0
}

/// Windows `VirtualProtect`.
#[cfg(windows)]
#[inline]
pub unsafe fn _mprotect(ptr: *mut u8, len: usize, prot: Prot::Ty) -> bool {
    let mut old = mem::MaybeUninit::uninit();
    windows_sys::Win32::System::Memory::VirtualProtect(ptr.cast(), len, prot, old.as_mut_ptr()) != 0
}


/// Secure `mprotect`.
#[cfg(any(unix, windows))]
pub unsafe fn mprotect<T: ?Sized>(memptr: NonNull<T>, prot: Prot::Ty) -> bool {
    let memptr = memptr.as_ptr() as *mut u8;

    let unprotected_ptr = unprotected_ptr_from_user_ptr(memptr);
    let base_ptr = unprotected_ptr.sub(PAGE_SIZE * 2);
    let unprotected_size = ptr::read(base_ptr as *const usize);
    _mprotect(unprotected_ptr, unprotected_size, prot)
}


// -- malloc / free --

#[inline]
unsafe fn page_round(size: usize) -> usize {
    (size + PAGE_MASK) & !PAGE_MASK
}

#[inline]
unsafe fn unprotected_ptr_from_user_ptr(memptr: *const u8) -> *mut u8 {
    let canary_ptr = memptr.sub(CANARY_SIZE);
    let unprotected_ptr_u = canary_ptr as usize & !PAGE_MASK;
    if unprotected_ptr_u <= PAGE_SIZE * 2 {
        abort();
    }
    unprotected_ptr_u as *mut u8
}

unsafe fn _malloc(size: usize) -> Option<*mut u8> {
    ALLOC_INIT.call_once(|| alloc_init());

    if size >= ::core::usize::MAX - PAGE_SIZE * 4 {
        return None;
    }

    // aligned alloc ptr
    let size_with_canary = CANARY_SIZE + size;
    let unprotected_size = page_round(size_with_canary);
    let total_size = PAGE_SIZE + PAGE_SIZE + unprotected_size + PAGE_SIZE;
    let base_ptr = alloc_aligned(total_size)?.as_ptr();
    let unprotected_ptr = base_ptr.add(PAGE_SIZE * 2);

    // mprotect ptr
    _mprotect(base_ptr.add(PAGE_SIZE), PAGE_SIZE, Prot::NoAccess);
    _mprotect(unprotected_ptr.add(unprotected_size), PAGE_SIZE, Prot::NoAccess);
    crate::mlock(unprotected_ptr, unprotected_size);

    let canary_ptr = unprotected_ptr.add(unprotected_size - size_with_canary);
    let user_ptr = canary_ptr.add(CANARY_SIZE);
    ptr::copy_nonoverlapping(CANARY.as_ptr(), canary_ptr, CANARY_SIZE);
    ptr::write_unaligned(base_ptr as *mut usize, unprotected_size);
    _mprotect(base_ptr, PAGE_SIZE, Prot::ReadOnly);

    assert_eq!(unprotected_ptr_from_user_ptr(user_ptr), unprotected_ptr);

    Some(user_ptr)
}

/// Secure `malloc`.
#[inline]
pub unsafe fn malloc<T>() -> Option<NonNull<T>> {
    _malloc(mem::size_of::<T>())
        .map(|memptr| {
            ptr::write_bytes(memptr, GARBAGE_VALUE, mem::size_of::<T>());
            NonNull::new_unchecked(memptr as *mut T)
        })
}

/// Secure `malloc_sized`.
#[inline]
pub unsafe fn malloc_sized(size: usize) -> Option<NonNull<[u8]>> {
    _malloc(size)
        .map(|memptr| {
            ptr::write_bytes(memptr, GARBAGE_VALUE, size);
            NonNull::new_unchecked(slice::from_raw_parts_mut(memptr, size))
        })
}

/// Secure `free`.
pub unsafe fn free<T: ?Sized>(memptr: NonNull<T>) {
    let memptr = memptr.as_ptr() as *mut u8;

    // get unprotected ptr
    let canary_ptr = memptr.sub(CANARY_SIZE);
    let unprotected_ptr = unprotected_ptr_from_user_ptr(memptr);
    let base_ptr = unprotected_ptr.sub(PAGE_SIZE * 2);
    let unprotected_size = ptr::read(base_ptr as *const usize);

    // check
    if !crate::memeq(canary_ptr as *const u8, CANARY.as_ptr(), CANARY_SIZE) {
        abort();
    }

    // free
    let total_size = PAGE_SIZE + PAGE_SIZE + unprotected_size + PAGE_SIZE;
    _mprotect(base_ptr, total_size, Prot::ReadWrite);

    crate::munlock(unprotected_ptr, unprotected_size);

    free_aligned(base_ptr, total_size);
}
