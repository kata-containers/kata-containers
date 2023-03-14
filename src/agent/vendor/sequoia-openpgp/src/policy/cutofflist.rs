use std::fmt;
use std::ops::{Deref, Index, IndexMut};

use crate::{
    Error,
    Result,
    types::Timestamp,
    types::Duration,
};

// A `const fn` function can only use a subset of Rust's
// functionality.  The subset is growing, but we restrict ourselves to
// only use `const fn` functionality that is available in Debian
// stable, which, as of 2020, includes rustc version 1.34.2.  This
// requires a bit of creativity.
#[derive(Debug, Clone)]
pub(super) enum VecOrSlice<'a, T> {
    Vec(Vec<T>),
    Slice(&'a [T]),
    Empty(),
}

// Make a `VecOrSlice` act like a `Vec`.
impl<'a, T> VecOrSlice<'a, T> {
    // Returns an empty `VecOrSlice`.
    const fn empty() -> Self {
        VecOrSlice::Empty()
    }

    // Like `Vec::get`.
    fn get(&self, i: usize) -> Option<&T> {
        match self {
            VecOrSlice::Vec(v) => v.get(i),
            VecOrSlice::Slice(s) => s.get(i),
            VecOrSlice::Empty() => None,
        }
    }

    // Like `Vec::len`.
    fn len(&self) -> usize {
        match self {
            VecOrSlice::Vec(v) => v.len(),
            VecOrSlice::Slice(s) => s.len(),
            VecOrSlice::Empty() => 0,
        }
    }

    // Like `Vec::resize`.
    fn resize(&mut self, size: usize, value: T)
        where T: Clone
    {
        let v = self.as_mut();
        v.resize(size, value);
    }

    pub(super) fn as_mut(&mut self) -> &mut Vec<T>
        where T: Clone
    {
        let v: Vec<T> = match self {
            VecOrSlice::Vec(ref mut v) => std::mem::take(v),
            VecOrSlice::Slice(s) => s.to_vec(),
            VecOrSlice::Empty() => Vec::new(),
        };

        *self = VecOrSlice::Vec(v);
        if let VecOrSlice::Vec(ref mut v) = self {
            v
        } else {
            unreachable!()
        }
    }
}

impl<'a, T> Deref for VecOrSlice<'a, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        match self {
            VecOrSlice::Vec(ref v) => &v[..],
            VecOrSlice::Slice(s) => s,
            VecOrSlice::Empty() => &[],
        }
    }
}

impl<'a, T> Index<usize> for VecOrSlice<'a, T> {
    type Output = T;

    fn index(&self, i: usize) -> &T {
        match self {
            VecOrSlice::Vec(v) => &v[i],
            VecOrSlice::Slice(s) => &s[i],
            VecOrSlice::Empty() => &[][i],
        }
    }
}

impl<'a, T> IndexMut<usize> for VecOrSlice<'a, T>
    where T: Clone
{
    fn index_mut(&mut self, i: usize) -> &mut T {
        if let VecOrSlice::Slice(s) = self {
            *self = VecOrSlice::Vec(s.to_vec());
        };

        match self {
            VecOrSlice::Vec(v) => &mut v[i],
            VecOrSlice::Slice(_) => unreachable!(),
            VecOrSlice::Empty() =>
                panic!("index out of bounds: the len is 0 but the index is {}",
                       i),
        }
    }
}

/// A given algorithm may be considered: completely broken, safe, or
/// too weak to be used after a certain time.
#[derive(Debug, Clone)]
pub(super) struct CutoffList<A> {
    // Indexed by `A as u8`.
    //
    // A value of `None` means that no vulnerabilities are known.
    //
    // Note: we use `u64` and not `SystemTime`, because there is no
    // way to construct a `SystemTime` in a `const fn`.
    pub(super) cutoffs: VecOrSlice<'static, Option<Timestamp>>,

    pub(super) _a: std::marker::PhantomData<A>,
}

pub(super) const REJECT : Option<Timestamp> = Some(Timestamp::UNIX_EPOCH);
pub(super) const ACCEPT : Option<Timestamp> = None;

pub(super) const DEFAULT_POLICY : Option<Timestamp> = REJECT;

impl<A> Default for CutoffList<A> {
    fn default() -> Self {
        Self::reject_all()
    }
}

impl<A> CutoffList<A> {
    // Rejects all algorithms.
    pub(super) const fn reject_all() -> Self {
        Self {
            cutoffs: VecOrSlice::empty(),
            _a: std::marker::PhantomData,
        }
    }
}

