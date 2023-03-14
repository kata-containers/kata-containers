/*! [`serde`]-powered de/serialization.

This module implements the Serde traits for the [`bitvec`] types.

As [`BitArray`] does not use dynamic indexing for its starting index or its
length, it implements [`Deserialize`] and [`Serialize`] by forwarding to the
interior buffer, and adds no additional information. This also renders it
incapable of deserializing a serialized [`BitSlice`]; it can only deserialize
bare value sequences.

[`BitSlice`] is able to implement [`Serialize`], but [`serde`] does not provide
a behavior to deserialize data into a buffer provided by the calling context, so
it cannot deserialize into any of the owning structures.

[`BitBox`] and [`BitVec`] implement [`Serialize`] through [`BitSlice`], and can
deserialize the [`BitSlice`] format into themselves.

If you require de/serialization compatibility between [`BitArray`] and the other
structures, please file an issue.

The exact implementation of the `serde` interfaces is considered an internal
detail and is not guaranteed; however, as it is technically public ABI, it will
only be modified in a major release (`0.X.n` to `0.Y.0` or `X.m.n` to `Y.0.0`).

[`BitArray`]: crate::array::BitArray
[`BitBox`]: crate::boxed::BitBox
[`BitSlice`]: crate::slice::BitSlice
[`BitVec`]: crate::vec::BitVec
[`Deserialize`]: serde::de::Deserialize
[`Serialize`]: serde::ser::Serialize
[`bitvec`]: crate
[`serde`]: serde
!*/

#![cfg(feature = "serde")]

use crate::{
	array::BitArray,
	domain::Domain,
	mem::BitMemory,
	order::BitOrder,
	ptr::{
		AddressError,
		BitPtr,
		BitPtrError,
		BitSpanError,
	},
	slice::BitSlice,
	store::BitStore,
	view::BitView,
};

use core::{
	cmp,
	fmt::{
		self,
		Formatter,
	},
	marker::PhantomData,
	mem::ManuallyDrop,
};

use serde::{
	de::{
		self,
		Deserialize,
		Deserializer,
		MapAccess,
		SeqAccess,
		Unexpected,
		Visitor,
	},
	ser::{
		Serialize,
		SerializeSeq,
		SerializeStruct,
		Serializer,
	},
};

use tap::pipe::Pipe;

#[cfg(feature = "alloc")]
use crate::{
	boxed::BitBox,
	vec::BitVec,
};

impl<O, T> Serialize for BitSlice<O, T>
where
	O: BitOrder,
	T: BitStore,
	T::Mem: Serialize,
{
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where S: Serializer {
		let head = self.as_bitspan().head();
		let mut state = serializer.serialize_struct("BitSeq", 3)?;

		state.serialize_field("head", &head.value())?;
		state.serialize_field("bits", &(self.len() as u64))?;
		state.serialize_field("data", &self.domain())?;

		state.end()
	}
}

impl<T> Serialize for Domain<'_, T>
where
	T: BitStore,
	T::Mem: Serialize,
{
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where S: Serializer {
		let mut state = serializer.serialize_seq(Some(self.len()))?;
		for elem in *self {
			state.serialize_element(&elem)?;
		}
		state.end()
	}
}

/// Serializes the interior storage type directly, rather than routing through a
/// dynamic sequence serializer.
#[cfg(not(tarpaulin_include))]
impl<O, V> Serialize for BitArray<O, V>
where
	O: BitOrder,
	V: BitView + Serialize,
{
	#[inline]
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where S: Serializer {
		unsafe { core::ptr::read(self) }
			.value()
			.serialize(serializer)
	}
}

#[cfg(feature = "alloc")]
impl<O, T> Serialize for BitBox<O, T>
where
	O: BitOrder,
	T: BitStore,
	T::Mem: Serialize,
{
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where S: Serializer {
		self.as_bitslice().serialize(serializer)
	}
}

#[cfg(feature = "alloc")]
impl<O, T> Serialize for BitVec<O, T>
where
	O: BitOrder,
	T: BitStore,
	T::Mem: Serialize,
{
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where S: Serializer {
		self.as_bitslice().serialize(serializer)
	}
}

impl<'de, O, V> Deserialize<'de> for BitArray<O, V>
where
	O: BitOrder,
	V: BitView + Deserialize<'de>,
{
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where D: Deserializer<'de> {
		deserializer
			.pipe(<V as Deserialize<'de>>::deserialize)
			.map(Self::new)
	}
}

