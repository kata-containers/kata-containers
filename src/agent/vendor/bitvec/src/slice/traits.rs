//! Non-operator trait implementations.

use crate::{
	domain::Domain,
	mem::BitMemory,
	order::{
		BitOrder,
		Lsb0,
		Msb0,
	},
	slice::BitSlice,
	store::BitStore,
	view::BitView,
};

use core::{
	any::TypeId,
	cmp,
	convert::TryFrom,
	fmt::{
		self,
		Binary,
		Debug,
		Display,
		Formatter,
		LowerHex,
		Octal,
		UpperHex,
	},
	hash::{
		Hash,
		Hasher,
	},
	str,
};

use tap::pipe::Pipe;

#[cfg(feature = "alloc")]
use crate::vec::BitVec;

#[cfg(feature = "alloc")]
use alloc::borrow::ToOwned;

impl<O, T> Eq for BitSlice<O, T>
where
	O: BitOrder,
	T: BitStore,
{
}

impl<O, T> Ord for BitSlice<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	fn cmp(&self, rhs: &Self) -> cmp::Ordering {
		self.partial_cmp(rhs)
			.expect("BitSlice has a total ordering")
	}
}

/** Tests if two `BitSlice`s are semantically — not bitwise — equal.

It is valid to compare slices of different ordering or memory types.

The equality condition requires that they have the same length and that at each
index, the two slices have the same bit value.
**/
impl<O1, O2, T1, T2> PartialEq<BitSlice<O2, T2>> for BitSlice<O1, T1>
where
	O1: BitOrder,
	O2: BitOrder,
	T1: BitStore,
	T2: BitStore,
{
	fn eq(&self, rhs: &BitSlice<O2, T2>) -> bool {
		let fallback = || {
			if self.len() != rhs.len() {
				return false;
			}
			self.iter()
				.by_val()
				.zip(rhs.iter().by_val())
				.all(|(l, r)| l == r)
		};

		if TypeId::of::<O1>() == TypeId::of::<O2>()
			&& TypeId::of::<T1>() == TypeId::of::<T2>()
		{
			if TypeId::of::<O1>() == TypeId::of::<Lsb0>() {
				let this: &BitSlice<Lsb0, T1> =
					unsafe { &*(self as *const _ as *const _) };
				let that: &BitSlice<Lsb0, T1> =
					unsafe { &*(rhs as *const _ as *const _) };
				this.sp_eq(that)
			}
			else if TypeId::of::<O1>() == TypeId::of::<Msb0>() {
				let this: &BitSlice<Msb0, T1> =
					unsafe { &*(self as *const _ as *const _) };
				let that: &BitSlice<Msb0, T1> =
					unsafe { &*(rhs as *const _ as *const _) };
				this.sp_eq(that)
			}
			else {
				fallback()
			}
		}
		else {
			fallback()
		}
	}
}

//  ref-to-val equality

impl<O1, O2, T1, T2> PartialEq<BitSlice<O2, T2>> for &BitSlice<O1, T1>
where
	O1: BitOrder,
	O2: BitOrder,
	T1: BitStore,
	T2: BitStore,
{
	fn eq(&self, rhs: &BitSlice<O2, T2>) -> bool {
		**self == rhs
	}
}

impl<O1, O2, T1, T2> PartialEq<BitSlice<O2, T2>> for &mut BitSlice<O1, T1>
where
	O1: BitOrder,
	O2: BitOrder,
	T1: BitStore,
	T2: BitStore,
{
	fn eq(&self, rhs: &BitSlice<O2, T2>) -> bool {
		**self == rhs
	}
}

//  val-to-ref equality

impl<O1, O2, T1, T2> PartialEq<&BitSlice<O2, T2>> for BitSlice<O1, T1>
where
	O1: BitOrder,
	O2: BitOrder,
	T1: BitStore,
	T2: BitStore,
{
	fn eq(&self, rhs: &&BitSlice<O2, T2>) -> bool {
		*self == **rhs
	}
}

impl<O1, O2, T1, T2> PartialEq<&mut BitSlice<O2, T2>> for BitSlice<O1, T1>
where
	O1: BitOrder,
	O2: BitOrder,
	T1: BitStore,
	T2: BitStore,
{
	fn eq(&self, rhs: &&mut BitSlice<O2, T2>) -> bool {
		*self == **rhs
	}
}

