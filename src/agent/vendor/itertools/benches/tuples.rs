#![feature(test)]

extern crate test;
extern crate itertools;

use test::Bencher;
use itertools::Itertools;

fn s1(a: u32) -> u32 {
    a
}

fn s2(a: u32, b: u32) -> u32 {
    a + b
}

fn s3(a: u32, b: u32, c: u32) -> u32 {
    a + b + c
}

fn s4(a: u32, b: u32, c: u32, d: u32) -> u32 {
    a + b + c + d
}

fn sum_s1(s: &[u32]) -> u32 {
    s1(s[0])
}

fn sum_s2(s: &[u32]) -> u32 {
    s2(s[0], s[1])
}

fn sum_s3(s: &[u32]) -> u32 {
    s3(s[0], s[1], s[2])
}

fn sum_s4(s: &[u32]) -> u32 {
    s4(s[0], s[1], s[2], s[3])
}

fn sum_t1(s: &(&u32, )) -> u32 {
    s1(*s.0)
}

fn sum_t2(s: &(&u32, &u32)) -> u32 {
    s2(*s.0, *s.1)
}

fn sum_t3(s: &(&u32, &u32, &u32)) -> u32 {
    s3(*s.0, *s.1, *s.2)
}

fn sum_t4(s: &(&u32, &u32, &u32, &u32)) -> u32 {
    s4(*s.0, *s.1, *s.2, *s.3)
}

macro_rules! def_benchs {
    ($N:expr;
     $TUPLE_FUN:ident,
     $TUPLES:ident,
     $TUPLE_WINDOWS:ident;
     $SLICE_FUN:ident,
     $CHUNKS:ident,
     $WINDOWS:ident;
     $FOR_CHUNKS:ident,
     $FOR_WINDOWS:ident
     ) => (
        #[bench]
        fn $FOR_CHUNKS(b: &mut Bencher) {
            let v: Vec<u32> = (0.. $N * 1_000).collect();
            let mut s = 0;
            b.iter(|| {
                let mut j = 0;
                for _ in 0..1_000 {
                    s += $SLICE_FUN(&v[j..(j + $N)]);
                    j += $N;
                }
                s
            });
        }

        #[bench]
        fn $FOR_WINDOWS(b: &mut Bencher) {
            let v: Vec<u32> = (0..1_000).collect();
            let mut s = 0;
            b.iter(|| {
                for i in 0..(1_000 - $N) {
                    s += $SLICE_FUN(&v[i..(i + $N)]);
                }
                s
            });
        }

        #[bench]
        fn $TUPLES(b: &mut Bencher) {
            let v: Vec<u32> = (0.. $N * 1_000).collect();
            let mut s = 0;
            b.iter(|| {
                for x in v.iter().tuples() {
                    s += $TUPLE_FUN(&x);
                }
                s
            });
        }

        #[bench]
        fn $CHUNKS(b: &mut Bencher) {
            let v: Vec<u32> = (0.. $N * 1_000).collect();
            let mut s = 0;
            b.iter(|| {
                for x in v.chunks($N) {
                    s += $SLICE_FUN(x);
                }
                s
            });
        }

        #[bench]
        fn $TUPLE_WINDOWS(b: &mut Bencher) {
            let v: Vec<u32> = (0..1_000).collect();
            let mut s = 0;
            b.iter(|| {
                for x in v.iter().tuple_windows() {
                    s += $TUPLE_FUN(&x);
                }
                s
            });
        }

        #[bench]
        fn $WINDOWS(b: &mut Bencher) {
            let v: Vec<u32> = (0..1_000).collect();
            let mut s = 0;
            b.iter(|| {
                for x in v.windows($N) {
                    s += $SLICE_FUN(x);
                }
                s
            });
        }
    )
}

def_benchs!{
    1;
    sum_t1,
    tuple_chunks_1,
    tuple_windows_1;
    sum_s1,
    slice_chunks_1,
    slice_windows_1;
    for_chunks_1,
    for_windows_1
}

def_benchs!{
    2;
    sum_t2,
    tuple_chunks_2,
    tuple_windows_2;
    sum_s2,
    slice_chunks_2,
    slice_windows_2;
    for_chunks_2,
    for_windows_2
}

def_benchs!{
    3;
    sum_t3,
    tuple_chunks_3,
    tuple_windows_3;
    sum_s3,
    slice_chunks_3,
    slice_windows_3;
    for_chunks_3,
    for_windows_3
}

def_benchs!{
    4;
    sum_t4,
    tuple_chunks_4,
    tuple_windows_4;
    sum_s4,
    slice_chunks_4,
    slice_windows_4;
    for_chunks_4,
    for_windows_4
}
