use core::ffi::c_void;

#[link(name = "Security", kind = "framework")]
extern "C" {
	fn SecRandomCopyBytes(rnd: *const c_void, count: usize, bytes: *mut u8) -> u32;
}

/// Obtain a series of random bytes.
pub fn entropy(out: &mut [u8]) -> bool {
	unsafe { SecRandomCopyBytes(core::ptr::null(), out.len(), out.as_mut_ptr()) == 0 }
}
