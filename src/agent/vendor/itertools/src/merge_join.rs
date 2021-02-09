use std::cmp::Ordering;
use std::iter::Fuse;
use std::fmt;

use super::adaptors::{PutBack, put_back};
use either_or_both::EitherOrBoth;

/// Return an iterator adaptor that merge-joins items from the two base iterators in ascending order.
///
/// See [`.merge_join_by()`](trait.Itertools.html#method.merge_join_by) for more information.
pub fn merge_join_by<I, J, F>(left: I, right: J, cmp_fn: F)
    -> MergeJoinBy<I::IntoIter, J::IntoIter, F>
    where I: IntoIterator,
          J: IntoIterator,
          F: FnMut(&I::Item, &J::Item) -> Ordering
{
    MergeJoinBy {
        left: put_back(left.into_iter().fuse()),
        right: put_back(right.into_iter().fuse()),
        cmp_fn: cmp_fn
    }
}

/// An iterator adaptor that merge-joins items from the two base iterators in ascending order.
///
/// See [`.merge_join_by()`](../trait.Itertools.html#method.merge_join_by) for more information.
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct MergeJoinBy<I: Iterator, J: Iterator, F> {
    left: PutBack<Fuse<I>>,
    right: PutBack<Fuse<J>>,
    cmp_fn: F
}

impl<I, J, F> fmt::Debug for MergeJoinBy<I, J, F>
    where I: Iterator + fmt::Debug,
          I::Item: fmt::Debug,
          J: Iterator + fmt::Debug,
          J::Item: fmt::Debug,
{
    debug_fmt_fields!(MergeJoinBy, left, right);
}

impl<I, J, F> Iterator for MergeJoinBy<I, J, F>
    where I: Iterator,
          J: Iterator,
          F: FnMut(&I::Item, &J::Item) -> Ordering
{
    type Item = EitherOrBoth<I::Item, J::Item>;

    fn next(&mut self) -> Option<Self::Item> {
        match (self.left.next(), self.right.next()) {
            (None, None) => None,
            (Some(left), None) =>
                Some(EitherOrBoth::Left(left)),
            (None, Some(right)) =>
                Some(EitherOrBoth::Right(right)),
            (Some(left), Some(right)) => {
                match (self.cmp_fn)(&left, &right) {
                    Ordering::Equal =>
                        Some(EitherOrBoth::Both(left, right)),
                    Ordering::Less => {
                        self.right.put_back(right);
                        Some(EitherOrBoth::Left(left))
                    },
                    Ordering::Greater => {
                        self.left.put_back(left);
                        Some(EitherOrBoth::Right(right))
                    }
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (a_lower, a_upper) = self.left.size_hint();
        let (b_lower, b_upper) = self.right.size_hint();

        let lower = ::std::cmp::max(a_lower, b_lower);

        let upper = match (a_upper, b_upper) {
            (Some(x), Some(y)) => Some(x + y),
            _ => None,
        };

        (lower, upper)
    }
}