/** Compares two `BitSlice`s by semantic — not bitwise — ordering.

The comparison sorts by testing at each index if one slice has a high bit where
the other has a low. At the first index where the slices differ, the slice with
the high bit is greater. If the slices are equal until at least one terminates,
then they are compared by length.
**/
impl<O1, O2, T1, T2> PartialOrd<BitSlice<O2, T2>> for BitSlice<O1, T1>
where
	O1: BitOrder,
	O2: BitOrder,
	T1: BitStore,
	T2: BitStore,
{
	fn partial_cmp(&self, rhs: &BitSlice<O2, T2>) -> Option<cmp::Ordering> {
		for (l, r) in self.iter().zip(rhs.iter()) {
			match (*l, *r) {
				(true, false) => return Some(cmp::Ordering::Greater),
				(false, true) => return Some(cmp::Ordering::Less),
				_ => continue,
			}
		}
		self.len().partial_cmp(&rhs.len())
	}
}

//  ref-to-val ordering

impl<O1, O2, T1, T2> PartialOrd<BitSlice<O2, T2>> for &BitSlice<O1, T1>
where
	O1: BitOrder,
	O2: BitOrder,
	T1: BitStore,
	T2: BitStore,
{
	fn partial_cmp(&self, rhs: &BitSlice<O2, T2>) -> Option<cmp::Ordering> {
		(*self).partial_cmp(rhs)
	}
}

impl<O1, O2, T1, T2> PartialOrd<BitSlice<O2, T2>> for &mut BitSlice<O1, T1>
where
	O1: BitOrder,
	O2: BitOrder,
	T1: BitStore,
	T2: BitStore,
{
	fn partial_cmp(&self, rhs: &BitSlice<O2, T2>) -> Option<cmp::Ordering> {
		(**self).partial_cmp(rhs)
	}
}

//  val-to-ref ordering

impl<O1, O2, T1, T2> PartialOrd<&BitSlice<O2, T2>> for BitSlice<O1, T1>
where
	O1: BitOrder,
	O2: BitOrder,
	T1: BitStore,
	T2: BitStore,
{
	fn partial_cmp(&self, rhs: &&BitSlice<O2, T2>) -> Option<cmp::Ordering> {
		(*self).partial_cmp(&**rhs)
	}
}

impl<O1, O2, T1, T2> PartialOrd<&mut BitSlice<O2, T2>> for BitSlice<O1, T1>
where
	O1: BitOrder,
	O2: BitOrder,
	T1: BitStore,
	T2: BitStore,
{
	fn partial_cmp(&self, rhs: &&mut BitSlice<O2, T2>) -> Option<cmp::Ordering> {
		(*self).partial_cmp(&**rhs)
	}
}

//  &mut-to-& ordering

impl<O1, O2, T1, T2> PartialOrd<&mut BitSlice<O2, T2>> for &BitSlice<O1, T1>
where
	O1: BitOrder,
	O2: BitOrder,
	T1: BitStore,
	T2: BitStore,
{
	fn partial_cmp(&self, rhs: &&mut BitSlice<O2, T2>) -> Option<cmp::Ordering> {
		(**self).partial_cmp(&**rhs)
	}
}

impl<O1, O2, T1, T2> PartialOrd<&BitSlice<O2, T2>> for &mut BitSlice<O1, T1>
where
	O1: BitOrder,
	O2: BitOrder,
	T1: BitStore,
	T2: BitStore,
{
	fn partial_cmp(&self, rhs: &&BitSlice<O2, T2>) -> Option<cmp::Ordering> {
		(**self).partial_cmp(&**rhs)
	}
}

impl<'a, O, T> TryFrom<&'a [T]> for &'a BitSlice<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	type Error = &'a [T];

	fn try_from(slice: &'a [T]) -> Result<Self, Self::Error> {
		BitSlice::from_slice(slice).map_err(|_| slice)
	}
}

impl<'a, O, T> TryFrom<&'a mut [T]> for &'a mut BitSlice<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	type Error = &'a mut [T];

	fn try_from(slice: &'a mut [T]) -> Result<Self, Self::Error> {
		let slice_ptr = slice as *mut [T];
		BitSlice::from_slice_mut(slice).map_err(|_| unsafe { &mut *slice_ptr })
	}
}

impl<O, T> Default for &BitSlice<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	fn default() -> Self {
		BitSlice::empty()
	}
}

impl<O, T> Default for &mut BitSlice<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	fn default() -> Self {
		BitSlice::empty_mut()
	}
}

