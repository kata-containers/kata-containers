#![feature(test)]

extern crate test;
extern crate itertools;

use itertools::Itertools;
use itertools::cloned;
use test::Bencher;

trait IterEx : Iterator {
    // Another efficient implementation against which to compare,
    // but needs `std` so is less desirable.
    fn tree_fold1_vec<F>(self, mut f: F) -> Option<Self::Item>
        where F: FnMut(Self::Item, Self::Item) -> Self::Item,
              Self: Sized,
    {
        let hint = self.size_hint().0;
        let cap = std::mem::size_of::<usize>() * 8 - hint.leading_zeros() as usize;
        let mut stack = Vec::with_capacity(cap);
        self.enumerate().foreach(|(mut i, mut x)| {
            while (i & 1) != 0 {
                x = f(stack.pop().unwrap(), x);
                i >>= 1;
            }
            stack.push(x);
        });
        stack.into_iter().fold1(f)
    }
}
impl<T:Iterator> IterEx for T {}

macro_rules! def_benchs {
    ($N:expr,
     $FUN:ident,
     $BENCH_NAME:ident,
     ) => (
        mod $BENCH_NAME {
        use super::*;

        #[bench]
        fn sum(b: &mut Bencher) {
            let v: Vec<u32> = (0.. $N).collect();
            b.iter(|| {
                cloned(&v).$FUN(|x, y| x + y)
            });
        }

        #[bench]
        fn complex_iter(b: &mut Bencher) {
            let u = (3..).take($N / 2);
            let v = (5..).take($N / 2);
            let it = u.chain(v);

            b.iter(|| {
                it.clone().map(|x| x as f32).$FUN(f32::atan2)
            });
        }

        #[bench]
        fn string_format(b: &mut Bencher) {
            // This goes quadratic with linear `fold1`, so use a smaller
            // size to not waste too much time in travis.  The allocations
            // in here are so expensive anyway that it'll still take
            // way longer per iteration than the other two benchmarks.
            let v: Vec<u32> = (0.. ($N/4)).collect();
            b.iter(|| {
                cloned(&v).map(|x| x.to_string()).$FUN(|x, y| format!("{} + {}", x, y))
            });
        }
        }
    )
}

def_benchs!{
    10_000,
    fold1,
    fold1_10k,
}

def_benchs!{
    10_000,
    tree_fold1,
    tree_fold1_stack_10k,
}

def_benchs!{
    10_000,
    tree_fold1_vec,
    tree_fold1_vec_10k,
}

def_benchs!{
    100,
    fold1,
    fold1_100,
}

def_benchs!{
    100,
    tree_fold1,
    tree_fold1_stack_100,
}

def_benchs!{
    100,
    tree_fold1_vec,
    tree_fold1_vec_100,
}

def_benchs!{
    8,
    fold1,
    fold1_08,
}

def_benchs!{
    8,
    tree_fold1,
    tree_fold1_stack_08,
}

def_benchs!{
    8,
    tree_fold1_vec,
    tree_fold1_vec_08,
}
