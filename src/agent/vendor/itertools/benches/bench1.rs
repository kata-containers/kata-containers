#![feature(test)]

extern crate test;
#[macro_use] extern crate itertools;

use test::{black_box};
use itertools::Itertools;

use itertools::free::cloned;
use itertools::Permutations;

use std::iter::repeat;
use std::cmp;
use std::ops::{Add, Range};

mod extra;

use extra::ZipSlices;

#[bench]
fn slice_iter(b: &mut test::Bencher)
{
    let xs: Vec<_> = repeat(1i32).take(20).collect();
    b.iter(|| for elt in xs.iter() {
        test::black_box(elt);
    })
}

#[bench]
fn slice_iter_rev(b: &mut test::Bencher)
{
    let xs: Vec<_> = repeat(1i32).take(20).collect();
    b.iter(|| for elt in xs.iter().rev() {
        test::black_box(elt);
    })
}

#[bench]
fn zip_default_zip(b: &mut test::Bencher)
{
    let xs = vec![0; 1024];
    let ys = vec![0; 768];
    let xs = black_box(xs);
    let ys = black_box(ys);

    b.iter(|| {
        for (&x, &y) in xs.iter().zip(&ys) {
            test::black_box(x);
            test::black_box(y);
        }
    })
}

#[bench]
fn zipdot_i32_default_zip(b: &mut test::Bencher)
{
    let xs = vec![2; 1024];
    let ys = vec![2; 768];
    let xs = black_box(xs);
    let ys = black_box(ys);

    b.iter(|| {
        let mut s = 0;
        for (&x, &y) in xs.iter().zip(&ys) {
            s += x * y;
        }
        s
    })
}

#[bench]
fn zipdot_f32_default_zip(b: &mut test::Bencher)
{
    let xs = vec![2f32; 1024];
    let ys = vec![2f32; 768];
    let xs = black_box(xs);
    let ys = black_box(ys);

    b.iter(|| {
        let mut s = 0.;
        for (&x, &y) in xs.iter().zip(&ys) {
            s += x * y;
        }
        s
    })
}

#[bench]
fn zip_default_zip3(b: &mut test::Bencher)
{
    let xs = vec![0; 1024];
    let ys = vec![0; 768];
    let zs = vec![0; 766];
    let xs = black_box(xs);
    let ys = black_box(ys);
    let zs = black_box(zs);

    b.iter(|| {
        for ((&x, &y), &z) in xs.iter().zip(&ys).zip(&zs) {
            test::black_box(x);
            test::black_box(y);
            test::black_box(z);
        }
    })
}

#[bench]
fn zip_slices_ziptuple(b: &mut test::Bencher)
{
    let xs = vec![0; 1024];
    let ys = vec![0; 768];

    b.iter(|| {
        let xs = black_box(&xs);
        let ys = black_box(&ys);
        for (&x, &y) in itertools::multizip((xs, ys)) {
            test::black_box(x);
            test::black_box(y);
        }
    })
}

#[bench]
fn zipslices(b: &mut test::Bencher)
{
    let xs = vec![0; 1024];
    let ys = vec![0; 768];
    let xs = black_box(xs);
    let ys = black_box(ys);

    b.iter(|| {
        for (&x, &y) in ZipSlices::new(&xs, &ys) {
            test::black_box(x);
            test::black_box(y);
        }
    })
}

#[bench]
fn zipslices_mut(b: &mut test::Bencher)
{
    let xs = vec![0; 1024];
    let ys = vec![0; 768];
    let xs = black_box(xs);
    let mut ys = black_box(ys);

    b.iter(|| {
        for (&x, &mut y) in ZipSlices::from_slices(&xs[..], &mut ys[..]) {
            test::black_box(x);
            test::black_box(y);
        }
    })
}

#[bench]
fn zipdot_i32_zipslices(b: &mut test::Bencher)
{
    let xs = vec![2; 1024];
    let ys = vec![2; 768];
    let xs = black_box(xs);
    let ys = black_box(ys);

    b.iter(|| {
        let mut s = 0i32;
        for (&x, &y) in ZipSlices::new(&xs, &ys) {
            s += x * y;
        }
        s
    })
}

