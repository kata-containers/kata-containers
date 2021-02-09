//! Lazy value initialization.

use std::fmt;

use super::internal::{Erased, Inner, Visitor};
use super::{Error, Value};

impl<'v> Value<'v> {
    /// Get a value from a fillable slot.
    pub fn from_fill<T>(value: &'v T) -> Self
    where
        T: Fill + 'static,
    {
        Value {
            inner: Inner::Fill(unsafe { Erased::new_unchecked::<T>(value) }),
        }
    }
}

/// A type that requires extra work to convert into a [`Value`](struct.Value.html).
///
/// This trait is a more advanced initialization API than [`ToValue`](trait.ToValue.html).
/// It's intended for erased values coming from other logging frameworks that may need
/// to perform extra work to determine the concrete type to use.
pub trait Fill {
    /// Fill a value.
    fn fill(&self, slot: &mut Slot) -> Result<(), Error>;
}

impl<'a, T> Fill for &'a T
where
    T: Fill + ?Sized,
{
    fn fill(&self, slot: &mut Slot) -> Result<(), Error> {
        (**self).fill(slot)
    }
}

/// A value slot to fill using the [`Fill`](trait.Fill.html) trait.
pub struct Slot<'s, 'f> {
    filled: bool,
    visitor: &'s mut dyn Visitor<'f>,
}

impl<'s, 'f> fmt::Debug for Slot<'s, 'f> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Slot").finish()
    }
}

impl<'s, 'f> Slot<'s, 'f> {
    pub(super) fn new(visitor: &'s mut dyn Visitor<'f>) -> Self {
        Slot {
            visitor,
            filled: false,
        }
    }

    pub(super) fn fill<F>(&mut self, f: F) -> Result<(), Error>
    where
        F: FnOnce(&mut dyn Visitor<'f>) -> Result<(), Error>,
    {
        assert!(!self.filled, "the slot has already been filled");
        self.filled = true;

        f(self.visitor)
    }

    /// Fill the slot with a value.
    ///
    /// The given value doesn't need to satisfy any particular lifetime constraints.
    ///
    /// # Panics
    ///
    /// Calling more than a single `fill` method on this slot will panic.
    pub fn fill_any<T>(&mut self, value: T) -> Result<(), Error>
    where
        T: Into<Value<'f>>,
    {
        self.fill(|visitor| value.into().inner.visit(visitor))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_value_borrowed() {
        struct TestFill;

        impl Fill for TestFill {
            fn fill(&self, slot: &mut Slot) -> Result<(), Error> {
                let dbg: &dyn fmt::Debug = &1;

                slot.fill_debug(&dbg)
            }
        }

        assert_eq!("1", Value::from_fill(&TestFill).to_string());
    }

    #[test]
    fn fill_value_owned() {
        struct TestFill;

        impl Fill for TestFill {
            fn fill(&self, slot: &mut Slot) -> Result<(), Error> {
                slot.fill_any("a string")
            }
        }
    }

    #[test]
    #[should_panic]
    fn fill_multiple_times_panics() {
        struct BadFill;

        impl Fill for BadFill {
            fn fill(&self, slot: &mut Slot) -> Result<(), Error> {
                slot.fill_any(42)?;
                slot.fill_any(6789)?;

                Ok(())
            }
        }

        let _ = Value::from_fill(&BadFill).to_string();
    }

    #[test]
    fn fill_cast() {
        struct TestFill;

        impl Fill for TestFill {
            fn fill(&self, slot: &mut Slot) -> Result<(), Error> {
                slot.fill_any("a string")
            }
        }

        assert_eq!(
            "a string",
            Value::from_fill(&TestFill)
                .to_borrowed_str()
                .expect("invalid value")
        );
    }

    #[test]
    fn fill_debug() {
        struct TestFill;

        impl Fill for TestFill {
            fn fill(&self, slot: &mut Slot) -> Result<(), Error> {
                slot.fill_any(42u64)
            }
        }

        assert_eq!(
            format!("{:04?}", 42u64),
            format!("{:04?}", Value::from_fill(&TestFill)),
        )
    }
}
