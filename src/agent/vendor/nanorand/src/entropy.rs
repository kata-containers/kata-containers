#[cfg(all(target_vendor = "apple", not(feature = "getrandom")))]
pub use darwin::entropy as system;
#[cfg(all(
	any(target_os = "linux", target_os = "android"),
	not(feature = "getrandom")
))]
pub use linux::entropy as system;
#[cfg(all(windows, not(target_vendor = "uwp"), not(feature = "getrandom")))]
pub use windows::entropy as system;
#[cfg(all(windows, target_vendor = "uwp", not(feature = "getrandom")))]
pub use windows_uwp::entropy as system;

#[cfg(all(
	any(target_os = "linux", target_os = "android"),
	not(feature = "getrandom")
))]
/// An entropy generator for Linux, using libc's `getrandom` function.
pub mod linux;

#[cfg(all(target_vendor = "apple", not(feature = "getrandom")))]
/// An entropy generator for macOS/iOS, using libc's `getrandom` function.
pub mod darwin;

#[cfg(all(windows, target_vendor = "uwp", not(feature = "getrandom")))]
/// An entropy generator for Windows, using WinAPI's `BCryptGenRandom` function.
pub mod windows_uwp;

#[cfg(all(windows, not(target_vendor = "uwp"), not(feature = "getrandom")))]
/// An entropy generator for Windows, using WinAPI's `RtlGenRandom` function.
pub mod windows;

#[cfg(feature = "getrandom")]
/// Pull in system entropy using the [`getrandom`](https://crates.io/crates/getrandom) crate.
/// Uses backup entropy (rdseed and system time) if it fails.
pub fn system(out: &mut [u8]) {
	match getrandom::getrandom(out) {
		Ok(_) => (),
		Err(_) => backup(out),
	}
}

/// Pull in backup entropy (rdseed and system time).
#[cfg(not(any(
	feature = "getrandom",
	target_os = "linux",
	target_os = "android",
	target_vendor = "apple",
	windows
)))]
pub fn system(out: &mut [u8]) {
	backup_entropy(out);
}

#[cfg(feature = "rdseed")]
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn stupid_rdseed_hack() -> Option<u64> {
	#[cfg(target_arch = "x86")]
	use core::arch::x86::_rdseed64_step as rdseed;
	#[cfg(target_arch = "x86_64")]
	use core::arch::x86_64::_rdseed64_step as rdseed;
	let mut x = 0;
	for _ in 0..10 {
		if 0 != unsafe { rdseed(&mut x) } {
			return Some(x);
		}
	}
	None
}

#[cfg(all(feature = "rdseed", any(target_arch = "x86", target_arch = "x86_64")))]
/// An rdseed-based entropy source.
/// Only works on x86/x86_64 platforms where the `rdseed` instructions are available.
/// Returns [`None`] if `rdseed` is not available.
/// Returns [`Some`] if it successfully managed to pull some bytes.
/// ***VERY unreliable.***
pub fn rdseed(out: &mut [u8]) -> Option<usize> {
	if !std::is_x86_feature_detected!("rdseed") {
		return None;
	}
	let amt = out.len();
	let mut bytes_pulled: usize = 0;

	let rdseed_amt = ((amt + core::mem::size_of::<u64>() - 1) / core::mem::size_of::<u64>()).max(0);
	for n in 0..rdseed_amt {
		let seed = match stupid_rdseed_hack() {
			Some(s) => s,
			None => return Some(bytes_pulled),
		};
		let x = seed.to_ne_bytes();
		bytes_pulled += x.len();
		x.iter()
			.enumerate()
			.for_each(|(i, val)| out[(core::mem::size_of::<u64>() * n) + i] = *val);
	}
	Some(bytes_pulled)
}

/// A wrapper function for non-x86(64) platforms that do not have rdseed.
#[cfg(any(
	not(feature = "rdseed"),
	not(any(target_arch = "x86", target_arch = "x86_64"))
))]
pub fn rdseed(_out: &mut [u8]) -> Option<usize> {
	None
}

#[cfg(feature = "std")]
/// A backup entropy source, trying rdseed first,
/// and if it fails or does not complete, combining it with or
/// using system time-based entropy generation.
///
/// # Panics
///
/// This function panics if sufficient entropy could not be obtained.
pub fn backup(out: &mut [u8]) {
	if let Some(amt) = rdseed(out) {
		if amt >= out.len() {
			return;
		}
	};

	panic!("Failed to source sufficient entropy!")
}

#[cfg(not(feature = "std"))]
/// This just panics.
pub fn backup_entropy(_: &mut [u8]) {
	panic!("Failed to source any entropy!")
}