#[bench]
fn zipdot_f32_zipslices(b: &mut test::Bencher)
{
    let xs = vec![2f32; 1024];
    let ys = vec![2f32; 768];
    let xs = black_box(xs);
    let ys = black_box(ys);

    b.iter(|| {
        let mut s = 0.;
        for (&x, &y) in ZipSlices::new(&xs, &ys) {
            s += x * y;
        }
        s
    })
}


#[bench]
fn zip_checked_counted_loop(b: &mut test::Bencher)
{
    let xs = vec![0; 1024];
    let ys = vec![0; 768];
    let xs = black_box(xs);
    let ys = black_box(ys);

    b.iter(|| {
        // Must slice to equal lengths, and then bounds checks are eliminated!
        let len = cmp::min(xs.len(), ys.len());
        let xs = &xs[..len];
        let ys = &ys[..len];

        for i in 0..len {
            let x = xs[i];
            let y = ys[i];
            test::black_box(x);
            test::black_box(y);
        }
    })
}

#[bench]
fn zipdot_i32_checked_counted_loop(b: &mut test::Bencher)
{
    let xs = vec![2; 1024];
    let ys = vec![2; 768];
    let xs = black_box(xs);
    let ys = black_box(ys);

    b.iter(|| {
        // Must slice to equal lengths, and then bounds checks are eliminated!
        let len = cmp::min(xs.len(), ys.len());
        let xs = &xs[..len];
        let ys = &ys[..len];

        let mut s = 0i32;

        for i in 0..len {
            s += xs[i] * ys[i];
        }
        s
    })
}

#[bench]
fn zipdot_f32_checked_counted_loop(b: &mut test::Bencher)
{
    let xs = vec![2f32; 1024];
    let ys = vec![2f32; 768];
    let xs = black_box(xs);
    let ys = black_box(ys);

    b.iter(|| {
        // Must slice to equal lengths, and then bounds checks are eliminated!
        let len = cmp::min(xs.len(), ys.len());
        let xs = &xs[..len];
        let ys = &ys[..len];

        let mut s = 0.;

        for i in 0..len {
            s += xs[i] * ys[i];
        }
        s
    })
}

#[bench]
fn zipdot_f32_checked_counted_unrolled_loop(b: &mut test::Bencher)
{
    let xs = vec![2f32; 1024];
    let ys = vec![2f32; 768];
    let xs = black_box(xs);
    let ys = black_box(ys);

    b.iter(|| {
        // Must slice to equal lengths, and then bounds checks are eliminated!
        let len = cmp::min(xs.len(), ys.len());
        let mut xs = &xs[..len];
        let mut ys = &ys[..len];

        let mut s = 0.;
        let (mut p0, mut p1, mut p2, mut p3, mut p4, mut p5, mut p6, mut p7) =
            (0., 0., 0., 0., 0., 0., 0., 0.);

        // how to unroll and have bounds checks eliminated (by cristicbz)
        // split sum into eight parts to enable vectorization (by bluss)
        while xs.len() >= 8 {
            p0 += xs[0] * ys[0];
            p1 += xs[1] * ys[1];
            p2 += xs[2] * ys[2];
            p3 += xs[3] * ys[3];
            p4 += xs[4] * ys[4];
            p5 += xs[5] * ys[5];
            p6 += xs[6] * ys[6];
            p7 += xs[7] * ys[7];

            xs = &xs[8..];
            ys = &ys[8..];
        }
        s += p0 + p4;
        s += p1 + p5;
        s += p2 + p6;
        s += p3 + p7;

        for i in 0..xs.len() {
            s += xs[i] * ys[i];
        }
        s
    })
}

#[bench]
fn zip_unchecked_counted_loop(b: &mut test::Bencher)
{
    let xs = vec![0; 1024];
    let ys = vec![0; 768];
    let xs = black_box(xs);
    let ys = black_box(ys);

    b.iter(|| {
        let len = cmp::min(xs.len(), ys.len());
        for i in 0..len {
            unsafe {
            let x = *xs.get_unchecked(i);
            let y = *ys.get_unchecked(i);
            test::black_box(x);
            test::black_box(y);
            }
        }
    })
}

