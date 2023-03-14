// Copyright 2019 Amazon.com, Inc. or its affiliates. All Rights Reserved.
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
//
// SPDX-License-Identifier: (Apache-2.0 AND BSD-3-Clause)

//! Trait and wrapper for working with C defined FAM structures.
//!
//! In C 99 an array of unknown size may appear within a struct definition as the last member
//! (as long as there is at least one other named member).
//! This is known as a flexible array member (FAM).
//! Pre C99, the same behavior could be achieved using zero length arrays.
//!
//! Flexible Array Members are the go-to choice for working with large amounts of data
//! prefixed by header values.
//!
//! For example the KVM API has many structures of this kind.

#[cfg(feature = "with-serde")]
use serde::de::{self, Deserialize, Deserializer, SeqAccess, Visitor};
#[cfg(feature = "with-serde")]
use serde::{ser::SerializeTuple, Serialize, Serializer};
#[cfg(feature = "with-serde")]
use std::fmt;
#[cfg(feature = "with-serde")]
use std::marker::PhantomData;
use std::mem::{self, size_of};

/// Errors associated with the [`FamStructWrapper`](struct.FamStructWrapper.html) struct.
#[derive(Clone, Debug)]
pub enum Error {
    /// The max size has been exceeded
    SizeLimitExceeded,
}

/// Trait for accessing properties of C defined FAM structures.
///
/// # Safety
///
/// This is unsafe due to the number of constraints that aren't checked:
/// * the implementer should be a POD
/// * the implementor should contain a flexible array member of elements of type `Entry`
/// * `Entry` should be a POD
///
/// Violating these may cause problems.
///
/// # Example
///
/// ```
/// use vmm_sys_util::fam::*;
///
/// #[repr(C)]
/// #[derive(Default)]
/// pub struct __IncompleteArrayField<T>(::std::marker::PhantomData<T>, [T; 0]);
/// impl<T> __IncompleteArrayField<T> {
///     #[inline]
///     pub fn new() -> Self {
///         __IncompleteArrayField(::std::marker::PhantomData, [])
///     }
///     #[inline]
///     pub unsafe fn as_ptr(&self) -> *const T {
///         ::std::mem::transmute(self)
///     }
///     #[inline]
///     pub unsafe fn as_mut_ptr(&mut self) -> *mut T {
///         ::std::mem::transmute(self)
///     }
///     #[inline]
///     pub unsafe fn as_slice(&self, len: usize) -> &[T] {
///         ::std::slice::from_raw_parts(self.as_ptr(), len)
///     }
///     #[inline]
///     pub unsafe fn as_mut_slice(&mut self, len: usize) -> &mut [T] {
///         ::std::slice::from_raw_parts_mut(self.as_mut_ptr(), len)
///     }
/// }
///
/// #[repr(C)]
/// #[derive(Default)]
/// struct MockFamStruct {
///     pub len: u32,
///     pub padding: u32,
///     pub entries: __IncompleteArrayField<u32>,
/// }
///
/// unsafe impl FamStruct for MockFamStruct {
///     type Entry = u32;
///
///     fn len(&self) -> usize {
///         self.len as usize
///     }
///
///     fn set_len(&mut self, len: usize) {
///         self.len = len as u32
///     }
///
///     fn max_len() -> usize {
///         100
///     }
///
///     fn as_slice(&self) -> &[u32] {
///         let len = self.len();
///         unsafe { self.entries.as_slice(len) }
///     }
///
///     fn as_mut_slice(&mut self) -> &mut [u32] {
///         let len = self.len();
///         unsafe { self.entries.as_mut_slice(len) }
///     }
/// }
///
/// type MockFamStructWrapper = FamStructWrapper<MockFamStruct>;
/// ```
#[allow(clippy::len_without_is_empty)]
pub unsafe trait FamStruct {
    /// The type of the FAM entries
    type Entry: PartialEq + Copy;

    /// Get the FAM length
    ///
    /// These type of structures contain a member that holds the FAM length.
    /// This method will return the value of that member.
    fn len(&self) -> usize;

    /// Set the FAM length
    ///
    /// These type of structures contain a member that holds the FAM length.
    /// This method will set the value of that member.
    fn set_len(&mut self, len: usize);

    /// Get max allowed FAM length
    ///
    /// This depends on each structure.
    /// For example a structure representing the cpuid can contain at most 80 entries.
    fn max_len() -> usize;

