/*! Internal implementation macros for the public exports.

The macros in this module are required to be exported from the crate, as the
public macros will call them from client contexts (`macro_rules!` expansion
bodies are not in source crate scope, as they are token expansion rather than
symbolic calls). However, they are not part of the public *API* of the crate,
and are not intended for use anywhere but in the expansion bodies of the
public-API constructor macros.
!*/

#![doc(hidden)]

#[doc(hidden)]
pub use core;

#[doc(hidden)]
pub use funty;

/** Encodes a sequence of bits into an array of `BitStore` types.

This is able to encode a bitstream into any of the fundamental integers, their
atomics, and their cells. It always produces an array of the requested type,
even if the array is one element long.
**/
#[doc(hidden)]
#[macro_export]
macro_rules! __encode_bits {
	//  Capture the `BitStore` storage arguments literally. The macro cannot
	//  accept unknown typenames, as it must use them to chunk the bitstream.

	($ord:tt, u8; $($val:expr),*) => {
		$crate::__encode_bits!($ord, u8 as u8; $($val),*)
	};
	($ord:tt, Cell<u8>; $($val:expr),*) => {
		$crate::__encode_bits!($ord, Cell<u8> as u8; $($val),*)
	};
	($ord:tt, AtomicU8; $($val:expr),*) => {
		$crate::__encode_bits!($ord, AtomicU8 as u8; $($val),*)
	};

	($ord:tt, u16; $($val:expr),*) => {
		$crate::__encode_bits!($ord, u16 as u16; $($val),*)
	};
	($ord:tt, Cell<u16>; $($val:expr),*) => {
		$crate::__encode_bits!($ord, Cell<u16> as u16; $($val),*)
	};
	($ord:tt, AtomicU16; $($val:expr),*) => {
		$crate::__encode_bits!($ord, AtomicU16 as u16; $($val),*)
	};

	($ord:tt, u32; $($val:expr),*) => {
		$crate::__encode_bits!($ord, u32 as u32; $($val),*)
	};
	($ord:tt, Cell<u32>; $($val:expr),*) => {
		$crate::__encode_bits!($ord, Cell<u32> as u32; $($val),*)
	};
	($ord:tt, AtomicU32; $($val:expr),*) => {
		$crate::__encode_bits!($ord, AtomicU32 as u32; $($val),*)
	};

	($ord:tt, u64; $($val:expr),*) => {
		$crate::__encode_bits!($ord, u64 as u64; $($val),*)
	};
	($ord:tt, Cell<u64>; $($val:expr),*) => {
		$crate::__encode_bits!($ord, Cell<u64> as u64; $($val),*)
	};
	($ord:tt, AtomicU64; $($val:expr),*) => {
		$crate::__encode_bits!($ord, AtomicU64 as u64; $($val),*)
	};

	($ord:tt, usize; $($val:expr),*) => {
		$crate::__encode_bits!($ord, usize as usize; $($val),*)
	};
	($ord:tt, Cell<usize>; $($val:expr),*) => {
		$crate::__encode_bits!($ord, Cell<usize> as usize; $($val),*)
	};
	($ord:tt, AtomicUsize; $($val:expr),*) => {
		$crate::__encode_bits!($ord, AtomicUsize as usize; $($val),*)
	};

	//  Capture `$typ as usize`, and forward them to the correct known-width
	//  integer for construction.
	($ord:tt, $typ:ty as usize; $($val:expr),*) => {{
		const LEN: usize = $crate::__count_elts!(usize; $($val),*);

		#[cfg(target_pointer_width = "32")]
		let out: [$typ; LEN] = $crate::__encode_bits!(
			$ord, $typ as u32 as usize; $($val),*
		);

		#[cfg(target_pointer_width = "64")]
		let out: [$typ; LEN] = $crate::__encode_bits!(
			$ord, $typ as u64 as usize; $($val),*
		);

		out
	}};

	/* All matchers above forward to this matcher, which then forwards to those
	below.

	This block extends the bitstream with 64 `0` literals, ensuring that *any*
	provided bitstream can fit into the chunking matchers for subdivision.
	*/
	($ord:tt, $typ:ty as $uint:ident $(as $usz:ident)?; $($val:expr),*) => {
		$crate::__encode_bits!(
			$ord, $typ as $uint $(as $usz)?, []; $($val,)*
			0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 16
			0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 32
			0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 48
			0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 64
		)
	};

	/* These two blocks are the last invoked. They require a sequence of chunked
	element candidates (the `$elem` token is actually an opaque cluster of bit
	expressions), followed by literal `0` tokens. Tokens provided by the caller
	are already opaque; only the zeros created in the previous arm are visible.

	As such, these enter only when the caller-provided bit tokens are exhausted.

	Once entered, these matchers convert each tuple of bit expressions into the
	requested storage type, and collect them into an array. This array is the
	return value of the originally-called macro.
	*/
	($ord:tt, $typ:ty as $uint:ident as usize, [$( ( $($elem:tt),* ) )*]; $(0,)*) => {
		//  `usize` must be constructed as a fixed-width integer, converted into
		//  `usize`, and *then* converted into the final storage type.
		[$(<$typ as From<usize>>::from(
			$crate::__make_elem!($ord, $uint as $uint; $($elem),*) as usize
		)),*]
	};
	($ord:tt, $typ:ty as $uint:ident, [$( ( $($elem:tt),* ) )*]; $(0,)*) => {
		[$(
			$crate::__make_elem!($ord, $typ as $uint; $($elem),*)
		),*]
	};

	/* These matchers chunk a stream of bit expressions into storage elements.

	On each entry, one element’s worth of bit tokens are pulled from the front
	of the stream (possibly including the literal `0` tokens provided above) and
	appended to the accumulator array as a n-tuple of bit expressions. This
	process continues until no more caller-provided bitstream tokens remain, at
	which point recursion traps in the above matchers, terminating the chunking
	and proceeding to element construction.
	*/
	(
		$ord:tt, $typ:tt as u8, [$( $elem:tt )*];
		$a0:tt, $b0:tt, $c0:tt, $d0:tt, $e0:tt, $f0:tt, $g0:tt, $h0:tt,
		$($t:tt)*
	) => {
		$crate::__encode_bits!(
			$ord, $typ as u8, [$($elem)* (
				$a0, $b0, $c0, $d0, $e0, $f0, $g0, $h0
			)]; $($t)*
		)
	};

	(
		$ord:tt, $typ:tt as u16, [$( $elem:tt )*];
		$a0:tt, $b0:tt, $c0:tt, $d0:tt, $e0:tt, $f0:tt, $g0:tt, $h0:tt,
		$a1:tt, $b1:tt, $c1:tt, $d1:tt, $e1:tt, $f1:tt, $g1:tt, $h1:tt,
		$($t:tt)*
	) => {
		$crate::__encode_bits!(
			$ord, $typ as u16, [$($elem)* (
				$a0, $b0, $c0, $d0, $e0, $f0, $g0, $h0,
				$a1, $b1, $c1, $d1, $e1, $f1, $g1, $h1
			)]; $($t)*
		)
	};

	(
		$ord:tt, $typ:tt as u32 $(as $usz:ident)?, [$( $elem:tt )*];
		$a0:tt, $b0:tt, $c0:tt, $d0:tt, $e0:tt, $f0:tt, $g0:tt, $h0:tt,
		$a1:tt, $b1:tt, $c1:tt, $d1:tt, $e1:tt, $f1:tt, $g1:tt, $h1:tt,
		$a2:tt, $b2:tt, $c2:tt, $d2:tt, $e2:tt, $f2:tt, $g2:tt, $h2:tt,
		$a3:tt, $b3:tt, $c3:tt, $d3:tt, $e3:tt, $f3:tt, $g3:tt, $h3:tt,
		$($t:tt)*
	) => {
		$crate::__encode_bits!(
			$ord, $typ as u32 $(as $usz)?, [$($elem)* (
				$a0, $b0, $c0, $d0, $e0, $f0, $g0, $h0,
				$a1, $b1, $c1, $d1, $e1, $f1, $g1, $h1,
				$a2, $b2, $c2, $d2, $e2, $f2, $g2, $h2,
				$a3, $b3, $c3, $d3, $e3, $f3, $g3, $h3
			)]; $($t)*
		)
	};

	(
		$ord:tt, $typ:tt as u64 $(as $usz:ident)?, [$( $elem:tt )*];
		$a0:tt, $b0:tt, $c0:tt, $d0:tt, $e0:tt, $f0:tt, $g0:tt, $h0:tt,
		$a1:tt, $b1:tt, $c1:tt, $d1:tt, $e1:tt, $f1:tt, $g1:tt, $h1:tt,
		$a2:tt, $b2:tt, $c2:tt, $d2:tt, $e2:tt, $f2:tt, $g2:tt, $h2:tt,
		$a3:tt, $b3:tt, $c3:tt, $d3:tt, $e3:tt, $f3:tt, $g3:tt, $h3:tt,
		$a4:tt, $b4:tt, $c4:tt, $d4:tt, $e4:tt, $f4:tt, $g4:tt, $h4:tt,
		$a5:tt, $b5:tt, $c5:tt, $d5:tt, $e5:tt, $f5:tt, $g5:tt, $h5:tt,
		$a6:tt, $b6:tt, $c6:tt, $d6:tt, $e6:tt, $f6:tt, $g6:tt, $h6:tt,
		$a7:tt, $b7:tt, $c7:tt, $d7:tt, $e7:tt, $f7:tt, $g7:tt, $h7:tt,
		$($t:tt)*
	) => {
		$crate::__encode_bits!(
			$ord, $typ as u64 $(as $usz)?, [$($elem)* (
				$a0, $b0, $c0, $d0, $e0, $f0, $g0, $h0,
				$a1, $b1, $c1, $d1, $e1, $f1, $g1, $h1,
				$a2, $b2, $c2, $d2, $e2, $f2, $g2, $h2,
				$a3, $b3, $c3, $d3, $e3, $f3, $g3, $h3,
				$a4, $b4, $c4, $d4, $e4, $f4, $g4, $h4,
				$a5, $b5, $c5, $d5, $e5, $f5, $g5, $h5,
				$a6, $b6, $c6, $d6, $e6, $f6, $g6, $h6,
				$a7, $b7, $c7, $d7, $e7, $f7, $g7, $h7
			)]; $($t)*
		)
	};
}

