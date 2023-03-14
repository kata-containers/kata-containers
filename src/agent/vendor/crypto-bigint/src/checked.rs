//! Checked arithmetic.

use subtle::{Choice, ConditionallySelectable, ConstantTimeEq, CtOption};

/// Provides intentionally-checked arithmetic on `T`.
///
/// Internally this leverages the [`CtOption`] type from the [`subtle`] crate
/// in order to handle overflows in constant time.
#[derive(Copy, Clone, Debug)]
pub struct Checked<T>(pub CtOption<T>);

impl<T> Checked<T> {
    /// Create a new checked arithmetic wrapper for the given value.
    pub fn new(val: T) -> Self {
        Self(CtOption::new(val, Choice::from(1)))
    }
}

impl<T> Default for Checked<T>
where
    T: Default,
{
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: ConditionallySelectable> ConditionallySelectable for Checked<T> {
    #[inline]
    fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self {
        Self(CtOption::conditional_select(&a.0, &b.0, choice))
    }
}

impl<T: ConstantTimeEq> ConstantTimeEq for Checked<T> {
    #[inline]
    fn ct_eq(&self, rhs: &Self) -> Choice {
        self.0.ct_eq(&rhs.0)
    }
}

impl<T> From<Checked<T>> for CtOption<T> {
    fn from(checked: Checked<T>) -> CtOption<T> {
        checked.0
    }
}

impl<T> From<CtOption<T>> for Checked<T> {
    fn from(ct_option: CtOption<T>) -> Checked<T> {
        Checked(ct_option)
    }
}

impl<T> From<Checked<T>> for Option<T> {
    fn from(checked: Checked<T>) -> Option<T> {
        checked.0.into()
    }
}