#[bench]
fn zipdot_i32_unchecked_counted_loop(b: &mut test::Bencher)
{
    let xs = vec![2; 1024];
    let ys = vec![2; 768];
    let xs = black_box(xs);
    let ys = black_box(ys);

    b.iter(|| {
        let len = cmp::min(xs.len(), ys.len());
        let mut s = 0i32;
        for i in 0..len {
            unsafe {
            let x = *xs.get_unchecked(i);
            let y = *ys.get_unchecked(i);
            s += x * y;
            }
        }
        s
    })
}

#[bench]
fn zipdot_f32_unchecked_counted_loop(b: &mut test::Bencher)
{
    let xs = vec![2.; 1024];
    let ys = vec![2.; 768];
    let xs = black_box(xs);
    let ys = black_box(ys);

    b.iter(|| {
        let len = cmp::min(xs.len(), ys.len());
        let mut s = 0f32;
        for i in 0..len {
            unsafe {
            let x = *xs.get_unchecked(i);
            let y = *ys.get_unchecked(i);
            s += x * y;
            }
        }
        s
    })
}

#[bench]
fn zip_unchecked_counted_loop3(b: &mut test::Bencher)
{
    let xs = vec![0; 1024];
    let ys = vec![0; 768];
    let zs = vec![0; 766];
    let xs = black_box(xs);
    let ys = black_box(ys);
    let zs = black_box(zs);

    b.iter(|| {
        let len = cmp::min(xs.len(), cmp::min(ys.len(), zs.len()));
        for i in 0..len {
            unsafe {
            let x = *xs.get_unchecked(i);
            let y = *ys.get_unchecked(i);
            let z = *zs.get_unchecked(i);
            test::black_box(x);
            test::black_box(y);
            test::black_box(z);
            }
        }
    })
}

#[bench]
fn group_by_lazy_1(b: &mut test::Bencher) {
    let mut data = vec![0; 1024];
    for (index, elt) in data.iter_mut().enumerate() {
        *elt = index / 10;
    }

    let data = test::black_box(data);

    b.iter(|| {
        for (_key, group) in &data.iter().group_by(|elt| **elt) {
            for elt in group {
                test::black_box(elt);
            }
        }
    })
}

#[bench]
fn group_by_lazy_2(b: &mut test::Bencher) {
    let mut data = vec![0; 1024];
    for (index, elt) in data.iter_mut().enumerate() {
        *elt = index / 2;
    }

    let data = test::black_box(data);

    b.iter(|| {
        for (_key, group) in &data.iter().group_by(|elt| **elt) {
            for elt in group {
                test::black_box(elt);
            }
        }
    })
}

#[bench]
fn slice_chunks(b: &mut test::Bencher) {
    let data = vec![0; 1024];

    let data = test::black_box(data);
    let sz = test::black_box(10);

    b.iter(|| {
        for group in data.chunks(sz) {
            for elt in group {
                test::black_box(elt);
            }
        }
    })
}

#[bench]
fn chunks_lazy_1(b: &mut test::Bencher) {
    let data = vec![0; 1024];

    let data = test::black_box(data);
    let sz = test::black_box(10);

    b.iter(|| {
        for group in &data.iter().chunks(sz) {
            for elt in group {
                test::black_box(elt);
            }
        }
    })
}

#[bench]
fn equal(b: &mut test::Bencher) {
    let data = vec![7; 1024];
    let l = data.len();
    let alpha = test::black_box(&data[1..]);
    let beta = test::black_box(&data[..l - 1]);
    b.iter(|| {
        itertools::equal(alpha, beta)
    })
}