/// Counts the number of repetitions inside a `$()*` sequence.
#[doc(hidden)]
#[macro_export]
macro_rules! __count {
	(@ $val:expr) => { 1 };
	($($val:expr),*) => {{
		/* Clippy warns that `.. EXPR + 1`, for any value of `EXPR`, should be
		replaced with `..= EXPR`. This means that `.. $crate::__count!` raises
		the lint, causing `bits![(val,)…]` to have an unfixable lint warning.
		By binding to a `const`, then returning the `const`, this syntax
		construction is avoided as macros only expand to
		`.. { const LEN = …; LEN }` rather than `.. 0 (+ 1)…`.
		*/
		const LEN: usize = 0usize $(+ $crate::__count!(@ $val))*;
		LEN
	}};
}

/// Counts the number of elements needed to store a number of bits.
#[doc(hidden)]
#[macro_export]
macro_rules! __count_elts {
	($t:ty; $($val:expr),*) => {{
		$crate::mem::elts::<$t>($crate::__count!($($val),*))
	}};
}

/** Constructs a `T: BitStore` element from a byte-chunked sequence of bits.

# Arguments

- one of `Lsb0`, `Msb0`, `LocalBits`, or some path to a `BitOrder` implementor:
  the ordering parameter to use. Token matching against the three named
  orderings allows immediate work; unknown tokens invoke their trait
  implementation.
- `$typ` as `$uint`: Any `BitStore` implementor and its `::Mem` type.
- A sequence of any number of `(`, eight expressions, then `)`. These cluster
  bits into bytes, bytes into `$uint`, and then `$uint` into `$typ`.

# Returns

Exactly one `$typ`, whose bit-pattern is set to the provided sequence according
to the provided ordering.
**/
#[doc(hidden)]
#[macro_export]
macro_rules! __make_elem {
	//  Token-matching ordering names can use specialized work.
	(Lsb0, $typ:ty as $uint:ident; $(
		$a:expr, $b:expr, $c:expr, $d:expr,
		$e:expr, $f:expr, $g:expr, $h:expr
	),*) => {
		<$typ as From<$uint>>::from($crate::__ty_from_bytes!(
			Lsb0, $uint, [$($crate::macros::internal::u8_from_le_bits(
				$a != 0, $b != 0, $c != 0, $d != 0,
				$e != 0, $f != 0, $g != 0, $h != 0,
			)),*]
		))
	};
	(Msb0, $typ:ty as $uint:ident; $(
		$a:expr, $b:expr, $c:expr, $d:expr,
		$e:expr, $f:expr, $g:expr, $h:expr
	),*) => {
		<$typ as From<$uint>>::from($crate::__ty_from_bytes!(
			Msb0, $uint, [$($crate::macros::internal::u8_from_be_bits(
				$a != 0, $b != 0, $c != 0, $d != 0,
				$e != 0, $f != 0, $g != 0, $h != 0,
			)),*]
		))
	};
	(LocalBits, $typ:ty as $uint:ident; $(
		$a:expr, $b:expr, $c:expr, $d:expr,
		$e:expr, $f:expr, $g:expr, $h:expr
	),*) => {
		<$typ as From<$uint>>::from($crate::__ty_from_bytes!(
			LocalBits, $uint, [$($crate::macros::internal::u8_from_ne_bits(
				$a != 0, $b != 0, $c != 0, $d != 0,
				$e != 0, $f != 0, $g != 0, $h != 0,
			)),*]
		))
	};
	//  Otherwise, invoke `BitOrder` for each bit and accumulate.
	($ord:tt, $typ:ty as $uint:ident; $($bit:expr),* $(,)?) => {{
		let mut tmp: $uint = 0;
		let _bits = $crate::slice::BitSlice::<$ord, $uint>::from_element_mut(
			&mut tmp
		);
		let mut _idx = 0;
		$( _bits.set(_idx, $bit != 0); _idx += 1; )*
		<$typ as From<$uint>>::from(tmp)
	}};
}