impl<O, T> Debug for BitSlice<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		self.as_bitspan().render(fmt, "Slice", None)?;
		fmt.write_str(" ")?;
		Display::fmt(self, fmt)
	}
}

impl<O, T> Display for BitSlice<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
		Binary::fmt(self, fmt)
	}
}

/// Constructs numeric formatting implementations.
macro_rules! fmt {
	($trait:ident, $base:expr, $pfx:expr, $blksz:expr) => {
		/// Render the contents of a `BitSlice` in a numeric format.
		///
		/// These implementations render the bits of memory contained in a
		/// `BitSlice` as one of the three numeric bases that the Rust format
		/// system supports:
		///
		/// - `Binary` renders each bit individually as `0` or `1`,
		/// - `Octal` renders clusters of three bits as the numbers `0` through
		///   `7`,
		/// - and `UpperHex` and `LowerHex` render clusters of four bits as the
		///   numbers `0` through `9` and `A` through `F`.
		///
		/// The formatters produce a “word” for each element `T` of memory. The
		/// chunked formats (octal and hexadecimal) operate somewhat peculiarly:
		/// they show the semantic value of the memory, as interpreted by the
		/// ordering parameter’s implementation rather than the raw value of
		/// memory you might observe with a debugger. In order to ease the
		/// process of expanding numbers back into bits, each digit is grouped to
		/// the right edge of the memory element. So, for example, the byte
		/// `0xFF` would be rendered in as `0o377` rather than `0o773`.
		///
		/// Rendered words are chunked by memory elements, rather than by as
		/// clean as possible a number of digits, in order to aid visualization
		/// of the slice’s place in memory.
		impl<O, T> $trait for BitSlice<O, T>
		where
			O: BitOrder,
			T: BitStore,
		{
			fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
				/// Renders an accumulated text buffer as UTF-8.
				struct Seq<'a>(&'a [u8]);
				impl Debug for Seq<'_> {
					fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
						fmt.write_str(unsafe {
							str::from_utf8_unchecked(self.0)
						})
					}
				}

				//  If the alternate flag is set, include the radix prefix.
				let start = if fmt.alternate() { 0 } else { 2 };
				//  Create a list format accumulator.
				let mut dbg = fmt.debug_list();
				/* Create a static buffer sized for the maximum number of UTF-8
				bytes needed to render a `usize` in the selected radix.

				Rust does not yet grant access to trait constants for use in
				constant expressions within generics.
				*/
				const D: usize = <usize as BitMemory>::BITS as usize / $blksz;
				#[allow(clippy::modulo_one)]
				const M: usize = <usize as BitMemory>::BITS as usize % $blksz;
				const W: usize = D + (M != 0) as usize;
				let mut w: [u8; W + 2] = [b'0'; W + 2];
				//  Write the prefix symbol into the buffer.
				w[1] = $pfx;

				/* This closure does the main work of rendering a bit-slice as
				text. It will be called on each memory element of the slice
				being formatted.
				*/
				let mut writer = |bits: &BitSlice<O, T::Mem>| {
					//  Set the end index of the text accumulator.
					let mut end = 2;
					/* Taking `rchunks` clusters the bits to the right edge, so
					that any remainder is in the left-most (first-rendered)
					digit, in the same manner as English digit clusters in
					ordinary writing.

					Since `rchunks` takes from back to front, it must be
					reversed in order to traverse the slice from front to back.
					The enumeration provides the offset from the buffer start
					for writing the computed digit into the text accumulator.
					*/
					for chunk in bits.rchunks($blksz).rev() {
						/* Copy the bits of the slice into the temporary, in
						Msb0 order, at the LSedge of the temporary. This will
						translate the bit sequence into the binary digit that
						represents it.
						*/
						let mut val = 0u8;
						for bit in chunk {
							val <<= 1;
							val |= *bit as u8;
						}

						/* Translate the accumulator digit into the matching
						ASCII hexadecimal glyph, and write the glyph into the
						text accumulator.
						*/
						w[end] = match val {
							v @ 0 ..= 9 => b'0' + v,
							v @ 10 ..= 16 => $base + (v - 10),
							_ => unsafe { core::hint::unreachable_unchecked() },
						};
						end += 1;
					}

					//  View the text accumulator as UTF-8 and write it into the
					//  main formatter.
					dbg.entry(&Seq(&w[start .. end]));
				};

				/* Break the source `BitSlice` into its aliased sub-regions.
				This is necessary in order to load each element into local
				memory for formatting.
				*/
				match self.domain() {
					Domain::Enclave { head, elem, tail } => {
						//  Load a copy of `*elem` into the stack,
						let tmp = elem.load_value();
						//  View the whole element as bits, narrow it to the
						//  live span, and render.
						let bits = tmp.view_bits::<O>();
						unsafe {
							bits.get_unchecked(
								head.value() as usize .. tail.value() as usize,
							)
						}
						.pipe(writer);
					},
					//  Same process as above, but at different truncations.
					Domain::Region { head, body, tail } => {
						if let Some((head, elem)) = head {
							let tmp = elem.load_value();
							let bits = tmp.view_bits::<O>();
							unsafe {
								bits.get_unchecked(head.value() as usize ..)
							}
							.pipe(&mut writer);
						}
						for elem in body.iter().map(BitStore::load_value) {
							elem.view_bits::<O>().pipe(&mut writer);
						}
						if let Some((elem, tail)) = tail {
							let tmp = elem.load_value();
							let bits = tmp.view_bits::<O>();
							unsafe {
								bits.get_unchecked(.. tail.value() as usize)
							}
							.pipe(&mut writer);
						}
					},
				}
				dbg.finish()
			}
		}
	};
}