    /// Get the FAM entries as slice
    fn as_slice(&self) -> &[Self::Entry];

    /// Get the FAM entries as mut slice
    fn as_mut_slice(&mut self) -> &mut [Self::Entry];
}

/// A wrapper for [`FamStruct`](trait.FamStruct.html).
///
/// It helps in treating a [`FamStruct`](trait.FamStruct.html) similarly to an actual `Vec`.
#[derive(Debug)]
pub struct FamStructWrapper<T: Default + FamStruct> {
    // This variable holds the FamStruct structure. We use a `Vec<T>` to make the allocation
    // large enough while still being aligned for `T`. Only the first element of `Vec<T>`
    // will actually be used as a `T`. The remaining memory in the `Vec<T>` is for `entries`,
    // which must be contiguous. Since the entries are of type `FamStruct::Entry` we must
    // be careful to convert the desired capacity of the `FamStructWrapper`
    // from `FamStruct::Entry` to `T` when reserving or releasing memory.
    mem_allocator: Vec<T>,
}

impl<T: Default + FamStruct> FamStructWrapper<T> {
    /// Convert FAM len to `mem_allocator` len.
    ///
    /// Get the capacity required by mem_allocator in order to hold
    /// the provided number of [`FamStruct::Entry`](trait.FamStruct.html#associatedtype.Entry).
    fn mem_allocator_len(fam_len: usize) -> usize {
        let wrapper_size_in_bytes = size_of::<T>() + fam_len * size_of::<T::Entry>();
        (wrapper_size_in_bytes + size_of::<T>() - 1) / size_of::<T>()
    }

    /// Convert `mem_allocator` len to FAM len.
    ///
    /// Get the number of elements of type
    /// [`FamStruct::Entry`](trait.FamStruct.html#associatedtype.Entry)
    /// that fit in a mem_allocator of provided len.
    fn fam_len(mem_allocator_len: usize) -> usize {
        if mem_allocator_len == 0 {
            return 0;
        }

        let array_size_in_bytes = (mem_allocator_len - 1) * size_of::<T>();
        array_size_in_bytes / size_of::<T::Entry>()
    }

    /// Create a new FamStructWrapper with `num_elements` elements.
    ///
    /// The elements will be zero-initialized. The type of the elements will be
    /// [`FamStruct::Entry`](trait.FamStruct.html#associatedtype.Entry).
    ///
    /// # Arguments
    ///
    /// * `num_elements` - The number of elements in the FamStructWrapper.
    ///
    /// # Errors
    ///
    /// When `num_elements` is greater than the max possible len, it returns
    /// `Error::SizeLimitExceeded`.
    pub fn new(num_elements: usize) -> Result<FamStructWrapper<T>, Error> {
        if num_elements > T::max_len() {
            return Err(Error::SizeLimitExceeded);
        }
        let required_mem_allocator_capacity =
            FamStructWrapper::<T>::mem_allocator_len(num_elements);

        let mut mem_allocator = Vec::with_capacity(required_mem_allocator_capacity);
        mem_allocator.push(T::default());
        for _ in 1..required_mem_allocator_capacity {
            mem_allocator.push(unsafe { mem::zeroed() })
        }
        mem_allocator[0].set_len(num_elements);

        Ok(FamStructWrapper { mem_allocator })
    }

    /// Create a new FamStructWrapper from a slice of elements.
    ///
    /// # Arguments
    ///
    /// * `entries` - The slice of [`FamStruct::Entry`](trait.FamStruct.html#associatedtype.Entry)
    ///               entries.
    ///
    /// # Errors
    ///
    /// When the size of `entries` is greater than the max possible len, it returns
    /// `Error::SizeLimitExceeded`.
    pub fn from_entries(entries: &[T::Entry]) -> Result<FamStructWrapper<T>, Error> {
        let mut adapter = FamStructWrapper::<T>::new(entries.len())?;

        {
            let wrapper_entries = adapter.as_mut_fam_struct().as_mut_slice();
            wrapper_entries.copy_from_slice(entries);
        }

        Ok(adapter)
    }