impl<A> CutoffList<A>
    where u8: From<A>,
          A: fmt::Display,
          A: std::clone::Clone
{
    // Sets a cutoff time.
    pub(super) fn set(&mut self, a: A, cutoff: Option<Timestamp>) {
        let i : u8 = a.into();
        let i : usize = i.into();

        if i >= self.cutoffs.len() {
            // We reject by default.
            self.cutoffs.resize(i + 1, DEFAULT_POLICY);
        }
        self.cutoffs[i] = cutoff;
    }

    // Returns the cutoff time for algorithm `a`.
    #[inline]
    pub(super) fn cutoff(&self, a: A) -> Option<Timestamp> {
        let i : u8 = a.into();
        *self.cutoffs.get(i as usize).unwrap_or(&DEFAULT_POLICY)
    }

    // Checks whether the `a` is safe to use at time `time`.
    //
    // `tolerance` is added to the cutoff time.
    #[inline]
    pub(super) fn check(&self, a: A, time: Timestamp,
                        tolerance: Option<Duration>)
        -> Result<()>
    {
        if let Some(cutoff) = self.cutoff(a.clone()) {
            let cutoff = cutoff
                .checked_add(tolerance.unwrap_or_else(|| Duration::seconds(0)))
                .unwrap_or(Timestamp::MAX);
            if time >= cutoff {
                Err(Error::PolicyViolation(
                    a.to_string(), Some(cutoff.into())).into())
            } else {
                Ok(())
            }
        } else {
            // None => always secure.
            Ok(())
        }
    }
}

macro_rules! a_cutoff_list {
    ($name:ident, $algo:ty, $values_count:expr, $values:expr) => {
        // It would be nicer to just have a `CutoffList` and store the
        // default as a `VecOrSlice::Slice`.  Unfortunately, we can't
        // create a slice in a `const fn`, so that doesn't work.
        //
        // To work around that issue, we store the array in the
        // wrapper type, and remember if we are using it or a custom
        // version.
        #[derive(Debug, Clone)]
        enum $name {
            Default(),
            Custom(CutoffList<$algo>),
        }

        #[allow(unused)]
        impl $name {
            const DEFAULTS : [ Option<Timestamp>; $values_count ] = $values;

            // Turn the `Foo::Default` into a `Foo::Custom`, if
            // necessary.
            fn force(&mut self) -> &mut CutoffList<$algo> {
                use crate::policy::cutofflist::VecOrSlice;

                if let $name::Default() = self {
                    *self = $name::Custom(CutoffList {
                        cutoffs: VecOrSlice::Vec(Self::DEFAULTS.to_vec()),
                        _a: std::marker::PhantomData,
                    });
                }

                match self {
                    $name::Custom(ref mut l) => l,
                    _ => unreachable!(),
                }
            }

            fn set(&mut self, a: $algo, cutoff: Option<Timestamp>) {
                self.force().set(a, cutoff)
            }

            // Reset the cutoff list to its defaults.
            fn defaults(&mut self) {
                *self = Self::Default();
            }

            fn reject_all(&mut self) {
                *self = Self::Custom(CutoffList::reject_all());
            }

            fn cutoff(&self, a: $algo) -> Option<Timestamp> {
                use crate::policy::cutofflist::DEFAULT_POLICY;

                match self {
                    $name::Default() => {
                        let i : u8 = a.into();
                        let i : usize = i.into();

                        if i >= Self::DEFAULTS.len() {
                            DEFAULT_POLICY
                        } else {
                            Self::DEFAULTS[i]
                        }
                    }
                    $name::Custom(ref l) => l.cutoff(a),
                }
            }

            fn check(&self, a: $algo, time: Timestamp, d: Option<types::Duration>)
                -> Result<()>
            {
                use crate::policy::cutofflist::VecOrSlice;

                match self {
                    $name::Default() => {
                        // Convert the default to a `CutoffList` on
                        // the fly to avoid duplicating
                        // `CutoffList::check`.
                        CutoffList {
                            cutoffs: VecOrSlice::Slice(&Self::DEFAULTS[..]),
                            _a: std::marker::PhantomData,
                        }.check(a, time, d)
                    }

                    $name::Custom(ref l) => l.check(a, time, d),
                }
            }
        }
    }
}

