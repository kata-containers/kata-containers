extern "C" {
	fn getrandom(buf: *mut u8, buflen: usize, flags: u32) -> isize;
}

/// Obtain a series of random bytes.
pub fn entropy(out: &mut [u8]) -> bool {
	unsafe { getrandom(out.as_mut_ptr(), out.len(), 0x0001) >= 1 }
}
