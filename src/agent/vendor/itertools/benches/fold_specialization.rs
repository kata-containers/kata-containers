#![feature(test)]

extern crate test;
extern crate itertools;

use itertools::Itertools;

struct Unspecialized<I>(I);

impl<I> Iterator for Unspecialized<I>
where I: Iterator
{
    type Item = I::Item;

    #[inline(always)]
    fn next(&mut self) -> Option<I::Item> {
        self.0.next()
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

mod specialization {
    use super::*;

    mod intersperse {
        use super::*;

        #[bench]
        fn external(b: &mut test::Bencher)
        {
            let arr = [1; 1024];

            b.iter(|| {
                let mut sum = 0;
                for &x in arr.into_iter().intersperse(&0) {
                    sum += x;
                }
                sum
            })
        }

        #[bench]
        fn internal_specialized(b: &mut test::Bencher)
        {
            let arr = [1; 1024];

            b.iter(|| {
                arr.into_iter().intersperse(&0).fold(0, |acc, x| acc + x)
            })
        }

        #[bench]
        fn internal_unspecialized(b: &mut test::Bencher)
        {
            let arr = [1; 1024];

            b.iter(|| {
                Unspecialized(arr.into_iter().intersperse(&0)).fold(0, |acc, x| acc + x)
            })
        }
    }
}
