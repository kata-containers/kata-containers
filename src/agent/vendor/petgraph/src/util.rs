
use std::iter;

pub fn enumerate<I>(iterable: I) -> iter::Enumerate<I::IntoIter>
    where I: IntoIterator
{
    iterable.into_iter().enumerate()
}

#[cfg(feature = "serde-1")]
pub fn rev<I>(iterable: I) -> iter::Rev<I::IntoIter>
    where I: IntoIterator,
          I::IntoIter: DoubleEndedIterator
{
    iterable.into_iter().rev()
}

pub fn zip<I, J>(i: I, j: J) -> iter::Zip<I::IntoIter, J::IntoIter>
    where I: IntoIterator,
          J: IntoIterator
{
    i.into_iter().zip(j)
}
