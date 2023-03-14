use crate::{
	crypto::chacha,
	rand::{Rng, SeedableRng},
};
use core::fmt::{self, Debug, Display, Formatter};
#[cfg(feature = "zeroize")]
use zeroize::Zeroize;

/// The ChaCha CSPRNG, with 8 rounds.
pub type ChaCha8 = ChaCha<8>;

/// The ChaCha CSPRNG, with 12 rounds.
pub type ChaCha12 = ChaCha<12>;

/// The ChaCha CSPRNG, with 20 rounds.
pub type ChaCha20 = ChaCha<20>;

/// An instance of the ChaCha random number generator.
/// Seeded from the system entropy generator when available.
/// **This generator _is theoretically_ cryptographically secure.**
#[cfg_attr(feature = "zeroize", derive(Zeroize))]
#[cfg_attr(feature = "zeroize", zeroize(drop))]
pub struct ChaCha<const ROUNDS: u8> {
	state: [u32; 16],
}

impl<const ROUNDS: u8> ChaCha<ROUNDS> {
	/// Create a new [`ChaCha`] instance, seeding from the system's default source of entropy.
	#[must_use]
	pub fn new() -> Self {
		let mut key: [u8; 32] = Default::default();
		crate::entropy::system(&mut key);
		let mut nonce: [u8; 8] = Default::default();
		crate::entropy::system(&mut nonce);
		let state = chacha::chacha_init(key, nonce);
		Self { state }
	}

	/// Create a new [`ChaCha`] instance, using the provided key and nonce.
	#[must_use]
	pub const fn new_key(key: [u8; 32], nonce: [u8; 8]) -> Self {
		let state = chacha::chacha_init(key, nonce);
		Self { state }
	}
}

impl<const ROUNDS: u8> Default for ChaCha<ROUNDS> {
	fn default() -> Self {
		let mut key: [u8; 32] = Default::default();
		crate::entropy::system(&mut key);
		let mut nonce: [u8; 8] = Default::default();
		crate::entropy::system(&mut nonce);
		let state = chacha::chacha_init(key, nonce);
		Self { state }
	}
}

impl<const ROUNDS: u8> Rng<64> for ChaCha<ROUNDS> {
	fn rand(&mut self) -> [u8; 64] {
		let block = chacha::chacha_block::<ROUNDS>(self.state);
		let mut ret = [0_u8; 64];
		block.iter().enumerate().for_each(|(idx, num)| {
			let x = num.to_ne_bytes();
			let n = idx * 4;
			ret[n] = x[0];
			ret[n + 1] = x[1];
			ret[n + 2] = x[2];
			ret[n + 3] = x[3];
		});
		// Now, we're going to just increment our counter so we get an entirely new output next time.
		// If the counter overflows, we just reseed entirely instead.
		if !chacha::chacha_increment_counter(&mut self.state) {
			let mut new_seed: [u8; 40] = [42_u8; 40];
			crate::entropy::system(&mut new_seed);
			self.reseed(new_seed);
		}
		ret
	}
}

impl<const ROUNDS: u8> Clone for ChaCha<ROUNDS> {
	fn clone(&self) -> Self {
		Self { state: self.state }
	}
}

impl<const ROUNDS: u8> Display for ChaCha<ROUNDS> {
	fn fmt(&self, f: &mut Formatter) -> fmt::Result {
		write!(f, "ChaCha ({:p}, {} rounds)", self, ROUNDS)
	}
}

impl<const ROUNDS: u8> SeedableRng<40, 64> for ChaCha<ROUNDS> {
	fn reseed(&mut self, seed: [u8; 40]) {
		let mut key = [0_u8; 32];
		let mut nonce = [0_u8; 8];
		key.copy_from_slice(&seed[..32]);
		nonce.copy_from_slice(&seed[32..]);
		self.state = chacha::chacha_init(key, nonce);
	}
}

impl<const ROUNDS: u8> Debug for ChaCha<ROUNDS> {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		let counter = ((self.state[13] as u64) << 32) | (self.state[12] as u64);
		f.debug_struct("ChaCha20")
			.field("rounds", &ROUNDS)
			.field("counter", &counter)
			.finish()
	}
}