#[bench]
fn merge_default(b: &mut test::Bencher) {
    let mut data1 = vec![0; 1024];
    let mut data2 = vec![0; 800];
    let mut x = 0;
    for (_, elt) in data1.iter_mut().enumerate() {
        *elt = x;
        x += 1;
    }

    let mut y = 0;
    for (i, elt) in data2.iter_mut().enumerate() {
        *elt += y;
        if i % 3 == 0 {
            y += 3;
        } else {
            y += 0;
        }
    }
    let data1 = test::black_box(data1);
    let data2 = test::black_box(data2);
    b.iter(|| {
        data1.iter().merge(&data2).count()
    })
}

#[bench]
fn merge_by_cmp(b: &mut test::Bencher) {
    let mut data1 = vec![0; 1024];
    let mut data2 = vec![0; 800];
    let mut x = 0;
    for (_, elt) in data1.iter_mut().enumerate() {
        *elt = x;
        x += 1;
    }

    let mut y = 0;
    for (i, elt) in data2.iter_mut().enumerate() {
        *elt += y;
        if i % 3 == 0 {
            y += 3;
        } else {
            y += 0;
        }
    }
    let data1 = test::black_box(data1);
    let data2 = test::black_box(data2);
    b.iter(|| {
        data1.iter().merge_by(&data2, PartialOrd::le).count()
    })
}

#[bench]
fn merge_by_lt(b: &mut test::Bencher) {
    let mut data1 = vec![0; 1024];
    let mut data2 = vec![0; 800];
    let mut x = 0;
    for (_, elt) in data1.iter_mut().enumerate() {
        *elt = x;
        x += 1;
    }

    let mut y = 0;
    for (i, elt) in data2.iter_mut().enumerate() {
        *elt += y;
        if i % 3 == 0 {
            y += 3;
        } else {
            y += 0;
        }
    }
    let data1 = test::black_box(data1);
    let data2 = test::black_box(data2);
    b.iter(|| {
        data1.iter().merge_by(&data2, |a, b| a <= b).count()
    })
}

#[bench]
fn kmerge_default(b: &mut test::Bencher) {
    let mut data1 = vec![0; 1024];
    let mut data2 = vec![0; 800];
    let mut x = 0;
    for (_, elt) in data1.iter_mut().enumerate() {
        *elt = x;
        x += 1;
    }

    let mut y = 0;
    for (i, elt) in data2.iter_mut().enumerate() {
        *elt += y;
        if i % 3 == 0 {
            y += 3;
        } else {
            y += 0;
        }
    }
    let data1 = test::black_box(data1);
    let data2 = test::black_box(data2);
    let its = &[data1.iter(), data2.iter()];
    b.iter(|| {
        its.iter().cloned().kmerge().count()
    })
}

#[bench]
fn kmerge_tenway(b: &mut test::Bencher) {
    let mut data = vec![0; 10240];

    let mut state = 1729u16;
    fn rng(state: &mut u16) -> u16 {
        let new = state.wrapping_mul(31421) + 6927;
        *state = new;
        new
    }

    for elt in &mut data {
        *elt = rng(&mut state);
    }

    let mut chunks = Vec::new();
    let mut rest = &mut data[..];
    while rest.len() > 0 {
        let chunk_len = 1 + rng(&mut state) % 512;
        let chunk_len = cmp::min(rest.len(), chunk_len as usize);
        let (fst, tail) = {rest}.split_at_mut(chunk_len);
        fst.sort();
        chunks.push(fst.iter().cloned());
        rest = tail;
    }

    // println!("Chunk lengths: {}", chunks.iter().format_with(", ", |elt, f| f(&elt.len())));

    b.iter(|| {
        chunks.iter().cloned().kmerge().count()
    })
}


fn fast_integer_sum<I>(iter: I) -> I::Item
    where I: IntoIterator,
          I::Item: Default + Add<Output=I::Item>
{
    iter.into_iter().fold(<_>::default(), |x, y| x + y)
}


#[bench]
fn step_vec_2(b: &mut test::Bencher) {
    let v = vec![0; 1024];
    b.iter(|| {
        fast_integer_sum(cloned(v.iter().step(2)))
    });
}

