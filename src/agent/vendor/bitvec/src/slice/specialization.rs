/*! Specialization overrides.

This module contains override functions used when generic bit-by-bit iteration
can be accelerated for specific type parameters.

Call sites that wish to take advantage of specialization methods must first
inspect their type arguments to determine if specialization is even possible,
and transmute generic slices into slices with concrete type arguments applied.
!*/

use crate::{
	devel as dvl,
	domain::Domain,
	field::BitField,
	mem::BitMemory,
	order::{
		BitOrder,
		Lsb0,
		Msb0,
	},
	slice::BitSlice,
	store::BitStore,
};

use core::ops::RangeBounds;

use funty::IsInteger;

/** Order-specialized function implementations.

These functions use [`BitField`] to provide batched load/store behavior.
Where loads or stores cross a `T` element boundary, they use the `_le`
behavior to ensure that bits stay in the correct order relative to each
other, even as they may change position within an element.

[`BitField`]: crate::field::BitField
**/
impl<T> BitSlice<Lsb0, T>
where T: BitStore
{
	/// Accelerates copies between disjoint slices with batch loads.
	pub(crate) fn sp_copy_from_bitslice(&mut self, src: &Self) {
		assert_eq!(
			self.len(),
			src.len(),
			"Copying between slices requires equal lengths"
		);

		let chunk_size = <usize as BitMemory>::BITS as usize;
		for (to, from) in unsafe { self.chunks_mut(chunk_size).remove_alias() }
			.zip(src.chunks(chunk_size))
		{
			to.store_le::<usize>(from.load_le::<usize>())
		}
	}

	/// Accelerates possibly-overlapping copies within a single slice with batch
	/// loads.
	pub(crate) unsafe fn sp_copy_within_unchecked<R>(
		&mut self,
		src: R,
		dest: usize,
	) where
		R: RangeBounds<usize>,
	{
		let source = dvl::normalize_range(src, self.len());
		let rev = source.contains(&dest);
		let dest = dest .. dest + source.len();

		/* The `&mut self` receiver ensures that this method has an exclusive
		access to the bits of its region prior to entry. In order to satisfy
		element-based aliasing rules, the correct but pessimal behavior is to
		mark the entirety of the source and destination subregions *may*
		overlap, either in the actual bits they affect **or** merely in the
		elements that contain them. As this is an `_unchecked` method, it is
		preferable to unconditionally taint the regions rather than compute
		whether the taint is necessary. For performance, the fact that this
		method has exclusive access to its bits (and will be already-tainted if
		external aliases exist) should suffice to ensure that issuing lock
		instructions will not in fact result in bus delays while the processor
		clears the bus.

		The actual alias tainting can be deferred to the loop header, since
		construction of aliased *pointers* is fine, and the reference tainting
		precludes the simultaneous liveness of untainted im/mut references.
		*/
		let from: *const Self = self.get_unchecked(source) as *const _;
		//  This can stay unaliased for now, because `.{,r}chunks_mut()` taints.
		let to: *mut Self = self.get_unchecked_mut(dest) as *mut _;
		let chunk_size = <usize as BitMemory>::BITS as usize;
		if rev {
			for (src, dst) in (&*from)
				.alias()
				.rchunks(chunk_size)
				.zip((&mut *to).rchunks_mut(chunk_size))
			{
				dst.store_le::<usize>(src.load_le::<usize>());
			}
		}
		else {
			for (src, dst) in (&*from)
				.alias()
				.chunks(chunk_size)
				.zip((&mut *to).chunks_mut(chunk_size))
			{
				dst.store_le::<usize>(src.load_le::<usize>());
			}
		}
	}

	/// Accelerates equality checking with batch loads.
	pub(crate) fn sp_eq(&self, other: &Self) -> bool {
		if self.len() != other.len() {
			return false;
		}
		let chunk_size = <usize as BitMemory>::BITS as usize;
		self.chunks(chunk_size)
			.zip(other.chunks(chunk_size))
			.all(|(a, b)| a.load_le::<usize>() == b.load_le::<usize>())
	}

	/// Seeks the index of the first `1` bit in the bit-slice.
	pub(crate) fn sp_iter_ones_first(&self) -> Option<usize> {
		let mut accum = 0;

		match self.domain() {
			Domain::Enclave { head, elem, tail } => {
				let val = (Lsb0::mask(head, tail) & elem.load_value()).value();
				if val != T::Mem::ZERO {
					accum +=
						val.trailing_zeros() as usize - head.value() as usize;
					return Some(accum);
				}
				None
			},
			Domain::Region { head, body, tail } => {
				if let Some((head, elem)) = head {
					let val =
						(Lsb0::mask(head, None) & elem.load_value()).value();
					accum +=
						val.trailing_zeros() as usize - head.value() as usize;
					if val != T::Mem::ZERO {
						return Some(accum);
					}
				}

				for elem in body {
					let val = elem.load_value();
					accum += val.trailing_zeros() as usize;
					if val != T::Mem::ZERO {
						return Some(accum);
					}
				}

				if let Some((elem, tail)) = tail {
					let val =
						(Lsb0::mask(None, tail) & elem.load_value()).value();
					if val != T::Mem::ZERO {
						accum += val.trailing_zeros() as usize;
						return Some(accum);
					}
				}

				None
			},
		}
	}

	/// Seeks the index of the last `1` bit in the bit-slice.
	pub(crate) fn sp_iter_ones_last(&self) -> Option<usize> {
		let mut out = match self.len() {
			0 => return None,
			n => n,
		};
		(|| match self.domain() {
			Domain::Enclave { head, elem, tail } => {
				let val = (Lsb0::mask(head, tail) & elem.load_value()).value();
				let dead_bits = T::Mem::BITS - tail.value();
				if val != T::Mem::ZERO {
					out -= val.leading_zeros() as usize - dead_bits as usize;
					return Some(out);
				}
				None
			},
			Domain::Region { head, body, tail } => {
				if let Some((elem, tail)) = tail {
					let val =
						(Lsb0::mask(None, tail) & elem.load_value()).value();
					let dead_bits =
						T::Mem::BITS as usize - tail.value() as usize;
					out -= val.leading_zeros() as usize - dead_bits;
					if val != T::Mem::ZERO {
						return Some(out);
					}
				}

				for elem in body.iter().rev() {
					let val = elem.load_value();
					out -= val.leading_zeros() as usize;
					if val != T::Mem::ZERO {
						return Some(out);
					}
				}

				if let Some((head, elem)) = head {
					let val =
						(Lsb0::mask(head, None) & elem.load_value()).value();
					if val != T::Mem::ZERO {
						out -= val.leading_zeros() as usize;
						return Some(out);
					}
				}

				None
			},
		})()
		.map(|idx| idx - 1)
	}

	/// Seeks the index of the first `0` bit in the bit-slice.
	pub(crate) fn sp_iter_zeros_first(&self) -> Option<usize> {
		let mut accum = 0;

		match self.domain() {
			Domain::Enclave { head, elem, tail } => {
				//  Load, invert, then mask and search for `1`.
				let val = (Lsb0::mask(head, tail) & !elem.load_value()).value();
				accum += val.trailing_zeros() as usize - head.value() as usize;
				if val != T::Mem::ZERO {
					return Some(accum);
				}
				None
			},
			Domain::Region { head, body, tail } => {
				if let Some((head, elem)) = head {
					let val =
						(Lsb0::mask(head, None) & !elem.load_value()).value();
					accum +=
						val.trailing_zeros() as usize - head.value() as usize;
					if val != T::Mem::ZERO {
						return Some(accum);
					}
				}

				for elem in body {
					let val = !elem.load_value();
					accum += val.trailing_zeros() as usize;
					if val != T::Mem::ZERO {
						return Some(accum);
					}
				}

				if let Some((elem, tail)) = tail {
					let val =
						(Lsb0::mask(None, tail) & !elem.load_value()).value();
					accum += val.trailing_zeros() as usize;
					if val != T::Mem::ZERO {
						return Some(accum);
					}
				}

				None
			},
		}
	}

	/// Seeks the index of the last `0` bit in the bit-slice.
	pub(crate) fn sp_iter_zeros_last(&self) -> Option<usize> {
		let mut out = match self.len() {
			0 => return None,
			n => n,
		};
		(|| match self.domain() {
			Domain::Enclave { head, elem, tail } => {
				let val = (Lsb0::mask(head, tail) & !elem.load_value()).value();
				let dead_bits = T::Mem::BITS - tail.value();
				if val != T::Mem::ZERO {
					out -= val.leading_zeros() as usize - dead_bits as usize;
					return Some(out);
				}
				None
			},
			Domain::Region { head, body, tail } => {
				if let Some((elem, tail)) = tail {
					let val =
						(Lsb0::mask(None, tail) & !elem.load_value()).value();
					let dead_bits =
						T::Mem::BITS as usize - tail.value() as usize;
					out -= val.leading_zeros() as usize - dead_bits;
					if val != T::Mem::ZERO {
						return Some(out);
					}
				}

				for elem in body.iter().rev() {
					let val = !elem.load_value();
					out -= val.leading_zeros() as usize;
					if val != T::Mem::ZERO {
						return Some(out);
					}
				}

				if let Some((head, elem)) = head {
					let val =
						(Lsb0::mask(head, None) & !elem.load_value()).value();
					if val != T::Mem::ZERO {
						out -= val.leading_zeros() as usize;
						return Some(out);
					}
				}

				None
			},
		})()
		.map(|idx| idx - 1)
	}
}

