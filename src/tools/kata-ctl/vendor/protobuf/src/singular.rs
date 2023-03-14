use std::default::Default;
use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
use std::mem;
use std::option;

#[cfg(feature = "with-serde")]
use serde;

use crate::clear::Clear;

/// Like `Option<T>`, but keeps the actual element on `clear`.
pub struct SingularField<T> {
    value: T,
    set: bool,
}

/// Like `Option<Box<T>>`, but keeps the actual element on `clear`.
pub struct SingularPtrField<T> {
    value: Option<Box<T>>,
    set: bool,
}

impl<T> SingularField<T> {
    /// Construct this object from given value.
    #[inline]
    pub fn some(value: T) -> SingularField<T> {
        SingularField {
            value: value,
            set: true,
        }
    }

    /// True iff this object contains data.
    #[inline]
    pub fn is_some(&self) -> bool {
        self.set
    }

    /// True iff this object contains no data.
    #[inline]
    pub fn is_none(&self) -> bool {
        !self.is_some()
    }

    /// Convert this object into `Option`.
    #[inline]
    pub fn into_option(self) -> Option<T> {
        if self.set {
            Some(self.value)
        } else {
            None
        }
    }

    /// View data as `Option`.
    #[inline]
    pub fn as_ref<'a>(&'a self) -> Option<&'a T> {
        if self.set {
            Some(&self.value)
        } else {
            None
        }
    }

    /// View data as mutable `Option`.
    #[inline]
    pub fn as_mut<'a>(&'a mut self) -> Option<&'a mut T> {
        if self.set {
            Some(&mut self.value)
        } else {
            None
        }
    }

    /// Unwrap data as reference.
    #[inline]
    pub fn unwrap_ref<'a>(&'a self) -> &'a T {
        self.as_ref().unwrap()
    }

    /// Unwrap data as mutable reference.
    #[inline]
    pub fn unwrap_mut_ref<'a>(&'a mut self) -> &'a mut T {
        self.as_mut().unwrap()
    }

    /// Unwrap data, panic if not set.
    #[inline]
    pub fn unwrap(self) -> T {
        if self.set {
            self.value
        } else {
            panic!();
        }
    }

    /// Unwrap data or return given default value.
    #[inline]
    pub fn unwrap_or(self, def: T) -> T {
        if self.set {
            self.value
        } else {
            def
        }
    }

    /// Unwrap data or return given default value.
    #[inline]
    pub fn unwrap_or_else<F>(self, f: F) -> T
    where
        F: FnOnce() -> T,
    {
        if self.set {
            self.value
        } else {
            f()
        }
    }

    /// Apply a function to contained element and store result in new `SingularPtrField`.
    #[inline]
    pub fn map<U, F>(self, f: F) -> SingularPtrField<U>
    where
        F: FnOnce(T) -> U,
    {
        SingularPtrField::from_option(self.into_option().map(f))
    }

    /// View as iterator over references.
    #[inline]
    pub fn iter<'a>(&'a self) -> option::IntoIter<&'a T> {
        self.as_ref().into_iter()
    }

    /// View as iterator over mutable references.
    #[inline]
    pub fn mut_iter<'a>(&'a mut self) -> option::IntoIter<&'a mut T> {
        self.as_mut().into_iter()
    }

    /// Clear this object.
    /// Note, contained object destructor is not called, so allocated memory could be reused.
    #[inline]
    pub fn clear(&mut self) {
        self.set = false;
    }
}

impl<T: Default> SingularField<T> {
    /// Construct a `SingularField` with no data.
    #[inline]
    pub fn none() -> SingularField<T> {
        SingularField {
            value: Default::default(),
            set: false,
        }
    }

    /// Construct `SingularField` from `Option`.
    #[inline]
    pub fn from_option(option: Option<T>) -> SingularField<T> {
        match option {
            Some(x) => SingularField::some(x),
            None => SingularField::none(),
        }
    }

    /// Return data as option, clear this object.
    #[inline]
    pub fn take(&mut self) -> Option<T> {
        if self.set {
            self.set = false;
            Some(mem::replace(&mut self.value, Default::default()))
        } else {
            None
        }
    }
}

impl<T> SingularPtrField<T> {
    /// Construct `SingularPtrField` from given object.
    #[inline]
    pub fn some(value: T) -> SingularPtrField<T> {
        SingularPtrField {
            value: Some(Box::new(value)),
            set: true,
        }
    }

    /// Construct an empty `SingularPtrField`.
    #[inline]
    pub fn none() -> SingularPtrField<T> {
        SingularPtrField {
            value: None,
            set: false,
        }
    }

