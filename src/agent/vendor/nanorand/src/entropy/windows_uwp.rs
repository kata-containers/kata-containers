use core::{ffi::c_void, ptr};

use super::backup_entropy;

const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 0x00000002;

extern "system" {
	fn BCryptGenRandom(
		hAlgorithm: *mut c_void,
		pBuffer: *mut u8,
		cbBuffer: usize,
		dwFlags: u32,
	) -> u32;
}

/// Obtain a random 64-bit number using WinAPI's `BCryptGenRandom` function.
pub fn entropy(out: &mut [u8]) -> bool {
	unsafe {
		BCryptGenRandom(
			ptr::null_mut(),
			out.as_mut_ptr(),
			out.len(),
			BCRYPT_USE_SYSTEM_PREFERRED_RNG,
		) == 0
	}
}