    /// Create a new FamStructWrapper from the raw content represented as `Vec<T>`.
    ///
    /// Sometimes we already have the raw content of an FAM struct represented as `Vec<T>`,
    /// and want to use the FamStructWrapper as accessors.
    ///
    /// # Arguments
    ///
    /// * `content` - The raw content represented as `Vec[T]`.
    ///
    /// # Safety
    ///
    /// This function is unsafe because the caller needs to ensure that the raw content is
    /// correctly layed out.
    pub unsafe fn from_raw(content: Vec<T>) -> Self {
        FamStructWrapper {
            mem_allocator: content,
        }
    }

    /// Consume the FamStructWrapper and return the raw content as `Vec<T>`.
    pub fn into_raw(self) -> Vec<T> {
        self.mem_allocator
    }

    /// Get a reference to the actual [`FamStruct`](trait.FamStruct.html) instance.
    pub fn as_fam_struct_ref(&self) -> &T {
        &self.mem_allocator[0]
    }

    /// Get a mut reference to the actual [`FamStruct`](trait.FamStruct.html) instance.
    pub fn as_mut_fam_struct(&mut self) -> &mut T {
        &mut self.mem_allocator[0]
    }

    /// Get a pointer to the [`FamStruct`](trait.FamStruct.html) instance.
    ///
    /// The caller must ensure that the fam_struct outlives the pointer this
    /// function returns, or else it will end up pointing to garbage.
    ///
    /// Modifying the container referenced by this pointer may cause its buffer
    /// to be reallocated, which would also make any pointers to it invalid.
    pub fn as_fam_struct_ptr(&self) -> *const T {
        self.as_fam_struct_ref()
    }

    /// Get a mutable pointer to the [`FamStruct`](trait.FamStruct.html) instance.
    ///
    /// The caller must ensure that the fam_struct outlives the pointer this
    /// function returns, or else it will end up pointing to garbage.
    ///
    /// Modifying the container referenced by this pointer may cause its buffer
    /// to be reallocated, which would also make any pointers to it invalid.
    pub fn as_mut_fam_struct_ptr(&mut self) -> *mut T {
        self.as_mut_fam_struct()
    }

    /// Get the elements slice.
    pub fn as_slice(&self) -> &[T::Entry] {
        self.as_fam_struct_ref().as_slice()
    }

    /// Get the mutable elements slice.
    pub fn as_mut_slice(&mut self) -> &mut [T::Entry] {
        self.as_mut_fam_struct().as_mut_slice()
    }

    /// Get the number of elements of type `FamStruct::Entry` currently in the vec.
    fn len(&self) -> usize {
        self.as_fam_struct_ref().len()
    }

    /// Get the capacity of the `FamStructWrapper`
    ///
    /// The capacity is measured in elements of type `FamStruct::Entry`.
    fn capacity(&self) -> usize {
        FamStructWrapper::<T>::fam_len(self.mem_allocator.capacity())
    }

    /// Reserve additional capacity.
    ///
    /// Reserve capacity for at least `additional` more
    /// [`FamStruct::Entry`](trait.FamStruct.html#associatedtype.Entry) elements.
    ///
    /// If the capacity is already reserved, this method doesn't do anything.
    /// If not this will trigger a reallocation of the underlying buffer.
    fn reserve(&mut self, additional: usize) {
        let desired_capacity = self.len() + additional;
        if desired_capacity <= self.capacity() {
            return;
        }

        let current_mem_allocator_len = self.mem_allocator.len();
        let required_mem_allocator_len = FamStructWrapper::<T>::mem_allocator_len(desired_capacity);
        let additional_mem_allocator_len = required_mem_allocator_len - current_mem_allocator_len;

        self.mem_allocator.reserve(additional_mem_allocator_len);
    }