/** Aid for deserializing a protocol into a [`BitVec`].

[`BitVec`]: crate::vec::BitVec
**/
#[cfg(feature = "alloc")]
#[derive(Clone, Copy, Debug, Default)]
struct BitVecVisitor<'de, O, T>
where
	O: BitOrder,
	T: BitStore + Deserialize<'de>,
{
	_lt: PhantomData<&'de ()>,
	_bv: PhantomData<BitVec<O, T>>,
}

#[cfg(feature = "alloc")]
impl<'de, O, T> BitVecVisitor<'de, O, T>
where
	O: BitOrder,
	T: BitStore + Deserialize<'de>,
{
	const THIS: Self = Self {
		_lt: PhantomData,
		_bv: PhantomData,
	};

	/// Constructs a [`BitVec`] from deserialized components.
	///
	/// # Parameters
	///
	/// - `&self`: A visitor, only needed for access to an error message.
	/// - `head`: The deserialized head-bit index.
	/// - `bits`: The deserialized length counter.
	/// - `data`: A vector of memory containing the bitslice. Its dest
	///
	/// # Returns
	///
	/// The result of assembling the deserialized components into a [`BitVec`].
	/// This can fail if the `head` is invalid, or if the deserialized data
	/// cannot be encoded into a `BitSpan`.
	///
	/// [`BitVec`]: crate::vec::BitVec
	fn assemble<E>(
		&self,
		head: u8,
		bits: usize,
		data: Vec<T>,
	) -> Result<<Self as Visitor<'de>>::Value, E>
	where
		E: de::Error,
	{
		//  Disable the destructor on the deserialized buffer
		let mut data = ManuallyDrop::new(data);
		//  Ensure that the `bits` counter is not lying about the data size.
		let bits = cmp::min(
			bits,
			data.len()
				.saturating_mul(T::Mem::BITS as usize)
				.saturating_sub(head as usize),
		);
		//  Assemble a pointer to the start bit,
		BitPtr::try_new(data.as_mut_slice().as_mut_ptr(), head)
			.map_err(Into::into)
			//  Extend it into a region descriptor,
			.and_then(|bp| bp.span(bits))
			//  Capture the errors that can arise from the deser data,
			.map_err(|err| match err {
				BitSpanError::InvalidBitptr(BitPtrError::InvalidIndex(err)) => {
					de::Error::invalid_value(
						Unexpected::Unsigned(err.value() as u64),
						&"a head-bit index less than the deserialized element \
						  type’s bit width",
					)
				},
				BitSpanError::TooLong(len) => de::Error::invalid_value(
					Unexpected::Unsigned(len as u64),
					&"a bit length that can be encoded into a `*BitSlice` \
					  pointer",
				),
				BitSpanError::TooHigh(_) => unreachable!(
					"The allocator will not produce a vector too high in the \
					 memory space"
				),
				BitSpanError::InvalidBitptr(BitPtrError::InvalidAddress(
					AddressError::Null,
				)) => {
					unreachable!("The allocator will not produce a null pointer")
				},
				BitSpanError::InvalidBitptr(BitPtrError::InvalidAddress(
					AddressError::Misaligned(_),
				)) => {
					unreachable!(
						"The allocator will not produce a misaligned buffer"
					)
				},
			})
			//  And assemble a bit-vector over the allocated span.
			.map(|span| unsafe { BitVec::from_fields(span, data.capacity()) })
	}
}