#[bench]
fn step_vec_10(b: &mut test::Bencher) {
    let v = vec![0; 1024];
    b.iter(|| {
        fast_integer_sum(cloned(v.iter().step(10)))
    });
}

#[bench]
fn step_range_2(b: &mut test::Bencher) {
    let v = black_box(0..1024);
    b.iter(|| {
        fast_integer_sum(v.clone().step(2))
    });
}

#[bench]
fn step_range_10(b: &mut test::Bencher) {
    let v = black_box(0..1024);
    b.iter(|| {
        fast_integer_sum(v.clone().step(10))
    });
}

#[bench]
fn cartesian_product_iterator(b: &mut test::Bencher)
{
    let xs = vec![0; 16];

    b.iter(|| {
        let mut sum = 0;
        for (&x, &y, &z) in iproduct!(&xs, &xs, &xs) {
            sum += x;
            sum += y;
            sum += z;
        }
        sum
    })
}

#[bench]
fn cartesian_product_fold(b: &mut test::Bencher)
{
    let xs = vec![0; 16];

    b.iter(|| {
        let mut sum = 0;
        iproduct!(&xs, &xs, &xs).fold((), |(), (&x, &y, &z)| {
            sum += x;
            sum += y;
            sum += z;
        });
        sum
    })
}

#[bench]
fn multi_cartesian_product_iterator(b: &mut test::Bencher)
{
    let xs = [vec![0; 16], vec![0; 16], vec![0; 16]];

    b.iter(|| {
        let mut sum = 0;
        for x in xs.iter().multi_cartesian_product() {
            sum += x[0];
            sum += x[1];
            sum += x[2];
        }
        sum
    })
}

#[bench]
fn multi_cartesian_product_fold(b: &mut test::Bencher)
{
    let xs = [vec![0; 16], vec![0; 16], vec![0; 16]];

    b.iter(|| {
        let mut sum = 0;
        xs.iter().multi_cartesian_product().fold((), |(), x| {
            sum += x[0];
            sum += x[1];
            sum += x[2];
        });
        sum
    })
}

#[bench]
fn cartesian_product_nested_for(b: &mut test::Bencher)
{
    let xs = vec![0; 16];

    b.iter(|| {
        let mut sum = 0;
        for &x in &xs {
            for &y in &xs {
                for &z in &xs {
                    sum += x;
                    sum += y;
                    sum += z;
                }
            }
        }
        sum
    })
}

#[bench]
fn all_equal(b: &mut test::Bencher) {
    let mut xs = vec![0; 5_000_000];
    xs.extend(vec![1; 5_000_000]);

    b.iter(|| xs.iter().all_equal())
}

#[bench]
fn all_equal_for(b: &mut test::Bencher) {
    let mut xs = vec![0; 5_000_000];
    xs.extend(vec![1; 5_000_000]);

    b.iter(|| {
        for &x in &xs {
            if x != xs[0] {
                return false;
            }
        }
        true
    })
}

#[bench]
fn all_equal_default(b: &mut test::Bencher) {
    let mut xs = vec![0; 5_000_000];
    xs.extend(vec![1; 5_000_000]);

    b.iter(|| xs.iter().dedup().nth(1).is_none())
}

const PERM_COUNT: usize = 6;

#[bench]
fn permutations_iter(b: &mut test::Bencher) {
    struct NewIterator(Range<usize>);

    impl Iterator for NewIterator {
        type Item = usize;

        fn next(&mut self) -> Option<Self::Item> {
            self.0.next()
        }
    }

    b.iter(|| {
        for _ in NewIterator(0..PERM_COUNT).permutations(PERM_COUNT) {

        }
    })
}

#[bench]
fn permutations_range(b: &mut test::Bencher) {
    b.iter(|| {
        for _ in (0..PERM_COUNT).permutations(PERM_COUNT) {

        }
    })
}

#[bench]
fn permutations_slice(b: &mut test::Bencher) {
    let v = (0..PERM_COUNT).collect_vec();

    b.iter(|| {
        for _ in v.as_slice().iter().permutations(PERM_COUNT) {

        }
    })
}