    /// Update the length of the FamStructWrapper.
    ///
    /// The length of `self` will be updated to the specified value.
    /// The length of the `T` structure and of `self.mem_allocator` will be updated accordingly.
    /// If the len is increased additional capacity will be reserved.
    /// If the len is decreased the unnecessary memory will be deallocated.
    ///
    /// This method might trigger reallocations of the underlying buffer.
    ///
    /// # Errors
    ///
    /// When len is greater than the max possible len it returns Error::SizeLimitExceeded.
    fn set_len(&mut self, len: usize) -> Result<(), Error> {
        let additional_elements: isize = len as isize - self.len() as isize;
        // If len == self.len there's nothing to do.
        if additional_elements == 0 {
            return Ok(());
        }

        // If the len needs to be increased:
        if additional_elements > 0 {
            // Check if the new len is valid.
            if len > T::max_len() {
                return Err(Error::SizeLimitExceeded);
            }
            // Reserve additional capacity.
            self.reserve(additional_elements as usize);
        }

        let current_mem_allocator_len = self.mem_allocator.len();
        let required_mem_allocator_len = FamStructWrapper::<T>::mem_allocator_len(len);
        // Update the len of the `mem_allocator`.
        // This is safe since enough capacity has been reserved.
        unsafe {
            self.mem_allocator.set_len(required_mem_allocator_len);
        }
        // Zero-initialize the additional elements if any.
        for i in current_mem_allocator_len..required_mem_allocator_len {
            self.mem_allocator[i] = unsafe { mem::zeroed() }
        }
        // Update the len of the underlying `FamStruct`.
        self.as_mut_fam_struct().set_len(len);

        // If the len needs to be decreased, deallocate unnecessary memory
        if additional_elements < 0 {
            self.mem_allocator.shrink_to_fit();
        }

        Ok(())
    }

    /// Append an element.
    ///
    /// # Arguments
    ///
    /// * `entry` - The element that will be appended to the end of the collection.
    ///
    /// # Errors
    ///
    /// When len is already equal to max possible len it returns Error::SizeLimitExceeded.
    pub fn push(&mut self, entry: T::Entry) -> Result<(), Error> {
        let new_len = self.len() + 1;
        self.set_len(new_len)?;
        self.as_mut_slice()[new_len - 1] = entry;

        Ok(())
    }

    /// Retain only the elements specified by the predicate.
    ///
    /// # Arguments
    ///
    /// * `f` - The function used to evaluate whether an entry will be kept or not.
    ///         When `f` returns `true` the entry is kept.
    pub fn retain<P>(&mut self, mut f: P)
    where
        P: FnMut(&T::Entry) -> bool,
    {
        let mut num_kept_entries = 0;
        {
            let entries = self.as_mut_slice();
            for entry_idx in 0..entries.len() {
                let keep = f(&entries[entry_idx]);
                if keep {
                    entries[num_kept_entries] = entries[entry_idx];
                    num_kept_entries += 1;
                }
            }
        }

        // This is safe since this method is not increasing the len
        self.set_len(num_kept_entries).expect("invalid length");
    }
}

impl<T: Default + FamStruct + PartialEq> PartialEq for FamStructWrapper<T> {
    fn eq(&self, other: &FamStructWrapper<T>) -> bool {
        self.as_fam_struct_ref() == other.as_fam_struct_ref() && self.as_slice() == other.as_slice()
    }
}

impl<T: Default + FamStruct> Clone for FamStructWrapper<T> {
    fn clone(&self) -> Self {
        // The number of entries (self.as_slice().len()) can't be > T::max_len() since `self` is a
        // valid `FamStructWrapper`.
        let required_mem_allocator_capacity =
            FamStructWrapper::<T>::mem_allocator_len(self.as_slice().len());

        let mut mem_allocator = Vec::with_capacity(required_mem_allocator_capacity);

        // This is safe as long as the requirements for the `FamStruct` trait to be safe are met
        // (the implementing type and the entries elements are POD, therefore `Copy`, so memory
        // safety can't be violated by the ownership of `fam_struct`). It is also safe because we're
        // trying to read a T from a `&T` that is pointing to a properly initialized and aligned T.
        unsafe {
            let fam_struct: T = std::ptr::read(self.as_fam_struct_ref());
            mem_allocator.push(fam_struct);
        }
        for _ in 1..required_mem_allocator_capacity {
            mem_allocator.push(unsafe { mem::zeroed() })
        }

        let mut adapter = FamStructWrapper { mem_allocator };
        {
            let wrapper_entries = adapter.as_mut_fam_struct().as_mut_slice();
            wrapper_entries.copy_from_slice(self.as_slice());
        }
        adapter
    }
}

impl<T: Default + FamStruct> From<Vec<T>> for FamStructWrapper<T> {
    fn from(vec: Vec<T>) -> Self {
        FamStructWrapper { mem_allocator: vec }
    }
}

#[cfg(feature = "with-serde")]
impl<T: Default + FamStruct + Serialize> Serialize for FamStructWrapper<T>
where
    <T as FamStruct>::Entry: serde::Serialize,
{
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut s = serializer.serialize_tuple(2)?;
        s.serialize_element(self.as_fam_struct_ref())?;
        s.serialize_element(self.as_slice())?;
        s.end()
    }
}