    /// Construct `SingularPtrField` from optional.
    #[inline]
    pub fn from_option(option: Option<T>) -> SingularPtrField<T> {
        match option {
            Some(x) => SingularPtrField::some(x),
            None => SingularPtrField::none(),
        }
    }

    /// True iff this object contains data.
    #[inline]
    pub fn is_some(&self) -> bool {
        self.set
    }

    /// True iff this object contains no data.
    #[inline]
    pub fn is_none(&self) -> bool {
        !self.is_some()
    }

    /// Convert into `Option<T>`.
    #[inline]
    pub fn into_option(self) -> Option<T> {
        if self.set {
            Some(*self.value.unwrap())
        } else {
            None
        }
    }

    /// View data as reference option.
    #[inline]
    pub fn as_ref<'a>(&'a self) -> Option<&'a T> {
        if self.set {
            Some(&**self.value.as_ref().unwrap())
        } else {
            None
        }
    }

    /// View data as mutable reference option.
    #[inline]
    pub fn as_mut<'a>(&'a mut self) -> Option<&'a mut T> {
        if self.set {
            Some(&mut **self.value.as_mut().unwrap())
        } else {
            None
        }
    }

    /// Get data as reference.
    /// Panics if empty.
    #[inline]
    pub fn get_ref<'a>(&'a self) -> &'a T {
        self.as_ref().unwrap()
    }

    /// Get data as mutable reference.
    /// Panics if empty.
    #[inline]
    pub fn get_mut_ref<'a>(&'a mut self) -> &'a mut T {
        self.as_mut().unwrap()
    }

    /// Take the data.
    /// Panics if empty
    #[inline]
    pub fn unwrap(self) -> T {
        if self.set {
            *self.value.unwrap()
        } else {
            panic!();
        }
    }

    /// Take the data or return supplied default element if empty.
    #[inline]
    pub fn unwrap_or(self, def: T) -> T {
        if self.set {
            *self.value.unwrap()
        } else {
            def
        }
    }

    /// Take the data or return supplied default element if empty.
    #[inline]
    pub fn unwrap_or_else<F>(self, f: F) -> T
    where
        F: FnOnce() -> T,
    {
        if self.set {
            *self.value.unwrap()
        } else {
            f()
        }
    }

    /// Apply given function to contained data to construct another `SingularPtrField`.
    /// Returns empty `SingularPtrField` if this object is empty.
    #[inline]
    pub fn map<U, F>(self, f: F) -> SingularPtrField<U>
    where
        F: FnOnce(T) -> U,
    {
        SingularPtrField::from_option(self.into_option().map(f))
    }

    /// View data as iterator.
    #[inline]
    pub fn iter<'a>(&'a self) -> option::IntoIter<&'a T> {
        self.as_ref().into_iter()
    }

    /// View data as mutable iterator.
    #[inline]
    pub fn mut_iter<'a>(&'a mut self) -> option::IntoIter<&'a mut T> {
        self.as_mut().into_iter()
    }

    /// Take data as option, leaving this object empty.
    #[inline]
    pub fn take(&mut self) -> Option<T> {
        if self.set {
            self.set = false;
            Some(*self.value.take().unwrap())
        } else {
            None
        }
    }

    /// Clear this object, but do not call destructor of underlying data.
    #[inline]
    pub fn clear(&mut self) {
        self.set = false;
    }
}

impl<T: Default + Clear> SingularField<T> {
    /// Get contained data, consume self. Return default value for type if this is empty.
    #[inline]
    pub fn unwrap_or_default(mut self) -> T {
        if !self.set {
            self.value.clear();
        }
        self.value
    }

    /// Initialize this object with default value.
    /// This operation can be more efficient then construction of clear element,
    /// because it may reuse previously contained object.
    #[inline]
    pub fn set_default<'a>(&'a mut self) -> &'a mut T {
        self.set = true;
        self.value.clear();
        &mut self.value
    }
}

impl<T: Default + Clear> SingularPtrField<T> {
    /// Get contained data, consume self. Return default value for type if this is empty.
    #[inline]
    pub fn unwrap_or_default(mut self) -> T {
        if self.set {
            self.unwrap()
        } else if self.value.is_some() {
            self.value.clear();
            *self.value.unwrap()
        } else {
            Default::default()
        }
    }

    /// Initialize this object with default value.
    /// This operation can be more efficient then construction of clear element,
    /// because it may reuse previously contained object.
    #[inline]
    pub fn set_default<'a>(&'a mut self) -> &'a mut T {
        self.set = true;
        if self.value.is_some() {
            self.value.as_mut().unwrap().clear();
        } else {
            self.value = Some(Default::default());
        }
        self.as_mut().unwrap()
    }
}

impl<T: Default> Default for SingularField<T> {
    #[inline]
    fn default() -> SingularField<T> {
        SingularField::none()
    }
}

