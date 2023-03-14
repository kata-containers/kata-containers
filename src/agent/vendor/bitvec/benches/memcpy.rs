/*! Benchmarks for `BitSlice::copy_from_slice`.

The `copy_from_slice` implementation attempts to detect slice conditions that
allow element-wise `memcpy` behavior, rather than the conservative bit-by-bit
iteration, as element load/stores are faster than reading and writing each bit
in an element individually.
!*/

use bitvec::{
	mem::BitMemory,
	prelude::*,
};

use criterion::{
	criterion_group,
	criterion_main,
	BenchmarkId,
	Criterion,
	Throughput,
};

//  One kibibit
const FACTOR: usize = 1024;

pub fn benchmarks(crit: &mut Criterion) {
	fn steps() -> impl Iterator<Item = (BenchmarkId, usize, Throughput)> {
		[1, 2, 4, 8, 16, 24, 32, 40, 48, 56, 64]
			.iter()
			.copied()
			.map(|n| {
				(
					BenchmarkId::from_parameter(n),
					n * FACTOR,
					Throughput::Elements(
						(n * FACTOR / <usize as BitMemory>::BITS as usize)
							as u64,
					),
				)
			})
	}

	fn mkgroup<
		O: BitOrder,
		F: FnMut(usize, &mut BitSlice, &BitSlice<O, usize>),
	>(
		name: &'static str,
		crit: &mut Criterion,
		mut func: F,
	) {
		let mut group = crit.benchmark_group(name);
		for (id, len, elems) in steps() {
			let mut dst = BitVec::repeat(false, len);
			let src = BitVec::<O, usize>::repeat(true, len);

			let dst = dst.as_mut_bitslice();
			let src = src.as_bitslice();
			group.throughput(elems);
			group.bench_function(id, |b| b.iter(|| func(len, dst, src)));
		}
		group.finish();
	}

	mkgroup::<Lsb0, _>("memcpy", crit, |len, dst, src| {
		dst[10 .. len - 10].copy_from_bitslice(&src[10 .. len - 10]);
	});

	mkgroup::<Lsb0, _>("load_store", crit, |len, dst, src| {
		dst[10 ..].copy_from_bitslice(&src[.. len - 10]);
	});

	mkgroup::<Lsb0, _>("bitwise", crit, |_, dst, src| {
		dst.clone_from_bitslice(src)
	});

	mkgroup::<Msb0, _>("mismatch", crit, |_, dst, src| {
		dst.clone_from_bitslice(src)
	});
}

criterion_group!(benches, benchmarks);
criterion_main!(benches);