#[cfg(feature = "with-serde")]
impl<'de, T: Default + FamStruct + Deserialize<'de>> Deserialize<'de> for FamStructWrapper<T>
where
    <T as FamStruct>::Entry: std::marker::Copy + serde::Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct FamStructWrapperVisitor<X> {
            dummy: PhantomData<X>,
        }

        impl<'de, X: Default + FamStruct + Deserialize<'de>> Visitor<'de> for FamStructWrapperVisitor<X>
        where
            <X as FamStruct>::Entry: std::marker::Copy + serde::Deserialize<'de>,
        {
            type Value = FamStructWrapper<X>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("FamStructWrapper")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<FamStructWrapper<X>, V::Error>
            where
                V: SeqAccess<'de>,
            {
                use serde::de::Error;

                let header = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let entries: Vec<X::Entry> = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;

                let mut result: Self::Value = FamStructWrapper::from_entries(entries.as_slice())
                    .map_err(|e| V::Error::custom(format!("{:?}", e)))?;
                result.mem_allocator[0] = header;
                Ok(result)
            }
        }

        deserializer.deserialize_tuple(2, FamStructWrapperVisitor { dummy: PhantomData })
    }
}

/// Generate `FamStruct` implementation for structs with flexible array member.
#[macro_export]
macro_rules! generate_fam_struct_impl {
    ($struct_type: ty, $entry_type: ty, $entries_name: ident,
     $field_type: ty, $field_name: ident, $max: expr) => {
        unsafe impl FamStruct for $struct_type {
            type Entry = $entry_type;

            fn len(&self) -> usize {
                self.$field_name as usize
            }

            fn set_len(&mut self, len: usize) {
                self.$field_name = len as $field_type;
            }

            fn max_len() -> usize {
                $max
            }

            fn as_slice(&self) -> &[<Self as FamStruct>::Entry] {
                let len = self.len();
                unsafe { self.$entries_name.as_slice(len) }
            }

            fn as_mut_slice(&mut self) -> &mut [<Self as FamStruct>::Entry] {
                let len = self.len();
                unsafe { self.$entries_name.as_mut_slice(len) }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "with-serde")]
    use serde_derive::{Deserialize, Serialize};

    use super::*;

    const MAX_LEN: usize = 100;

    #[repr(C)]
    #[derive(Default, PartialEq)]
    pub struct __IncompleteArrayField<T>(::std::marker::PhantomData<T>, [T; 0]);
    impl<T> __IncompleteArrayField<T> {
        #[inline]
        pub fn new() -> Self {
            __IncompleteArrayField(::std::marker::PhantomData, [])
        }
        #[inline]
        pub unsafe fn as_ptr(&self) -> *const T {
            ::std::mem::transmute(self)
        }
        #[inline]
        pub unsafe fn as_mut_ptr(&mut self) -> *mut T {
            ::std::mem::transmute(self)
        }
        #[inline]
        pub unsafe fn as_slice(&self, len: usize) -> &[T] {
            ::std::slice::from_raw_parts(self.as_ptr(), len)
        }
        #[inline]
        pub unsafe fn as_mut_slice(&mut self, len: usize) -> &mut [T] {
            ::std::slice::from_raw_parts_mut(self.as_mut_ptr(), len)
        }
    }

    #[cfg(feature = "with-serde")]
    impl<T> Serialize for __IncompleteArrayField<T> {
        fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            [0u8; 0].serialize(serializer)
        }
    }

    #[cfg(feature = "with-serde")]
    impl<'de, T> Deserialize<'de> for __IncompleteArrayField<T> {
        fn deserialize<D>(_: D) -> std::result::Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            Ok(__IncompleteArrayField::new())
        }
    }

    #[repr(C)]
    #[derive(Default, PartialEq)]
    struct MockFamStruct {
        pub len: u32,
        pub padding: u32,
        pub entries: __IncompleteArrayField<u32>,
    }

    generate_fam_struct_impl!(MockFamStruct, u32, entries, u32, len, 100);

    type MockFamStructWrapper = FamStructWrapper<MockFamStruct>;

    const ENTRIES_OFFSET: usize = 2;

    const FAM_LEN_TO_MEM_ALLOCATOR_LEN: &[(usize, usize)] = &[
        (0, 1),
        (1, 2),
        (2, 2),
        (3, 3),
        (4, 3),
        (5, 4),
        (10, 6),
        (50, 26),
        (100, 51),
    ];

    const MEM_ALLOCATOR_LEN_TO_FAM_LEN: &[(usize, usize)] = &[
        (0, 0),
        (1, 0),
        (2, 2),
        (3, 4),
        (4, 6),
        (5, 8),
        (10, 18),
        (50, 98),
        (100, 198),
    ];

    #[test]
    fn test_mem_allocator_len() {
        for pair in FAM_LEN_TO_MEM_ALLOCATOR_LEN {
            let fam_len = pair.0;
            let mem_allocator_len = pair.1;
            assert_eq!(
                mem_allocator_len,
                MockFamStructWrapper::mem_allocator_len(fam_len)
            );
        }
    }

    #[test]
    fn test_wrapper_len() {
        for pair in MEM_ALLOCATOR_LEN_TO_FAM_LEN {
            let mem_allocator_len = pair.0;
            let fam_len = pair.1;
            assert_eq!(fam_len, MockFamStructWrapper::fam_len(mem_allocator_len));
        }
    }

    #[test]
    fn test_new() {
        let num_entries = 10;

        let adapter = MockFamStructWrapper::new(num_entries).unwrap();
        assert_eq!(num_entries, adapter.capacity());

        let u32_slice = unsafe {
            std::slice::from_raw_parts(
                adapter.as_fam_struct_ptr() as *const u32,
                num_entries + ENTRIES_OFFSET,
            )
        };
        assert_eq!(num_entries, u32_slice[0] as usize);
        for entry in u32_slice[1..].iter() {
            assert_eq!(*entry, 0);
        }

        // It's okay to create a `FamStructWrapper` with the maximum allowed number of entries.
        let adapter = MockFamStructWrapper::new(MockFamStruct::max_len()).unwrap();
        assert_eq!(MockFamStruct::max_len(), adapter.capacity());

        assert!(matches!(
            MockFamStructWrapper::new(MockFamStruct::max_len() + 1),
            Err(Error::SizeLimitExceeded)
        ));
    }

    #[test]
    fn test_from_entries() {
        let num_entries: usize = 10;

        let mut entries = Vec::new();
        for i in 0..num_entries {
            entries.push(i as u32);
        }

        let adapter = MockFamStructWrapper::from_entries(entries.as_slice()).unwrap();
        let u32_slice = unsafe {
            std::slice::from_raw_parts(
                adapter.as_fam_struct_ptr() as *const u32,
                num_entries + ENTRIES_OFFSET,
            )
        };
        assert_eq!(num_entries, u32_slice[0] as usize);
        for (i, &value) in entries.iter().enumerate().take(num_entries) {
            assert_eq!(adapter.as_slice()[i], value);
        }

        let mut entries = Vec::new();
        for i in 0..MockFamStruct::max_len() + 1 {
            entries.push(i as u32);
        }

        // Can't create a `FamStructWrapper` with a number of entries > MockFamStruct::max_len().
        assert!(matches!(
            MockFamStructWrapper::from_entries(entries.as_slice()),
            Err(Error::SizeLimitExceeded)
        ));
    }

    #[test]
    fn test_entries_slice() {
        let num_entries = 10;
        let mut adapter = MockFamStructWrapper::new(num_entries).unwrap();

        let expected_slice = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9];

        {
            let mut_entries_slice = adapter.as_mut_slice();
            mut_entries_slice.copy_from_slice(expected_slice);
        }

        let u32_slice = unsafe {
            std::slice::from_raw_parts(
                adapter.as_fam_struct_ptr() as *const u32,
                num_entries + ENTRIES_OFFSET,
            )
        };
        assert_eq!(expected_slice, &u32_slice[ENTRIES_OFFSET..]);
        assert_eq!(expected_slice, adapter.as_slice());
    }

    #[test]
    fn test_reserve() {
        let mut adapter = MockFamStructWrapper::new(0).unwrap();

        // test that the right capacity is reserved
        for pair in FAM_LEN_TO_MEM_ALLOCATOR_LEN {
            let num_elements = pair.0;
            let required_mem_allocator_len = pair.1;

            adapter.reserve(num_elements);

            assert!(adapter.mem_allocator.capacity() >= required_mem_allocator_len);
            assert_eq!(0, adapter.len());
            assert!(adapter.capacity() >= num_elements);
        }

        // test that when the capacity is already reserved, the method doesn't do anything
        let current_capacity = adapter.capacity();
        adapter.reserve(current_capacity - 1);
        assert_eq!(current_capacity, adapter.capacity());
    }

    #[test]
    fn test_set_len() {
        let mut desired_len = 0;
        let mut adapter = MockFamStructWrapper::new(desired_len).unwrap();

        // keep initial len
        assert!(adapter.set_len(desired_len).is_ok());
        assert_eq!(adapter.len(), desired_len);

        // increase len
        desired_len = 10;
        assert!(adapter.set_len(desired_len).is_ok());
        // check that the len has been increased and zero-initialized elements have been added
        assert_eq!(adapter.len(), desired_len);
        for element in adapter.as_slice() {
            assert_eq!(*element, 0_u32);
        }

        // decrease len
        desired_len = 5;
        assert!(adapter.set_len(desired_len).is_ok());
        assert_eq!(adapter.len(), desired_len);
    }

    #[test]
    fn test_push() {
        let mut adapter = MockFamStructWrapper::new(0).unwrap();

        for i in 0..MAX_LEN {
            assert!(adapter.push(i as u32).is_ok());
            assert_eq!(adapter.as_slice()[i], i as u32);
            assert_eq!(adapter.len(), i + 1);
            assert!(
                adapter.mem_allocator.capacity() >= MockFamStructWrapper::mem_allocator_len(i + 1)
            );
        }

        assert!(adapter.push(0).is_err());
    }

    #[test]
    fn test_retain() {
        let mut adapter = MockFamStructWrapper::new(0).unwrap();

        let mut num_retained_entries = 0;
        for i in 0..MAX_LEN {
            assert!(adapter.push(i as u32).is_ok());
            if i % 2 == 0 {
                num_retained_entries += 1;
            }
        }

        adapter.retain(|entry| entry % 2 == 0);

        for entry in adapter.as_slice().iter() {
            assert_eq!(0, entry % 2);
        }
        assert_eq!(adapter.len(), num_retained_entries);
        assert!(
            adapter.mem_allocator.capacity()
                >= MockFamStructWrapper::mem_allocator_len(num_retained_entries)
        );
    }

    #[test]
    fn test_partial_eq() {
        let mut wrapper_1 = MockFamStructWrapper::new(0).unwrap();
        let mut wrapper_2 = MockFamStructWrapper::new(0).unwrap();
        let mut wrapper_3 = MockFamStructWrapper::new(0).unwrap();

        for i in 0..MAX_LEN {
            assert!(wrapper_1.push(i as u32).is_ok());
            assert!(wrapper_2.push(i as u32).is_ok());
            assert!(wrapper_3.push(0).is_ok());
        }

        assert!(wrapper_1 == wrapper_2);
        assert!(wrapper_1 != wrapper_3);
    }

    #[test]
    fn test_clone() {
        let mut adapter = MockFamStructWrapper::new(0).unwrap();

        for i in 0..MAX_LEN {
            assert!(adapter.push(i as u32).is_ok());
        }

        assert!(adapter == adapter.clone());
    }

    #[test]
    fn test_raw_content() {
        let data = vec![
            MockFamStruct {
                len: 2,
                padding: 5,
                entries: __IncompleteArrayField::new(),
            },
            MockFamStruct {
                len: 0xA5,
                padding: 0x1e,
                entries: __IncompleteArrayField::new(),
            },
        ];

        let mut wrapper = unsafe { MockFamStructWrapper::from_raw(data) };
        {
            let payload = wrapper.as_slice();
            assert_eq!(payload[0], 0xA5);
            assert_eq!(payload[1], 0x1e);
        }
        assert_eq!(wrapper.as_mut_fam_struct().padding, 5);
        let data = wrapper.into_raw();
        assert_eq!(data[0].len, 2);
        assert_eq!(data[0].padding, 5);
    }

    #[cfg(feature = "with-serde")]
    #[test]
    fn test_ser_deser() {
        #[repr(C)]
        #[derive(Default, PartialEq)]
        #[cfg_attr(feature = "with-serde", derive(Deserialize, Serialize))]
        struct Message {
            pub len: u32,
            pub padding: u32,
            pub value: u32,
            #[cfg_attr(feature = "with-serde", serde(skip))]
            pub entries: __IncompleteArrayField<u32>,
        }

        generate_fam_struct_impl!(Message, u32, entries, u32, len, 100);

        type MessageFamStructWrapper = FamStructWrapper<Message>;

        let data = vec![
            Message {
                len: 2,
                padding: 0,
                value: 42,
                entries: __IncompleteArrayField::new(),
            },
            Message {
                len: 0xA5,
                padding: 0x1e,
                value: 0,
                entries: __IncompleteArrayField::new(),
            },
        ];

        let wrapper = unsafe { MessageFamStructWrapper::from_raw(data) };
        let data_ser = serde_json::to_string(&wrapper).unwrap();
        assert_eq!(
            data_ser,
            "[{\"len\":2,\"padding\":0,\"value\":42},[165,30]]"
        );
        let data_deser =
            serde_json::from_str::<MessageFamStructWrapper>(data_ser.as_str()).unwrap();
        assert!(wrapper.eq(&data_deser));

        let bad_data_ser = r#"{"foo": "bar"}"#;
        assert!(serde_json::from_str::<MessageFamStructWrapper>(bad_data_ser).is_err());

        #[repr(C)]
        #[derive(Default)]
        #[cfg_attr(feature = "with-serde", derive(Deserialize, Serialize))]
        struct Message2 {
            pub len: u32,
            pub padding: u32,
            pub value: u32,
            #[cfg_attr(feature = "with-serde", serde(skip))]
            pub entries: __IncompleteArrayField<u32>,
        }

        // Maximum number of entries = 1, so the deserialization should fail because of this reason.
        generate_fam_struct_impl!(Message2, u32, entries, u32, len, 1);

        type Message2FamStructWrapper = FamStructWrapper<Message2>;
        assert!(serde_json::from_str::<Message2FamStructWrapper>(data_ser.as_str()).is_err());
    }

    #[test]
    fn test_clone_multiple_fields() {
        #[derive(Default, PartialEq)]
        #[repr(C)]
        struct Foo {
            index: u32,
            length: u16,
            flags: u32,
            entries: __IncompleteArrayField<u32>,
        }

        generate_fam_struct_impl!(Foo, u32, entries, u16, length, 100);

        type FooFamStructWrapper = FamStructWrapper<Foo>;

        let mut wrapper = FooFamStructWrapper::new(0).unwrap();
        wrapper.as_mut_fam_struct().index = 1;
        wrapper.as_mut_fam_struct().flags = 2;
        wrapper.as_mut_fam_struct().length = 3;
        wrapper.push(3).unwrap();
        wrapper.push(14).unwrap();
        assert_eq!(wrapper.as_slice().len(), 3 + 2);
        assert_eq!(wrapper.as_slice()[3], 3);
        assert_eq!(wrapper.as_slice()[3 + 1], 14);

        let mut wrapper2 = wrapper.clone();
        assert_eq!(
            wrapper.as_mut_fam_struct().index,
            wrapper2.as_mut_fam_struct().index
        );
        assert_eq!(
            wrapper.as_mut_fam_struct().length,
            wrapper2.as_mut_fam_struct().length
        );
        assert_eq!(
            wrapper.as_mut_fam_struct().flags,
            wrapper2.as_mut_fam_struct().flags
        );
        assert_eq!(wrapper.as_slice(), wrapper2.as_slice());
        assert_eq!(
            wrapper2.as_slice().len(),
            wrapper2.as_mut_fam_struct().length as usize
        );
        assert!(wrapper == wrapper2);

        wrapper.as_mut_fam_struct().index = 3;
        assert!(wrapper != wrapper2);

        wrapper.as_mut_fam_struct().length = 7;
        assert!(wrapper != wrapper2);

        wrapper.push(1).unwrap();
        assert_eq!(wrapper.as_mut_fam_struct().length, 8);
        assert!(wrapper != wrapper2);

        let mut wrapper2 = wrapper.clone();
        assert!(wrapper == wrapper2);

        // Dropping the original variable should not affect its clone.
        drop(wrapper);
        assert_eq!(wrapper2.as_mut_fam_struct().index, 3);
        assert_eq!(wrapper2.as_mut_fam_struct().length, 8);
        assert_eq!(wrapper2.as_mut_fam_struct().flags, 2);
        assert_eq!(wrapper2.as_slice(), [0, 0, 0, 3, 14, 0, 0, 1]);
    }
}