#[cfg(feature = "alloc")]
impl<'de, O, T> Visitor<'de> for BitVecVisitor<'de, O, T>
where
	O: BitOrder,
	T: BitStore + Deserialize<'de>,
{
	type Value = BitVec<O, T>;

	fn expecting(&self, fmt: &mut Formatter) -> fmt::Result {
		fmt.write_str("a BitSeq data series")
	}

	/// Visit a sequence of anonymous data elements. These must be in the order
	/// `u8` (head-bit index), `u64` (length counter), `[T]` (data contents).
	fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
	where V: SeqAccess<'de> {
		let head = seq
			.next_element::<u8>()?
			.ok_or_else(|| de::Error::invalid_length(0, &self))?;
		let bits = seq
			.next_element::<u64>()?
			.ok_or_else(|| de::Error::invalid_length(1, &self))?;
		let data = seq
			.next_element::<Vec<T>>()?
			.ok_or_else(|| de::Error::invalid_length(2, &self))?;

		self.assemble(head, bits as usize, data)
	}

	/// Visit a map of named data elements. These may be in any order, and must
	/// be the pairs `head: u8`, `bits: u64`, and `data: [T]`.
	fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
	where V: MapAccess<'de> {
		let mut head: Option<u8> = None;
		let mut bits: Option<u64> = None;
		let mut data: Option<Vec<T>> = None;

		while let Some(key) = map.next_key()? {
			match key {
				"head" => {
					if head.replace(map.next_value()?).is_some() {
						return Err(de::Error::duplicate_field("head"));
					}
				},
				"bits" => {
					if bits.replace(map.next_value()?).is_some() {
						return Err(de::Error::duplicate_field("bits"));
					}
				},
				"data" => {
					if data.replace(map.next_value()?).is_some() {
						return Err(de::Error::duplicate_field("data"));
					}
				},
				f => {
					/* Once a key is pulled from the map, a value **must** also
					be pulled, otherwise `serde` will fail with its own error
					rather than this one.
					*/
					let _ = map.next_value::<()>();
					return Err(de::Error::unknown_field(f, &[
						"head", "bits", "data",
					]));
				},
			}
		}
		let head = head.ok_or_else(|| de::Error::missing_field("head"))?;
		let bits = bits.ok_or_else(|| de::Error::missing_field("bits"))?;
		let data = data.ok_or_else(|| de::Error::missing_field("data"))?;

		self.assemble(head, bits as usize, data)
	}
}

#[cfg(feature = "alloc")]
impl<'de, O, T> Deserialize<'de> for BitBox<O, T>
where
	O: BitOrder,
	T: BitStore + Deserialize<'de>,
{
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where D: Deserializer<'de> {
		deserializer
			.pipe(<BitVec<O, T> as Deserialize<'de>>::deserialize)
			.map(BitVec::into_boxed_bitslice)
	}
}

#[cfg(feature = "alloc")]
impl<'de, O, T> Deserialize<'de> for BitVec<O, T>
where
	O: BitOrder,
	T: BitStore + Deserialize<'de>,
{
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where D: Deserializer<'de> {
		deserializer.deserialize_struct(
			"BitSeq",
			&["head", "bits", "data"],
			BitVecVisitor::THIS,
		)
	}
}

#[cfg(test)]
mod tests {
	use crate::prelude::*;

	use serde::Deserialize;

	use serde_test::{
		assert_ser_tokens,
		Token,
	};

	#[cfg(feature = "alloc")]
	use serde_test::{
		assert_de_tokens,
		assert_de_tokens_error,
	};

	macro_rules! bvtok {
		( s $elts:expr, $head:expr, $bits:expr, $ty:ident $( , $data:expr )* ) => {
			&[
				Token::Struct { name: "BitSeq", len: 3, },
				Token::Str("head"), Token::U8( $head ),
				Token::Str("bits"), Token::U64( $bits ),
				Token::Str("data"), Token::Seq { len: Some( $elts ) },
				$( Token:: $ty ( $data ), )*
				Token::SeqEnd,
				Token::StructEnd,
			]
		};
		( d $elts:expr, $head:expr, $bits:expr, $ty:ident $( , $data:expr )* ) => {
			&[
				Token::Struct { name: "BitSeq", len: 3, },
				Token::BorrowedStr("head"), Token::U8( $head ),
				Token::BorrowedStr("bits"), Token::U64( $bits ),
				Token::BorrowedStr("data"), Token::Seq { len: Some( $elts ) },
				$( Token:: $ty ( $data ), )*
				Token::SeqEnd,
				Token::StructEnd,
			]
		};
	}

	#[test]
	fn empty() {
		let slice = BitSlice::<Msb0, u8>::empty();

		assert_ser_tokens(&slice, bvtok![s 0, 0, 0, U8]);

		#[cfg(feature = "alloc")]
		assert_de_tokens(&bitvec![], bvtok![ d 0, 0, 0, U8 ]);
	}

	#[test]
	fn small() {
		let bits = 0b1111_1000u8.view_bits::<Msb0>();
		let bits = &bits[1 .. 5];
		assert_ser_tokens(&bits, bvtok![s 1, 1, 4, U8, 0b1111_1000]);

		let bits = 0b00001111_11111111u16.view_bits::<Lsb0>();
		let bits = &bits[.. 12];
		assert_ser_tokens(&bits, bvtok![s 1, 0, 12, U16, 0b00001111_11111111]);

		let bits = 0b11_11111111u32.view_bits::<LocalBits>();
		let bits = &bits[.. 10];
		assert_ser_tokens(&bits, bvtok![s 1, 0, 10, U32, 0x00_00_03_FF]);
	}