/** Order-specialized function implementations.

These functions use [`BitField`] to provide batched load/store behavior.
Where loads or stores cross a `T` element boundary, they use the `_be`
behavior to ensure that bits stay in the correct order relative to each
other, even as they may change position within an element.

[`BitField`]: crate::field::BitField
**/
impl<T> BitSlice<Msb0, T>
where T: BitStore
{
	/// Accelerates copies between disjoint slices with batch loads.
	pub(crate) fn sp_copy_from_bitslice(&mut self, src: &Self) {
		assert_eq!(
			self.len(),
			src.len(),
			"Copying between slices requires equal lengths"
		);

		let chunk_size = <usize as BitMemory>::BITS as usize;
		for (to, from) in unsafe { self.chunks_mut(chunk_size).remove_alias() }
			.zip(src.chunks(chunk_size))
		{
			to.store_be::<usize>(from.load_be::<usize>())
		}
	}

	/// Accelerates possibly-overlapping copies within a single slice with batch
	/// loads.
	pub(crate) unsafe fn sp_copy_within_unchecked<R>(
		&mut self,
		src: R,
		dest: usize,
	) where
		R: RangeBounds<usize>,
	{
		let source = dvl::normalize_range(src, self.len());
		let rev = source.contains(&dest);
		let dest = dest .. dest + source.len();

		let from: *const Self = self.get_unchecked(source) as *const _;
		let to: *mut Self = self.get_unchecked_mut(dest) as *mut _;
		let chunk_size = <usize as BitMemory>::BITS as usize;
		if rev {
			for (src, dst) in (&*from)
				.alias()
				.rchunks(chunk_size)
				.zip((&mut *to).rchunks_mut(chunk_size))
			{
				dst.store_be::<usize>(src.load_be::<usize>());
			}
		}
		else {
			for (src, dst) in (&*from)
				.alias()
				.chunks(chunk_size)
				.zip((&mut *to).chunks_mut(chunk_size))
			{
				dst.store_be::<usize>(src.load_be::<usize>());
			}
		}
	}

	/// Accelerates equality checking with batch loads.
	pub(crate) fn sp_eq(&self, other: &Self) -> bool {
		if self.len() != other.len() {
			return false;
		}
		let chunk_size = <usize as BitMemory>::BITS as usize;
		self.chunks(chunk_size)
			.zip(other.chunks(chunk_size))
			.all(|(a, b)| a.load_be::<usize>() == b.load_be::<usize>())
	}

	/// Seeks the index of the first `1` bit in the bit-slice.
	pub(crate) fn sp_iter_ones_first(&self) -> Option<usize> {
		let mut accum = 0;

		match self.domain() {
			Domain::Enclave { head, elem, tail } => {
				let val = (Msb0::mask(head, tail) & elem.load_value()).value();
				accum += val.leading_zeros() as usize - head.value() as usize;
				if val != T::Mem::ZERO {
					return Some(accum);
				}
				None
			},
			Domain::Region { head, body, tail } => {
				if let Some((head, elem)) = head {
					let val =
						(Msb0::mask(head, None) & elem.load_value()).value();
					accum +=
						val.leading_zeros() as usize - head.value() as usize;
					if val != T::Mem::ZERO {
						return Some(accum);
					}
				}

				for elem in body {
					let val = elem.load_value();
					accum += val.leading_zeros() as usize;
					if val != T::Mem::ZERO {
						return Some(accum);
					}
				}

				if let Some((elem, tail)) = tail {
					let val =
						(Msb0::mask(None, tail) & elem.load_value()).value();
					accum += val.leading_zeros() as usize;
					if val != T::Mem::ZERO {
						return Some(accum);
					}
				}

				None
			},
		}
	}

	/// Seeks the index of the last `1` bit in the bit-slice.
	pub(crate) fn sp_iter_ones_last(&self) -> Option<usize> {
		//  Set the state tracker to the last live index in the bit-slice.
		let mut out = match self.len() {
			0 => return None,
			n => n - 1,
		};
		match self.domain() {
			Domain::Enclave { head, elem, tail } => {
				let val = (Msb0::mask(head, tail) & elem.load_value()).value();
				let dead_bits = T::Mem::BITS - tail.value();
				if val != T::Mem::ZERO {
					out -= val.trailing_zeros() as usize - dead_bits as usize;
					return Some(out);
				}
				None
			},
			Domain::Region { head, body, tail } => {
				if let Some((elem, tail)) = tail {
					let val =
						(Msb0::mask(None, tail) & elem.load_value()).value();
					let dead_bits =
						T::Mem::BITS as usize - tail.value() as usize;
					out -= val.trailing_zeros() as usize - dead_bits;
					if val != T::Mem::ZERO {
						return Some(out);
					}
				}

				for elem in body.iter().rev() {
					let val = elem.load_value();
					out -= val.trailing_zeros() as usize;
					if val != T::Mem::ZERO {
						return Some(out);
					}
				}

				if let Some((head, elem)) = head {
					let val =
						(Msb0::mask(head, None) & elem.load_value()).value();
					if val != T::Mem::ZERO {
						out -= val.trailing_zeros() as usize;
						return Some(out);
					}
				}

				None
			},
		}
	}

	/// Seeks the index of the first `0` bit in the bit-slice.
	pub(crate) fn sp_iter_zeros_first(&self) -> Option<usize> {
		let mut accum = 0;

		match self.domain() {
			Domain::Enclave { head, elem, tail } => {
				let val = (Msb0::mask(head, tail) & !elem.load_value()).value();
				accum += val.leading_zeros() as usize - head.value() as usize;
				if val != T::Mem::ZERO {
					return Some(accum);
				}
				None
			},
			Domain::Region { head, body, tail } => {
				if let Some((head, elem)) = head {
					let val =
						(Msb0::mask(head, None) & !elem.load_value()).value();
					accum +=
						val.leading_zeros() as usize - head.value() as usize;
					if val != T::Mem::ZERO {
						return Some(accum);
					}
				}

				for elem in body {
					let val = !elem.load_value();
					accum += val.leading_zeros() as usize;
					if val != T::Mem::ZERO {
						return Some(accum);
					}
				}

				if let Some((elem, tail)) = tail {
					let val =
						(Msb0::mask(None, tail) & !elem.load_value()).value();
					accum += val.leading_zeros() as usize;
					if val != T::Mem::ZERO {
						return Some(accum);
					}
				}

				None
			},
		}
	}

	/// Seeks the index of the last `0` bit in the bit-slice.
	pub(crate) fn sp_iter_zeros_last(&self) -> Option<usize> {
		let mut out = match self.len() {
			0 => return None,
			n => n - 1,
		};
		match self.domain() {
			Domain::Enclave { head, elem, tail } => {
				let val = (Msb0::mask(head, tail) & !elem.load_value()).value();
				let dead_bits = T::Mem::BITS - tail.value();
				if val != T::Mem::ZERO {
					out -= val.trailing_zeros() as usize - dead_bits as usize;
					return Some(out);
				}
				None
			},
			Domain::Region { head, body, tail } => {
				if let Some((elem, tail)) = tail {
					let val =
						(Msb0::mask(None, tail) & !elem.load_value()).value();
					let dead_bits =
						T::Mem::BITS as usize - tail.value() as usize;
					out -= val.trailing_zeros() as usize - dead_bits;
					if val != T::Mem::ZERO {
						return Some(out);
					}
				}

				for elem in body.iter().rev() {
					let val = !elem.load_value();
					out -= val.trailing_zeros() as usize;
					if val != T::Mem::ZERO {
						return Some(out);
					}
				}

				if let Some((head, elem)) = head {
					let val =
						(Msb0::mask(head, None) & !elem.load_value()).value();
					if val != T::Mem::ZERO {
						out -= val.trailing_zeros() as usize;
						return Some(out);
					}
				}

				None
			},
		}
	}
}
