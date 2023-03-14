// Based off Robert Kern's C implementation at https://github.com/rkern/pcg64/blob/master/pcg64.c

use crate::rand::{Rng, SeedableRng};
use core::fmt::{self, Debug, Display, Formatter};
#[cfg(feature = "zeroize")]
use zeroize::Zeroize;

const PCG_DEFAULT_MULTIPLIER_128: u128 = 47026247687942121848144207491837523525;

/// An instance of the Pcg64 random number generator.
/// Seeded from the system entropy generator when available.
/// **This generator is _NOT_ cryptographically secure.**
#[cfg_attr(feature = "zeroize", derive(Zeroize))]
#[cfg_attr(feature = "zeroize", zeroize(drop))]
pub struct Pcg64 {
	seed: u128,
	state: u128,
	inc: u128,
}

impl Pcg64 {
	/// Create a new [`Pcg64`] instance, seeding from the system's default source of entropy.
	#[cfg(feature = "std")]
	#[must_use]
	pub fn new() -> Self {
		let mut entropy: [u8; core::mem::size_of::<u128>()] = Default::default();
		crate::entropy::system(&mut entropy);
		Self {
			seed: u128::from_ne_bytes(entropy),
			inc: 0,
			state: 0,
		}
	}

	/// Create a new [`Pcg64`] instance, using a provided seed.
	#[must_use]
	pub const fn new_seed(seed: u128) -> Self {
		Self {
			seed,
			inc: 0,
			state: 0,
		}
	}

	fn step(&mut self) {
		self.state = self
			.state
			.wrapping_mul(PCG_DEFAULT_MULTIPLIER_128)
			.wrapping_add(self.inc);
	}

	fn rand128(&mut self) -> u64 {
		self.state = 0;
		self.inc = self.seed.wrapping_shl(1) | 1;
		self.step();
		self.state = self.state.wrapping_add(self.seed);
		self.step();
		self.step();
		self.state.wrapping_shr(64) as u64 ^ self.state as u64
	}
}

#[cfg(feature = "std")]
impl Default for Pcg64 {
	/// Create a new [`Pcg64`] instance, seeding from the system's default source of entropy.
	fn default() -> Self {
		let mut entropy: [u8; core::mem::size_of::<u128>()] = Default::default();
		crate::entropy::system(&mut entropy);
		Self {
			seed: u128::from_ne_bytes(entropy),
			inc: 0,
			state: 0,
		}
	}
}

impl Rng<8> for Pcg64 {
	fn rand(&mut self) -> [u8; 8] {
		let ret = self.rand128();
		self.seed = self.state ^ (ret as u128).wrapping_shr(64);
		ret.to_ne_bytes()
	}
}

impl SeedableRng<16, 8> for Pcg64 {
	fn reseed(&mut self, seed: [u8; 16]) {
		self.seed = u128::from_ne_bytes(seed);
	}
}

impl Clone for Pcg64 {
	fn clone(&self) -> Self {
		Self {
			seed: self.seed,
			inc: self.inc,
			state: self.state,
		}
	}
}

impl Display for Pcg64 {
	fn fmt(&self, f: &mut Formatter) -> fmt::Result {
		write!(f, "Pcg64 ({:p})", self)
	}
}

impl Debug for Pcg64 {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		f.debug_struct("Pcg64")
			.field("seed", &format_args!("0x{:x}", self.seed))
			.field("state", &format_args!("0x{:x}", self.state))
			.field("inc", &format_args!("0x{:x}", self.inc))
			.finish()
	}
}