/// Extend a single bit to fill an element.
#[doc(hidden)]
#[macro_export]
macro_rules! __extend_bool {
	($val:expr, $typ:tt) => {
		$typ::from([
			<<$typ as BitStore>::Mem as $crate::macros::internal::funty::IsInteger>::ZERO,
			<<$typ as BitStore>::Mem as $crate::mem::BitRegister>::ALL,
		][($val != 0) as usize])
	};
}

/// Constructs a fundamental integer from a list of bytes.
#[doc(hidden)]
#[macro_export]
macro_rules! __ty_from_bytes {
	(Msb0, u8, [$($byte:expr),*]) => {
		u8::from_be_bytes([$($byte),*])
	};
	(Lsb0, u8, [$($byte:expr),*]) => {
		u8::from_le_bytes([$($byte),*])
	};
	(LocalBits, u8, [$($byte:expr),*]) => {
		u8::from_ne_bytes([$($byte),*])
	};
	(Msb0, u16, [$($byte:expr),*]) => {
		u16::from_be_bytes([$($byte),*])
	};
	(Lsb0, u16, [$($byte:expr),*]) => {
		u16::from_le_bytes([$($byte),*])
	};
	(LocalBits, u16, [$($byte:expr),*]) => {
		u16::from_ne_bytes([$($byte),*])
	};
	(Msb0, u32, [$($byte:expr),*]) => {
		u32::from_be_bytes([$($byte),*])
	};
	(Lsb0, u32, [$($byte:expr),*]) => {
		u32::from_le_bytes([$($byte),*])
	};
	(LocalBits, u32, [$($byte:expr),*]) => {
		u32::from_ne_bytes([$($byte),*])
	};
	(Msb0, u64, [$($byte:expr),*]) => {
		u64::from_be_bytes([$($byte),*])
	};
	(Lsb0, u64, [$($byte:expr),*]) => {
		u64::from_le_bytes([$($byte),*])
	};
	(LocalBits, u64, [$($byte:expr),*]) => {
		u64::from_ne_bytes([$($byte),*])
	};
	(Msb0, usize, [$($byte:expr),*]) => {
		usize::from_be_bytes([$($byte),*])
	};
	(Lsb0, usize, [$($byte:expr),*]) => {
		usize::from_le_bytes([$($byte),*])
	};
	(LocalBits, usize, [$($byte:expr),*]) => {
		usize::from_ne_bytes([$($byte),*])
	};
}

