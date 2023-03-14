// Based off lemire's wyrand C++ code at https://github.com/lemire/testingRNG/blob/master/source/wyrand.h

use crate::rand::{Rng, SeedableRng};
use core::fmt::{self, Debug, Display, Formatter};
#[cfg(feature = "zeroize")]
use zeroize::Zeroize;

/// An instance of the WyRand random number generator.
/// Seeded from the system entropy generator when available.
/// **This generator is _NOT_ cryptographically secure.**
#[cfg_attr(feature = "zeroize", derive(Zeroize))]
#[cfg_attr(feature = "zeroize", zeroize(drop))]
pub struct WyRand {
	seed: u64,
}

impl WyRand {
	/// Create a new [`WyRand`] instance, seeding from the system's default source of entropy.
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}

	/// Create a new [`WyRand`] instance, using a provided seed.
	#[must_use]
	pub const fn new_seed(seed: u64) -> Self {
		Self { seed }
	}
}

impl Default for WyRand {
	/// Create a new [`WyRand`] instance, seeding from the system's default source of entropy.
	fn default() -> Self {
		let mut entropy: [u8; core::mem::size_of::<u64>()] = Default::default();
		crate::entropy::system(&mut entropy);
		Self {
			seed: u64::from_ne_bytes(entropy),
		}
	}
}

impl Rng<8> for WyRand {
	fn rand(&mut self) -> [u8; 8] {
		self.seed = self.seed.wrapping_add(0xa0761d6478bd642f);
		let t: u128 = (self.seed as u128).wrapping_mul((self.seed ^ 0xe7037ed1a0b428db) as u128);
		let ret = (t.wrapping_shr(64) ^ t) as u64;
		ret.to_ne_bytes()
	}
}

impl Clone for WyRand {
	fn clone(&self) -> Self {
		Self { seed: self.seed }
	}
}

impl Display for WyRand {
	fn fmt(&self, f: &mut Formatter) -> fmt::Result {
		write!(f, "WyRand ({:p})", self)
	}
}

impl Debug for WyRand {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		f.debug_struct("WyRand")
			.field("seed", &format_args!("0x{:x}", self.seed))
			.finish()
	}
}

impl SeedableRng<8, 8> for WyRand {
	fn reseed(&mut self, seed: [u8; 8]) {
		self.seed = u64::from_ne_bytes(seed);
	}
}
