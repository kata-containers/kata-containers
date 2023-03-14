//! Internal support utilities.

use crate::{
	order::BitOrder,
	store::BitStore,
};

use core::{
	any::TypeId,
	ops::{
		Bound,
		Range,
		RangeBounds,
	},
};

/** Normalizes any range into a basic `Range`.

This unpacks any range type into an ordinary `Range`, returning the start and
exclusive end markers. If the start marker is not provided, it is assumed to be
zero; if the end marker is not provided, then it is assumed to be `end`.

The end marker, if provided, may be greater than `end`. This is not checked in
the function, and must be inspected by the caller.

# Type Parameters

- `R`: A range of some kind

# Parameters

- `bounds`: A range of some kind
- `end`: The value to use as the exclusive end, if the range does not have an
  end.

# Returns

`bounds` normalized to an ordinary `Range`, optionally clamped to `end`.
**/
#[inline]
pub fn normalize_range<R>(bounds: R, end: usize) -> Range<usize>
where R: RangeBounds<usize> {
	let min = match bounds.start_bound() {
		Bound::Included(&n) => n,
		Bound::Excluded(&n) => n + 1,
		Bound::Unbounded => 0,
	};
	let max = match bounds.end_bound() {
		Bound::Included(&n) => n + 1,
		Bound::Excluded(&n) => n,
		Bound::Unbounded => end,
	};
	min .. max
}

/** Asserts that a range satisfies bounds constraints.

This requires that the range start be not greater than the range end, and the
range end be not greater than the ending marker (if provided).

# Parameters

- `range`: The range to validate
- `end`: An optional maximal value that the range cannot exceed

# Panics

This panics if the range fails a requirement.
**/
#[inline]
pub fn assert_range(range: Range<usize>, end: impl Into<Option<usize>>) {
	if range.start > range.end {
		panic!(
			"Malformed range: `{} .. {}` must run from lower to higher",
			range.start, range.end
		);
	}
	if let Some(end) = end.into() {
		if range.end > end {
			panic!(
				"Range out of bounds: `{} .. {}` must not exceed `{}`",
				range.start, range.end, end
			);
		}
	}
}

/// Tests if two `BitOrder` type parameters match each other.
///
/// This evaluates to a compile-time constant, and is removed during codegen.
#[inline(always)]
pub fn match_order<O1, O2>() -> bool
where
	O1: BitOrder,
	O2: BitOrder,
{
	TypeId::of::<O1>() == TypeId::of::<O2>()
}

/// Tests if two `<O, T>` type parameter pairs match each other.
///
/// This evaluates to a compile-time constant, and is removed during codegen.
#[inline(always)]
pub fn match_types<O1, T1, O2, T2>() -> bool
where
	O1: BitOrder,
	T1: BitStore,
	O2: BitOrder,
	T2: BitStore,
{
	match_order::<O1, O2>() && TypeId::of::<T1>() == TypeId::of::<T2>()
}

#[cfg(all(test, feature = "std"))]
mod tests {
	use super::*;
	use std::panic::catch_unwind;

	#[test]
	fn check_range_asserts() {
		assert!(catch_unwind(|| assert_range(7 .. 2, None)).is_err());
		assert!(catch_unwind(|| assert_range(0 .. 8, 4)).is_err());
	}
}
