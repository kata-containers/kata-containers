use crate::collections::{String, Vec};
use crate::Bump;

/// Wrapper trait for types that support being constructed from an iterator, parameterized by an allocator.
pub trait FromIteratorIn<A> {
    /// An allocator type
    type Alloc;
    /// Similar to `from_iter`, but with a given allocator.
    fn from_iter_in<I>(iter: I, alloc: Self::Alloc) -> Self
    where
        I: IntoIterator<Item = A>;
}

impl<'bump, T> FromIteratorIn<T> for Vec<'bump, T> {
    type Alloc = &'bump Bump;
    fn from_iter_in<I>(iter: I, alloc: Self::Alloc) -> Self
    where
        I: IntoIterator<Item = T>,
    {
        Vec::from_iter_in(iter, alloc)
    }
}

impl<'a> FromIteratorIn<char> for String<'a> {
    type Alloc = &'a Bump;
    fn from_iter_in<I>(iter: I, alloc: Self::Alloc) -> Self
    where
        I: IntoIterator<Item = char>,
    {
        String::from_iter_in(iter, alloc)
    }
}

/// Extension trait for iterators, in order to allow allocator-parameterized collections to be constructed more easily.
pub trait CollectIn: Iterator + Sized {
    /// Collect all items from an iterator, into a collection parameterized by an allocator.
    /// Similar to `Iterator::collect::<C>()`.
    ///
    /// ```rust
    /// #[cfg(feature = "collections")]
    /// # use bumpalo::collections::{FromIteratorIn, CollectIn, Vec, String};
    /// # use bumpalo::Bump;
    ///
    /// let bump = Bump::new();
    ///
    /// let str = "hello, world!".to_owned();
    /// let bump_str: String = str.chars().collect_in(&bump);
    /// assert_eq!(&bump_str, &str);
    ///
    /// let nums: Vec<i32> = (0..=3).collect_in::<Vec<_>>(&bump);
    /// assert_eq!(&nums, &[0,1,2,3]);
    /// ```
    fn collect_in<C: FromIteratorIn<Self::Item>>(self, alloc: C::Alloc) -> C {
        C::from_iter_in(self, alloc)
    }
}

impl<I: Iterator> CollectIn for I {}