/// A data structure may have multiple versions.  For instance, there
/// are multiple versions of packets.  Each version of a given packet
/// may have different security properties.
#[derive(Debug, Clone)]
pub(super) struct VersionedCutoffList<A> where A: 'static {
    // Indexed by `A as u8`.
    //
    // A value of `None` means that no vulnerabilities are known.
    //
    // Note: we use `u64` and not `SystemTime`, because there is no
    // way to construct a `SystemTime` in a `const fn`.
    pub(super) unversioned_cutoffs: VecOrSlice<'static, Option<Timestamp>>,

    // The content is: (algo, version, policy).
    pub(super) versioned_cutoffs:
        VecOrSlice<'static, (A, u8, Option<Timestamp>)>,

    pub(super) _a: std::marker::PhantomData<A>,
}

impl<A> Default for VersionedCutoffList<A> {
    fn default() -> Self {
        Self::reject_all()
    }
}

impl<A> VersionedCutoffList<A> {
    // Rejects all algorithms.
    pub(super) const fn reject_all() -> Self {
        Self {
            unversioned_cutoffs: VecOrSlice::empty(),
            versioned_cutoffs: VecOrSlice::empty(),
            _a: std::marker::PhantomData,
        }
    }
}

impl<A> VersionedCutoffList<A>
    where u8: From<A>,
          A: fmt::Display,
          A: std::clone::Clone,
          A: Eq,
          A: Ord,
{
    // versioned_cutoffs must be sorted and deduplicated.  Make sure
    // it is so.
    pub(super) fn assert_sorted(&self) {
        if cfg!(debug_assertions) || cfg!(test) {
            for window in self.versioned_cutoffs.windows(2) {
                let a = &window[0];
                let b = &window[1];

                // Sorted, no duplicates.
                assert!((&a.0, a.1) < (&b.0, b.1));
            }
        }
    }

    // Sets a cutoff time for version `version` of algorithm `algo`.
    pub(super) fn set_versioned(&mut self,
                                algo: A, version: u8,
                                cutoff: Option<Timestamp>)
    {
        self.assert_sorted();
        let cutofflist = self.versioned_cutoffs.as_mut();
        match cutofflist.binary_search_by(|(a, v, _)| {
            algo.cmp(a).then(version.cmp(v)).reverse()
        }) {
            Ok(i) => {
                // Replace.
                cutofflist[i] = (algo, version, cutoff);
            }
            Err(i) => {
                // Insert.
                cutofflist.insert(i, (algo, version, cutoff));
            }
        };
        self.assert_sorted();
    }

    // Sets a cutoff time for algorithm `algo`.
    pub(super) fn set_unversioned(&mut self, algo: A,
                                  cutoff: Option<Timestamp>)
    {
        let i: u8 = algo.into();
        let i: usize = i.into();

        if i >= self.unversioned_cutoffs.len() {
            // We reject by default.
            self.unversioned_cutoffs.resize(i + 1, DEFAULT_POLICY);
        }
        self.unversioned_cutoffs[i] = cutoff;
    }

    // Returns the cutoff time for version `version` of algorithm `algo`.
    #[inline]
    pub(super) fn cutoff(&self, algo: A, version: u8) -> Option<Timestamp> {
        self.assert_sorted();
        match self.versioned_cutoffs.binary_search_by(|(a, v, _)| {
            algo.cmp(a).then(version.cmp(v)).reverse()
        }) {
            Ok(i) => {
                self.versioned_cutoffs[i].2
            }
            Err(_loc) => {
                // Fallback to the unversioned cutoff list.
                *self.unversioned_cutoffs.get(u8::from(algo) as usize)
                    .unwrap_or(&DEFAULT_POLICY)
            }
        }
    }

    // Checks whether version `version` of the algorithm `algo` is safe
    // to use at time `time`.
    //
    // `tolerance` is added to the cutoff time.
    #[inline]
    pub(super) fn check(&self, algo: A, version: u8, time: Timestamp,
                        tolerance: Option<Duration>)
        -> Result<()>
    {
        if let Some(cutoff) = self.cutoff(algo.clone(), version) {
            let cutoff = cutoff
                .checked_add(tolerance.unwrap_or_else(|| Duration::seconds(0)))
                .unwrap_or(Timestamp::MAX);
            if time >= cutoff {
                Err(Error::PolicyViolation(
                    format!("{} v{}", algo, version),
                    Some(cutoff.into())).into())
            } else {
                Ok(())
            }
        } else {
            // None => always secure.
            Ok(())
        }
    }
}

