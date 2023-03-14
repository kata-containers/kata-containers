#[cfg(feature = "chacha")]
pub use chacha::{ChaCha, ChaCha12, ChaCha20, ChaCha8};
#[cfg(feature = "pcg64")]
pub use pcg64::Pcg64;
#[cfg(feature = "wyrand")]
pub use wyrand::WyRand;

use crate::gen::{RandomGen, RandomRange};
use core::ops::RangeBounds;

/// Implementation of the wyrand PRNG algorithm.
/// More details can be seen at <https://github.com/wangyi-fudan/wyhash>
#[cfg(feature = "wyrand")]
pub mod wyrand;

/// Implementation of the Pcg64 PRNG algorithm.
/// More details can be seen at <https://www.pcg-random.org/index.html>
#[cfg(feature = "pcg64")]
pub mod pcg64;

/// Implementation of the ChaCha CSPRNG algorithm.
/// More details can be seen at <https://en.wikipedia.org/wiki/Salsa20>
#[cfg(feature = "chacha")]
pub mod chacha;

/// A trait that represents a random number generator.
pub trait Rng<const OUTPUT: usize>: Clone {
	/// Generates a random sequence of bytes, seeding from the internal state.
	fn rand(&mut self) -> [u8; OUTPUT];
	/// Generates a random of the specified type, seeding from the internal state.
	fn generate<Generated>(&mut self) -> Generated
	where
		Generated: RandomGen<Self, OUTPUT>,
	{
		Generated::random(self)
	}
	/// Fill an array of bytes with randomness.
	fn fill_bytes<Bytes>(&mut self, mut buffer: Bytes)
	where
		Bytes: AsMut<[u8]>,
	{
		let mut buffer = buffer.as_mut();
		let mut length = buffer.len();
		while length > 0 {
			let chunk = self.rand();
			let generated = chunk.len().min(length);
			buffer[..generated].copy_from_slice(&chunk[..generated]);
			buffer = &mut buffer[generated..];
			length -= generated;
		}
	}
	/// Fill an array with the specified type.
	fn fill<Contents, Array>(&mut self, mut target: Array)
	where
		Contents: RandomGen<Self, OUTPUT>,
		Array: AsMut<[Contents]>,
	{
		let target = target.as_mut();
		target.iter_mut().for_each(|entry| *entry = self.generate());
	}
	/// Generates a random of the specified type, seeding from the internal state.
	fn generate_range<Number, Bounds>(&mut self, range: Bounds) -> Number
	where
		Number: RandomRange<Self, OUTPUT>,
		Bounds: RangeBounds<Number>,
	{
		Number::random_range(self, range)
	}
	/// Shuffle a slice, using the RNG.
	fn shuffle<Contents, Array>(&mut self, mut target: Array)
	where
		Array: AsMut<[Contents]>,
	{
		let target = target.as_mut();
		let target_len = target.len();
		for idx in 0..target_len {
			let random_idx = self.generate_range(0..target_len);
			target.swap(idx, random_idx);
		}
	}
}

/// A trait that represents an RNG that can be reseeded from arbitrary bytes.
pub trait SeedableRng<const SEED_SIZE: usize, const OUTPUT: usize>: Rng<OUTPUT> {
	/// Re-seed the RNG with the specified bytes.
	fn reseed(&mut self, seed: [u8; SEED_SIZE]);
}