/// Construct a `u8` from bits applied in Lsb0-order.
#[allow(clippy::many_single_char_names)]
#[allow(clippy::too_many_arguments)]
pub const fn u8_from_le_bits(
	a: bool,
	b: bool,
	c: bool,
	d: bool,
	e: bool,
	f: bool,
	g: bool,
	h: bool,
) -> u8 {
	(a as u8)
		| ((b as u8) << 1)
		| ((c as u8) << 2)
		| ((d as u8) << 3)
		| ((e as u8) << 4)
		| ((f as u8) << 5)
		| ((g as u8) << 6)
		| ((h as u8) << 7)
}

/// Construct a `u8` from bits applied in Msb0-order.
#[allow(clippy::many_single_char_names)]
#[allow(clippy::too_many_arguments)]
pub const fn u8_from_be_bits(
	a: bool,
	b: bool,
	c: bool,
	d: bool,
	e: bool,
	f: bool,
	g: bool,
	h: bool,
) -> u8 {
	(h as u8)
		| ((g as u8) << 1)
		| ((f as u8) << 2)
		| ((e as u8) << 3)
		| ((d as u8) << 4)
		| ((c as u8) << 5)
		| ((b as u8) << 6)
		| ((a as u8) << 7)
}

#[doc(hidden)]
#[cfg(target_endian = "little")]
pub use self::u8_from_le_bits as u8_from_ne_bits;

#[doc(hidden)]
#[cfg(target_endian = "big")]
pub use self::u8_from_be_bits as u8_from_ne_bits;

#[doc(hidden)]
#[cfg(not(tarpaulin_include))]
#[deprecated = "Ordering-only macro constructors are deprecated. Specify a \
                storage type as well, or remove the ordering and use the \
                default."]
pub const fn __deprecated_order_no_store() {
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn byte_assembly() {
		assert_eq!(
			u8_from_le_bits(false, false, true, true, false, true, false, true),
			0b1010_1100
		);

		assert_eq!(
			u8_from_be_bits(false, false, true, true, false, true, false, true),
			0b0011_0101
		);
	}
}
