use crate::Rng;
use core::ops::{Bound, RangeBounds};

macro_rules! gen {
	($($type:ty),+) => {
		$(
			impl<Generator: Rng<OUTPUT>, const OUTPUT: usize> RandomGen<Generator, OUTPUT> for $type {
				fn random(rng: &mut Generator) -> Self {
					let mut bytes = [0u8; core::mem::size_of::<$type>()];
					rng.fill_bytes(&mut bytes);
					Self::from_ne_bytes(bytes)
				}
			}
		)+
	};
}

macro_rules! range {
	($(($type:ty, $bigger:ty, $signed:ty)),+) => {
		$(
			impl<Generator: Rng<OUTPUT>, const OUTPUT: usize> RandomRange<Generator, OUTPUT> for $type {
				fn random_range<Bounds: RangeBounds<Self>>(rng: &mut Generator, bounds: Bounds) -> Self {
					const BITS: $bigger = core::mem::size_of::<$type>() as $bigger * 8;
					let lower = match bounds.start_bound() {
						Bound::Included(lower) => *lower,
						Bound::Excluded(lower) => lower.saturating_add(1),
						Bound::Unbounded => <$type>::MIN,
					};
					let upper = match bounds.end_bound() {
						Bound::Included(upper) => upper.saturating_add(1),
						Bound::Excluded(upper) => *upper,
						Bound::Unbounded => <$type>::MAX,
					};
					assert!(upper >= lower, "{} >= {} (lower bound was bigger than upper bound)", upper, lower);
					let upper = upper.saturating_sub(lower);
					let mut value = Self::random(rng);
					let mut m = (upper as $bigger).wrapping_mul(value as $bigger);
					if (m as $type) < upper {
						let t = (!upper + 1) % upper;
						while (m as $type) < t {
							value = Self::random(rng);
							m = (upper as $bigger).wrapping_mul(value as $bigger);
						}
					}
					(m >> BITS) as $type + lower
				}
			}

			impl<Generator: Rng<OUTPUT>, const OUTPUT: usize> RandomRange<Generator, OUTPUT> for $signed {
				fn random_range<Bounds: RangeBounds<Self>>(r: &mut Generator, bounds: Bounds) -> Self {
					let lower = match bounds.start_bound() {
						Bound::Included(lower) => *lower,
						Bound::Excluded(lower) => lower.saturating_add(1),
						Bound::Unbounded => <$signed>::MIN
					};
					let upper = match bounds.end_bound() {
						Bound::Included(upper) => *upper,
						Bound::Excluded(upper) => upper.saturating_sub(1),
						Bound::Unbounded => <$signed>::MAX,
					};
					assert!(upper >= lower, "{} >= {} (lower bound was bigger than upper bound)", upper, lower);
					let lower = lower.wrapping_sub(<$signed>::MIN) as $type;
					let upper = upper.wrapping_sub(<$signed>::MIN) as $type;
					<$type>::random_range(r, lower..=upper).wrapping_add(<$signed>::MAX as $type) as $signed
				}
			}
		)+
	}
}

/// A trait used for generating a random object with an RNG,
pub trait RandomGen<Generator: Rng<OUTPUT>, const OUTPUT: usize> {
	/// Return a random instance of the implementing type, from the specified RNG instance.
	fn random(rng: &mut Generator) -> Self;
}

/// A trait used for generating a random number within a range, with an RNG,
pub trait RandomRange<Generator: Rng<OUTPUT>, const OUTPUT: usize>:
	RandomGen<Generator, OUTPUT>
{
	/// Return a ranged number of the implementing type, from the specified RNG instance.
	///
	/// # Panics
	/// This function will panic if the lower bound of the range is greater than the upper bound.
	fn random_range<Bounds: RangeBounds<Self>>(nng: &mut Generator, range: Bounds) -> Self;
}

impl<Generator: Rng<OUTPUT>, const OUTPUT: usize> RandomGen<Generator, OUTPUT> for bool {
	fn random(rng: &mut Generator) -> Self {
		u8::random(rng) < 0b10000000
	}
}

impl<Generator: Rng<OUTPUT>, const OUTPUT: usize> RandomGen<Generator, OUTPUT> for f32 {
	fn random(rng: &mut Generator) -> Self {
		(u32::random(rng) as f32) / (u32::MAX as f32)
	}
}

impl<Generator: Rng<OUTPUT>, const OUTPUT: usize> RandomGen<Generator, OUTPUT> for f64 {
	fn random(rng: &mut Generator) -> Self {
		(u64::random(rng) as f64) / (u64::MAX as f64)
	}
}

gen!(i8, u8, i16, u16, i32, u32, i64, u64, i128, u128, isize, usize);
range!(
	(u8, u16, i8),
	(u16, u32, i16),
	(u32, u64, i32),
	(u64, u128, i64)
);
#[cfg(target_pointer_width = "16")]
range!((usize, u32, isize));
#[cfg(target_pointer_width = "32")]
range!((usize, u64, isize));
#[cfg(target_pointer_width = "64")]
range!((usize, u128, isize));

#[cfg(test)]
mod tests {
	use crate::{Rng, WyRand};
	#[test]
	fn ensure_unsigned_in_range() {
		let mut rng = WyRand::new();
		for _ in 0..1000 {
			let number = rng.generate_range(10_u64..=20);
			assert!(
				(10..=20).contains(&number),
				"{} was outside of 10..=20",
				number
			);

			let number = rng.generate_range(10_u64..30);
			assert!(
				(10..30).contains(&number),
				"{} was outside of 10..30",
				number
			);

			let number = rng.generate_range(512_u64..);
			assert!((512..).contains(&number), "{} was outside of 512..", number);

			let number = rng.generate_range(..1024_u64);
			assert!(
				(..1024).contains(&number),
				"{} was outside of ..1024",
				number
			);
		}
	}
	#[test]
	fn ensure_signed_in_range() {
		let mut rng = WyRand::new();
		for _ in 0..1000 {
			let number = rng.generate_range(-50..);
			assert!((-50..).contains(&number), "{} was outside of -50..", number);

			let number = rng.generate_range(..512);
			assert!((..512).contains(&number), "{} was outside of ..512", number);

			let number = rng.generate_range(..-32);
			assert!((..-32).contains(&number), "{} was outside of ..-32", number);
		}
	}

	#[test]
	fn ensure_floats_generate_properly() {
		let mut rng = WyRand::new();
		for _ in 0..1000 {
			let number = rng.generate::<f32>();
			assert!(1.0 >= number, "{} was bigger than 1.0", number);
			assert!(number >= 0.0, "0 was bigger than {}", number);

			let number = rng.generate::<f64>();
			assert!(1.0 >= number, "{} was bigger than 1.0", number);
			assert!(number >= 0.0, "0 was bigger than {}", number);
		}
	}

	#[test]
	#[should_panic]
	fn ensure_invalid_range_panics() {
		let mut rng = WyRand::new();
		#[allow(clippy::reversed_empty_ranges)]
		rng.generate_range(10..=5);
	}
}