macro_rules! a_versioned_cutoff_list {
    ($name:ident, $algo:ty,
     // A slice indexed by the algorithm.
     $unversioned_values_count: expr, $unversioned_values: expr,
     // A slice of the form: [ (algo, version, cutoff), ... ]
     //
     // Note: the values must be sorted and (algo, version) must be
     // unique!
     $versioned_values_count:expr, $versioned_values:expr) => {
        // It would be nicer to just have a `CutoffList` and store the
        // default as a `VecOrSlice::Slice`.  Unfortunately, we can't
        // create a slice in a `const fn`, so that doesn't work.
        //
        // To work around that issue, we store the array in the
        // wrapper type, and remember if we are using it or a custom
        // version.
        #[derive(Debug, Clone)]
        enum $name {
            Default(),
            Custom(VersionedCutoffList<$algo>),
        }

        impl std::ops::Deref for $name {
            type Target = VersionedCutoffList<$algo>;

            fn deref(&self) -> &Self::Target {
                match self {
                    $name::Default() => &Self::DEFAULT,
                    $name::Custom(l) => l,
                }
            }
        }

        #[allow(unused)]
        impl $name {
            const VERSIONED_DEFAULTS:
                [ ($algo, u8, Option<Timestamp>); $versioned_values_count ]
                = $versioned_values;
            const UNVERSIONED_DEFAULTS:
                [ Option<Timestamp>; $unversioned_values_count ]
                = $unversioned_values;

            const DEFAULT: VersionedCutoffList<$algo> = VersionedCutoffList {
                versioned_cutoffs:
                    crate::policy::cutofflist::VecOrSlice::Slice(
                        &Self::VERSIONED_DEFAULTS),
                unversioned_cutoffs:
                    crate::policy::cutofflist::VecOrSlice::Slice(
                        &Self::UNVERSIONED_DEFAULTS),
                _a: std::marker::PhantomData,
            };

            // Turn the `Foo::Default` into a `Foo::Custom`, if
            // necessary, to allow modification.
            fn force(&mut self) -> &mut VersionedCutoffList<$algo> {
                use crate::policy::cutofflist::VecOrSlice;

                if let $name::Default() = self {
                    *self = Self::Custom($name::DEFAULT);
                }

                match self {
                    $name::Custom(ref mut l) => l,
                    _ => unreachable!(),
                }
            }

            // Set the cutoff for the specified version of the
            // specified algorithm.
            fn set_versioned(&mut self, algo: $algo, version: u8,
                             cutoff: Option<Timestamp>)
            {
                self.force().set_versioned(algo, version, cutoff)
            }

            // Sets the cutoff for the specified algorithm independent
            // of its version.
            fn set_unversioned(&mut self, algo: $algo,
                               cutoff: Option<Timestamp>)
            {
                // Clear any versioned cutoffs.
                let l = self.force();
                l.versioned_cutoffs.as_mut().retain(|(a, _v, _c)| {
                    &algo != a
                });

                l.set_unversioned(algo, cutoff)
            }

            // Resets the cutoff list to its defaults.
            fn defaults(&mut self) {
                *self = Self::Default();
            }

            // Causes the cutoff list to reject everything.
            fn reject_all(&mut self) {
                *self = Self::Custom(VersionedCutoffList::reject_all());
            }

            // Returns the cutoff for the specified version of the
            // specified algorithm.
            //
            // This first considers the versioned cutoff list.  If
            // there is no entry in the versioned list, it fallsback
            // to the unversioned cutoff list.  If there is also no
            // entry there, then it falls back to the default.
            fn cutoff(&self, algo: $algo, version: u8) -> Option<Timestamp> {
                let cutofflist = if let $name::Custom(ref l) = self {
                    l
                } else {
                    &Self::DEFAULT
                };

                cutofflist.cutoff(algo, version)
            }

            fn check(&self, algo: $algo, version: u8,
                     time: Timestamp, d: Option<types::Duration>)
                -> Result<()>
            {
                let cutofflist = if let $name::Custom(ref l) = self {
                    l
                } else {
                    &Self::DEFAULT
                };

                cutofflist.check(algo, version, time, d)
            }
        }

        // Make sure VERSIONED_DEFAULTS is sorted and the keys are
        // unique.
        #[test]
        #[allow(non_snake_case)]
        fn $name() {
            $name::DEFAULT.assert_sorted();
        }
    }
}