impl<T> Default for SingularPtrField<T> {
    #[inline]
    fn default() -> SingularPtrField<T> {
        SingularPtrField::none()
    }
}

impl<T: Default> From<Option<T>> for SingularField<T> {
    fn from(o: Option<T>) -> Self {
        SingularField::from_option(o)
    }
}

impl<T> From<Option<T>> for SingularPtrField<T> {
    fn from(o: Option<T>) -> Self {
        SingularPtrField::from_option(o)
    }
}

impl<T: Clone + Default> Clone for SingularField<T> {
    #[inline]
    fn clone(&self) -> SingularField<T> {
        if self.set {
            SingularField::some(self.value.clone())
        } else {
            SingularField::none()
        }
    }
}

impl<T: Clone> Clone for SingularPtrField<T> {
    #[inline]
    fn clone(&self) -> SingularPtrField<T> {
        if self.set {
            SingularPtrField::some(self.as_ref().unwrap().clone())
        } else {
            SingularPtrField::none()
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for SingularField<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_some() {
            write!(f, "Some({:?})", *self.as_ref().unwrap())
        } else {
            write!(f, "None")
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for SingularPtrField<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_some() {
            write!(f, "Some({:?})", *self.as_ref().unwrap())
        } else {
            write!(f, "None")
        }
    }
}

impl<T: PartialEq> PartialEq for SingularField<T> {
    #[inline]
    fn eq(&self, other: &SingularField<T>) -> bool {
        self.as_ref() == other.as_ref()
    }
}

impl<T: Eq> Eq for SingularField<T> {}

impl<T: PartialEq> PartialEq for SingularPtrField<T> {
    #[inline]
    fn eq(&self, other: &SingularPtrField<T>) -> bool {
        self.as_ref() == other.as_ref()
    }
}

impl<T: Eq> Eq for SingularPtrField<T> {}

impl<T: Hash> Hash for SingularField<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_ref().hash(state);
    }
}

impl<T: Hash> Hash for SingularPtrField<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_ref().hash(state);
    }
}

impl<'a, T> IntoIterator for &'a SingularField<T> {
    type Item = &'a T;
    type IntoIter = option::IntoIter<&'a T>;

    fn into_iter(self) -> option::IntoIter<&'a T> {
        self.iter()
    }
}

impl<'a, T> IntoIterator for &'a SingularPtrField<T> {
    type Item = &'a T;
    type IntoIter = option::IntoIter<&'a T>;

    fn into_iter(self) -> option::IntoIter<&'a T> {
        self.iter()
    }
}

#[cfg(feature = "with-serde")]
impl<T: serde::Serialize> serde::Serialize for SingularPtrField<T> {
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> Result<<S as serde::Serializer>::Ok, <S as serde::Serializer>::Error>
    where
        S: serde::Serializer,
    {
        self.as_ref().serialize(serializer)
    }
}

#[cfg(feature = "with-serde")]
impl<T: serde::Serialize> serde::Serialize for SingularField<T> {
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> Result<<S as serde::Serializer>::Ok, <S as serde::Serializer>::Error>
    where
        S: serde::Serializer,
    {
        self.as_ref().serialize(serializer)
    }
}

#[cfg(feature = "with-serde")]
impl<'de, T: serde::Deserialize<'de>> serde::Deserialize<'de> for SingularPtrField<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as serde::Deserializer<'de>>::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Option::deserialize(deserializer).map(SingularPtrField::from_option)
    }
}

#[cfg(feature = "with-serde")]
impl<'de, T: serde::Deserialize<'de> + Default> serde::Deserialize<'de> for SingularField<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as serde::Deserializer<'de>>::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Option::deserialize(deserializer).map(SingularField::from_option)
    }
}

#[cfg(test)]
mod test {
    use super::SingularField;
    use crate::clear::Clear;

    #[test]
    fn test_set_default_clears() {
        #[derive(Default)]
        struct Foo {
            b: isize,
        }

        impl Clear for Foo {
            fn clear(&mut self) {
                self.b = 0;
            }
        }

        let mut x = SingularField::some(Foo { b: 10 });
        x.clear();
        x.set_default();
        assert_eq!(0, x.as_ref().unwrap().b);

        x.as_mut().unwrap().b = 11;
        // without clear
        x.set_default();
        assert_eq!(0, x.as_ref().unwrap().b);
    }

    #[test]
    fn unwrap_or_default() {
        assert_eq!(
            "abc",
            SingularField::some("abc".to_owned()).unwrap_or_default()
        );
        assert_eq!("", SingularField::<String>::none().unwrap_or_default());
        let mut some = SingularField::some("abc".to_owned());
        some.clear();
        assert_eq!("", some.unwrap_or_default());
    }
}