	#[test]
	fn wide() {
		let src: &[u8] = &[0, !0];
		let bs = src.view_bits::<LocalBits>();
		assert_ser_tokens(&(&bs[1 .. 15]), bvtok![s 2, 1, 14, U8, 0, !0]);
	}

	#[test]
	#[cfg(feature = "alloc")]
	fn deser() {
		let bv = bitvec![Msb0, u8; 0, 1, 1, 0, 1, 0];
		let bb = bv.clone().into_boxed_bitslice();
		assert_de_tokens(&bv, bvtok![d 1, 0, 6, U8, 0b0110_1000]);
		//  test that the bits outside the bits domain don't matter in deser
		assert_de_tokens(&bv, bvtok![d 1, 0, 6, U8, 0b0110_1001]);
		assert_de_tokens(&bb, bvtok![d 1, 0, 6, U8, 0b0110_1010]);
		assert_de_tokens(&bb, bvtok![d 1, 0, 6, U8, 0b0110_1011]);
	}

	#[test]
	#[cfg(feature = "alloc")]
	fn ser() {
		let bv = bitvec![Msb0, u8; 0, 1, 1, 0, 1, 0];
		let bb = bv.clone().into_boxed_bitslice();

		assert_ser_tokens(&bv, bvtok![s 1, 0, 6, U8, 0b0110_1000]);
		assert_ser_tokens(&bb, bvtok![s 1, 0, 6, U8, 0b0110_1000]);
	}

	#[test]
	#[cfg(feature = "alloc")]
	fn error_paths() {
		assert_de_tokens_error::<BitVec<Msb0, u8>>(
			bvtok!(d 0, 9, 0, U8),
			"invalid value: integer `9`, expected a head-bit index less than \
			 the deserialized element type’s bit width",
		);

		for field in &["head", "bits"] {
			assert_de_tokens_error::<BitVec<Msb0, u8>>(
				&[
					Token::Struct {
						name: "BitSeq",
						len: 2,
					},
					Token::BorrowedStr(field),
					Token::U8(0),
					Token::BorrowedStr(field),
					Token::U8(1),
					Token::StructEnd,
				],
				&format!("duplicate field `{}`", field),
			);
		}

		assert_de_tokens_error::<BitVec<Msb0, u8>>(
			&[
				Token::Struct {
					name: "BitSeq",
					len: 2,
				},
				Token::BorrowedStr("data"),
				Token::Seq { len: Some(1) },
				Token::U8(2),
				Token::SeqEnd,
				Token::BorrowedStr("data"),
				Token::Seq { len: Some(1) },
				Token::U8(3),
				Token::SeqEnd,
				Token::StructEnd,
			],
			"duplicate field `data`",
		);

		assert_de_tokens_error::<BitVec<Msb0, u8>>(
			&[
				Token::Struct {
					name: "BitSeq",
					len: 1,
				},
				Token::BorrowedStr("garbage"),
				Token::BorrowedStr("field"),
				Token::StructEnd,
			],
			"unknown field `garbage`, expected one of `head`, `bits`, `data`",
		);
	}

	#[test]
	fn deser_seq() {
		let bv = bitvec![Msb0, u8; 0, 1];
		assert_de_tokens::<BitVec<Msb0, u8>>(&bv, &[
			Token::Seq { len: Some(3) },
			Token::U8(0),
			Token::U64(2),
			Token::Seq { len: Some(1) },
			Token::U8(66),
			Token::SeqEnd,
			Token::SeqEnd,
		]);

		assert_de_tokens_error::<BitVec<Msb0, u8>>(
			&[Token::Seq { len: Some(0) }, Token::SeqEnd],
			"invalid length 0, expected a BitSeq data series",
		);

		assert_de_tokens_error::<BitVec<Msb0, u8>>(
			&[Token::Seq { len: Some(1) }, Token::U8(0), Token::SeqEnd],
			"invalid length 1, expected a BitSeq data series",
		);

		assert_de_tokens_error::<BitVec<Msb0, u8>>(
			&[
				Token::Seq { len: Some(2) },
				Token::U8(0),
				Token::U64(2),
				Token::SeqEnd,
			],
			"invalid length 2, expected a BitSeq data series",
		);
	}

	#[test]
	fn trait_impls() {
		const _: fn() = || {
			fn assert_impl_all<'de, T: Deserialize<'de>>() {
			}
			assert_impl_all::<BitArray<LocalBits, [usize; 32]>>();
		};
	}
}