fmt!(Binary, b'0', b'b', 1);
fmt!(Octal, b'0', b'o', 3);
fmt!(LowerHex, b'a', b'x', 4);
fmt!(UpperHex, b'A', b'x', 4);

/// Writes the contents of the `BitSlice`, in semantic bit order, into a hasher.
#[cfg(not(tarpaulin_include))]
impl<O, T> Hash for BitSlice<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	fn hash<H>(&self, hasher: &mut H)
	where H: Hasher {
		for bit in self {
			hasher.write_u8(*bit as u8);
		}
	}
}

/** Conditionally mark `BitSlice` as `Send` based on its `T` type argument.

In order for `BitSlice` to be `Send` (that is, `&mut BitSlice` can be moved
across thread boundaries), it must be capable of writing to memory without
invalidating any other `&BitSlice` handles that alias the same memory address.

This is true when `T` is one of the fundamental integers, because no other
`&BitSlice` handle is able to observe mutations, or when `T` is a `BitSafe` type
that implements atomic read-modify-write instructions, because other `&BitSlice`
types will be protected from data races by the hardware.

When `T` is a non-atomic `BitSafe` type, `BitSlice` cannot be `Send`, because
one `&mut BitSlice` moved across a thread boundary may cause mutation that
another `&BitSlice` may observe, but the instructions used to access memory do
not guard against data races.

A `&mut BitSlice` over aliased memory addresses is equivalent to either a
`&Cell` or `&AtomicT`, depending on what the [`radium`] crate makes available
for the register width.

[`radium`]: radium::types
**/
unsafe impl<O, T> Send for BitSlice<O, T>
where
	O: BitOrder,
	T: BitStore + Sync,
{
}

/** Conditionally mark `BitSlice` as `Sync` based on its `T` type argument.

In order for `BitSlice` to be `Sync` (that is, `&BitSlice` can be copied across
thread boundaries), it must be capable of reading from memory without being
invalidated by any other `&mut BitSlice` handles that alias the same memory
address.

This is true when `T` is one of the fundamental integers, because no other
`&mut BitSlice` handle can exist to effect mutations, or when `T` is a `BitSafe`
type that implements atomic read-modify-write instructions, because it will
guard against other `&mut BitSlice` modifications in hardware.

When `T` is a non-atomic `BitSafe` type, `BitSlice` cannot be `Sync`, because
one `&BitSlice` moved across a thread boundary may read from memory that is
modified by the originally-owning thread, but the instructions used to access
memory do not guard against such data races.

A `&BitSlice` over aliased memory addresses is equivalent to either a `&Cell`
or `&AtomicT`, depending on what the [`radium`] crate makes available for the
register width.

[`radium`]: radium::types
**/
unsafe impl<O, T> Sync for BitSlice<O, T>
where
	O: BitOrder,
	T: BitStore + Sync,
{
}

#[cfg(feature = "alloc")]
impl<O, T> ToOwned for BitSlice<O, T>
where
	O: BitOrder,
	T: BitStore,
{
	type Owned = BitVec<O, T>;

	fn to_owned(&self) -> Self::Owned {
		BitVec::from_bitslice(self)
	}
}
