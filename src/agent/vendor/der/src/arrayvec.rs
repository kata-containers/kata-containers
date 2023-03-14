//! Array-backed append-only vector type.
// TODO(tarcieri): use `core` impl of `ArrayVec`
// See: https://github.com/rust-lang/rfcs/pull/2990

use crate::{ErrorKind, Result};

/// Array-backed append-only vector type.
#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub(crate) struct ArrayVec<T, const N: usize> {
    /// Elements of the set.
    elements: [Option<T>; N],

    /// Last populated element.
    length: usize,
}

impl<T, const N: usize> ArrayVec<T, N> {
    /// Create a new [`ArrayVec`].
    pub fn new() -> Self {
        Self {
            elements: [(); N].map(|_| None),
            length: 0,
        }
    }

    /// Add an element to this [`ArrayVec`].
    ///
    /// Items MUST be added in lexicographical order according to the `Ord`
    /// impl on `T`.
    pub fn add(&mut self, element: T) -> Result<()> {
        match self.length.checked_add(1) {
            Some(n) if n < N => {
                self.elements[self.length] = Some(element);
                self.length = n;
                Ok(())
            }
            _ => Err(ErrorKind::Overlength.into()),
        }
    }

    /// Get an element from this [`ArrayVec`].
    pub fn get(&self, index: usize) -> Option<&T> {
        match self.elements.get(index) {
            Some(Some(ref item)) => Some(item),
            _ => None,
        }
    }

    /// Iterate over the elements in this [`ArrayVec`].
    pub fn iter(&self) -> Iter<'_, T> {
        Iter::new(&self.elements)
    }

    /// Is this [`ArrayVec`] empty?
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Get the number of elements in this [`ArrayVec`].
    pub fn len(&self) -> usize {
        self.length
    }

    /// Get the last item from this [`ArrayVec`].
    pub fn last(&self) -> Option<&T> {
        self.length.checked_sub(1).and_then(|n| self.get(n))
    }

    /// Try to convert this [`ArrayVec`] into a `[T; N]`.
    ///
    /// Returns `None` if the [`ArrayVec`] does not contain `N` elements.
    pub fn try_into_array(self) -> Result<[T; N]> {
        if self.length != N {
            return Err(ErrorKind::Incomplete {
                expected_len: N.try_into()?,
                actual_len: self.length.try_into()?,
            }
            .into());
        }

        Ok(self.elements.map(|elem| match elem {
            Some(e) => e,
            None => unreachable!(),
        }))
    }
}

impl<T, const N: usize> Default for ArrayVec<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

/// Iterator over the elements of an [`ArrayVec`].
#[derive(Clone, Debug)]
pub struct Iter<'a, T> {
    /// Decoder which iterates over the elements of the message.
    elements: &'a [Option<T>],

    /// Position within the iterator.
    position: usize,
}

impl<'a, T> Iter<'a, T> {
    pub(crate) fn new(elements: &'a [Option<T>]) -> Self {
        Self {
            elements,
            position: 0,
        }
    }
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        if let Some(Some(res)) = self.elements.get(self.position) {
            self.position = self.position.checked_add(1)?;
            Some(res)
        } else {
            None
        }
    }
}
