use crate::rand::{Rng, SeedableRng};
use alloc::vec::Vec;
use core::default::Default;

/// A buffered wrapper for any [Rng] implementation.
/// It will keep unused bytes from the last call to [`Rng::rand`], and use them
/// for subsequent randomness if needed, rather than throwing them away.
///
/// ```rust
/// use nanorand::{Rng, BufferedRng, WyRand};
///
/// let mut thingy = [0u8; 5];
/// let mut rng = BufferedRng::new(WyRand::new());
/// rng.fill(&mut thingy);
/// // As WyRand generates 8 bytes of output, and our target is only 5 bytes,
/// // 3 bytes will remain in the buffer.
/// assert_eq!(rng.buffered(), 3);
/// ```
#[derive(Clone)]
pub struct BufferedRng<InternalGenerator: Rng<OUTPUT>, const OUTPUT: usize> {
	rng: InternalGenerator,
	buffer: Vec<u8>,
}

impl<InternalGenerator: Rng<OUTPUT>, const OUTPUT: usize> BufferedRng<InternalGenerator, OUTPUT> {
	/// Wraps a [`Rng`] InternalGenerator in a [`BufferedRng`] instance.
	pub fn new(rng: InternalGenerator) -> Self {
		Self {
			rng,
			buffer: Vec::new(),
		}
	}

	/// Returns the internal RNG, dropping the buffer.
	pub fn into_inner(self) -> InternalGenerator {
		self.rng
	}

	/// Returns how many unused bytes are currently buffered.
	pub fn buffered(&self) -> usize {
		self.buffer.len()
	}
}

impl<InternalGenerator: Rng<OUTPUT>, const OUTPUT: usize> Rng<OUTPUT>
	for BufferedRng<InternalGenerator, OUTPUT>
{
	fn rand(&mut self) -> [u8; OUTPUT] {
		let mut out = [0_u8; OUTPUT];
		self.fill_bytes(&mut out);
		out
	}

	fn fill_bytes<Bytes>(&mut self, mut output: Bytes)
	where
		Bytes: AsMut<[u8]>,
	{
		let output = output.as_mut();
		let mut remaining = output.len();
		while remaining > 0 {
			if self.buffer.is_empty() {
				self.buffer.extend_from_slice(&self.rng.rand());
			}
			let to_copy = core::cmp::min(remaining, self.buffer.len());
			let output_len = output.len();
			let start_idx = output_len - remaining;
			output[start_idx..start_idx + to_copy].copy_from_slice(&self.buffer[..to_copy]);
			self.buffer.drain(..to_copy);
			remaining = remaining.saturating_sub(to_copy);
		}
	}
}

#[cfg(feature = "std")]
impl<InternalGenerator: Rng<OUTPUT>, const OUTPUT: usize> std::io::Read
	for BufferedRng<InternalGenerator, OUTPUT>
{
	fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
		self.fill_bytes(&mut *output);
		Ok(output.len())
	}

	fn read_to_end(&mut self, buf: &mut Vec<u8>) -> std::io::Result<usize> {
		buf.extend_from_slice(&self.buffer);
		Ok(self.buffer.drain(..).count())
	}

	fn read_to_string(&mut self, _buf: &mut String) -> std::io::Result<usize> {
		panic!("attempted to read an rng into a string")
	}
}

impl<
		InternalGenerator: SeedableRng<SEED_SIZE, OUTPUT>,
		const OUTPUT: usize,
		const SEED_SIZE: usize,
	> SeedableRng<SEED_SIZE, OUTPUT> for BufferedRng<InternalGenerator, OUTPUT>
{
	fn reseed(&mut self, seed: [u8; SEED_SIZE]) {
		self.rng.reseed(seed);
	}
}

impl<InternalGenerator: Rng<OUTPUT> + Default, const OUTPUT: usize> Default
	for BufferedRng<InternalGenerator, OUTPUT>
{
	fn default() -> Self {
		Self::new(InternalGenerator::default())
	}
}
