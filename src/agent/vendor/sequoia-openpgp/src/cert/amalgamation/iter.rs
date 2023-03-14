use std::slice;
use std::fmt;
use std::time::SystemTime;

use crate::{
    types::RevocationStatus,
    cert::prelude::*,
    packet::{
        Unknown,
        UserAttribute,
        UserID,
    },
    policy::Policy,
};

/// An iterator over components.
///
/// Using the [`ComponentAmalgamationIter::with_policy`], it is
/// possible to change the iterator to only return
/// [`ComponentAmalgamation`]s for valid components.  In this case,
/// `ComponentAmalgamationIter::with_policy` transforms the
/// `ComponentAmalgamationIter` into a
/// [`ValidComponentAmalgamationIter`], which returns
/// [`ValidComponentAmalgamation`]s.  `ValidComponentAmalgamation`
/// offers additional filters.
///
/// `ComponentAmalgamationIter` follows the builder pattern.  There is
/// no need to explicitly finalize it: it already implements the
/// `Iterator` trait.
///
/// A `ComponentAmalgamationIter` is returned by [`Cert::userids`],
/// [`Cert::user_attributes`], and [`Cert::unknowns`].
/// ([`Cert::keys`] returns a [`KeyAmalgamationIter`].)
///
/// # Examples
///
/// Iterate over the User IDs in a certificate:
///
/// ```
/// # use sequoia_openpgp as openpgp;
/// use openpgp::cert::prelude::*;
///
/// #
/// # fn main() -> openpgp::Result<()> {
/// #     let (cert, _) =
/// #         CertBuilder::general_purpose(None, Some("alice@example.org"))
/// #         .generate()?;
/// #     let fpr = cert.fingerprint();
/// // Iterate over all User IDs.
/// for ua in cert.userids() {
///     // ua is a `ComponentAmalgamation`, specifically, a `UserIDAmalgamation`.
/// }
/// #     Ok(())
/// # }
/// ```
///
/// Only return valid User IDs.
///
/// ```
/// # use sequoia_openpgp as openpgp;
/// use openpgp::cert::prelude::*;
/// use openpgp::policy::StandardPolicy;
/// #
/// # fn main() -> openpgp::Result<()> {
/// let p = &StandardPolicy::new();
///
/// #     let (cert, _) =
/// #         CertBuilder::general_purpose(None, Some("alice@example.org"))
/// #         .generate()?;
/// #     let fpr = cert.fingerprint();
/// // Iterate over all valid User IDs.
/// for ua in cert.userids().with_policy(p, None) {
///     // ua is a `ValidComponentAmalgamation`, specifically, a
///     // `ValidUserIDAmalgamation`.
/// }
/// #     Ok(())
/// # }
/// ```
///
/// [`ComponentAmalgamationIter::with_policy`]: ComponentAmalgamationIter::with_policy()
/// [`Cert::userids`]: super::Cert::userids()
/// [`Cert::user_attributes`]: super::Cert::user_attributes()
/// [`Cert::unknowns`]: super::Cert::unknowns()
/// [`Cert::keys`]: super::Cert::keys()
pub struct ComponentAmalgamationIter<'a, C> {
    cert: &'a Cert,
    iter: slice::Iter<'a, ComponentBundle<C>>,
}
assert_send_and_sync!(ComponentAmalgamationIter<'_, C> where C);

/// An iterator over `UserIDAmalgamtion`s.
///
/// A specialized version of [`ComponentAmalgamationIter`].
///
pub type UserIDAmalgamationIter<'a>
    = ComponentAmalgamationIter<'a, UserID>;

/// An iterator over `UserAttributeAmalgamtion`s.
///
/// A specialized version of [`ComponentAmalgamationIter`].
///
pub type UserAttributeAmalgamationIter<'a>
    = ComponentAmalgamationIter<'a, UserAttribute>;

/// An iterator over `UnknownComponentAmalgamtion`s.
///
/// A specialized version of [`ComponentAmalgamationIter`].
///
pub type UnknownComponentAmalgamationIter<'a>
    = ComponentAmalgamationIter<'a, Unknown>;


impl<'a, C> fmt::Debug for ComponentAmalgamationIter<'a, C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ComponentAmalgamationIter")
            .finish()
    }
}

impl<'a, C> Iterator for ComponentAmalgamationIter<'a, C>
{
    type Item = ComponentAmalgamation<'a, C>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|c| ComponentAmalgamation::new(self.cert, c))
    }
}

impl<'a, C> ComponentAmalgamationIter<'a, C> {
    /// Returns a new `ComponentAmalgamationIter` instance.
    pub(crate) fn new(cert: &'a Cert,
                      iter: std::slice::Iter<'a, ComponentBundle<C>>) -> Self
        where Self: 'a
    {
        ComponentAmalgamationIter {
            cert, iter,
        }
    }

    /// Changes the iterator to only return components that are valid
    /// according to the policy at the specified time.
    ///
    /// If `time` is None, then the current time is used.
    ///
    /// Refer to the [`ValidateAmalgamation`] trait for a definition
    /// of a valid component.
    ///
    /// [`ValidateAmalgamation`]: super::ValidateAmalgamation
    ///
    /// # Examples
    ///
    /// ```
    /// # use sequoia_openpgp as openpgp;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::policy::StandardPolicy;
    /// #
    /// # fn main() -> openpgp::Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// #     let (cert, _) =
    /// #         CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #         .generate()?;
    /// #     let fpr = cert.fingerprint();
    /// // Iterate over all valid User Attributes.
    /// for ua in cert.user_attributes().with_policy(p, None) {
    ///     // ua is a `ValidComponentAmalgamation`, specifically, a
    ///     // `ValidUserAttributeAmalgamation`.
    /// }
    /// #     Ok(())
    /// # }
    /// ```
    ///
    pub fn with_policy<T>(self, policy: &'a dyn Policy, time: T)
        -> ValidComponentAmalgamationIter<'a, C>
        where T: Into<Option<SystemTime>>
    {
        ValidComponentAmalgamationIter {
            cert: self.cert,
            iter: self.iter,
            time: time.into().unwrap_or_else(crate::now),
            policy,
            revoked: None,
        }
    }
}

/// An iterator over valid components.
///
/// A `ValidComponentAmalgamationIter` is a
/// [`ComponentAmalgamationIter`] with a policy and a reference time.
///
/// This allows it to filter the returned components based on
/// information available in the components' binding signatures.  For
/// instance, [`ValidComponentAmalgamationIter::revoked`] filters the
/// returned components by whether or not they are revoked.
///
/// `ValidComponentAmalgamationIter` follows the builder pattern.
/// There is no need to explicitly finalize it: it already implements
/// the `Iterator` trait.
///
/// A `ValidComponentAmalgamationIter` is returned by
/// [`ComponentAmalgamationIter::with_policy`].
///
/// # Examples
///
/// ```
/// # use sequoia_openpgp as openpgp;
/// use openpgp::cert::prelude::*;
/// use openpgp::policy::StandardPolicy;
/// #
/// # fn main() -> openpgp::Result<()> {
/// let p = &StandardPolicy::new();
///
/// #     let (cert, _) =
/// #         CertBuilder::general_purpose(None, Some("alice@example.org"))
/// #         .generate()?;
/// #     let fpr = cert.fingerprint();
/// // Iterate over all valid User Attributes.
/// for ua in cert.userids().with_policy(p, None) {
///     // ua is a `ValidComponentAmalgamation`, specifically, a
///     // `ValidUserIDAmalgamation`.
/// }
/// #     Ok(())
/// # }
/// ```
///
/// [`ValidComponentAmalgamationIter::revoked`]: ValidComponentAmalgamationIter::revoked()
/// [`ComponentAmalgamationIter::with_policy`]: ComponentAmalgamationIter::with_policy()
pub struct ValidComponentAmalgamationIter<'a, C> {
    // This is an option to make it easier to create an empty ValidComponentAmalgamationIter.
    cert: &'a Cert,
    iter: slice::Iter<'a, ComponentBundle<C>>,

    policy: &'a dyn Policy,
    // The time.
    time: SystemTime,

    // If not None, filters by whether the component is revoked or not
    // at time `t`.
    revoked: Option<bool>,
}
assert_send_and_sync!(ValidComponentAmalgamationIter<'_, C> where C);

/// An iterator over `ValidUserIDAmalgamtion`s.
///
/// This is just a specialized version of `ValidComponentAmalgamationIter`.
pub type ValidUserIDAmalgamationIter<'a>
    = ValidComponentAmalgamationIter<'a, UserID>;

/// An iterator over `ValidUserAttributeAmalgamtion`s.
///
/// This is just a specialized version of `ValidComponentAmalgamationIter`.
pub type ValidUserAttributeAmalgamationIter<'a>
    = ValidComponentAmalgamationIter<'a, UserAttribute>;


impl<'a, C> fmt::Debug for ValidComponentAmalgamationIter<'a, C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ValidComponentAmalgamationIter")
            .field("time", &self.time)
            .field("revoked", &self.revoked)
            .finish()
    }
}

impl<'a, C> Iterator for ValidComponentAmalgamationIter<'a, C>
    where C: std::fmt::Debug
{
    type Item = ValidComponentAmalgamation<'a, C>;

    fn next(&mut self) -> Option<Self::Item> {
        tracer!(false, "ValidComponentAmalgamationIter::next", 0);
        t!("ValidComponentAmalgamationIter: {:?}", self);

        loop {
            let ca = ComponentAmalgamation::new(self.cert, self.iter.next()?);
            t!("Considering component: {:?}", ca.component());

            let vca = match ca.with_policy(self.policy, self.time) {
                Ok(vca) => vca,
                Err(e) => {
                    t!("Rejected: {}", e);
                    continue;
                },
            };

            if let Some(want_revoked) = self.revoked {
                if let RevocationStatus::Revoked(_) = vca.revocation_status() {
                    // The component is definitely revoked.
                    if ! want_revoked {
                        t!("Component revoked... skipping.");
                        continue;
                    }
                } else {
                    // The component is probably not revoked.
                    if want_revoked {
                        t!("Component not revoked... skipping.");
                        continue;
                    }
                }
            }

            return Some(vca);
        }
    }
}

impl<'a, C> ExactSizeIterator for ComponentAmalgamationIter<'a, C>
{
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<'a, C> ValidComponentAmalgamationIter<'a, C> {
    /// Filters by whether a component is definitely revoked.
    ///
    /// A value of None disables this filter.
    ///
    /// If you call this function multiple times on the same iterator,
    /// only the last value is used.
    ///
    /// This filter only checks if the component is not revoked; it
    /// does not check whether the certificate not revoked.
    ///
    /// This filter checks whether a component's revocation status is
    /// [`RevocationStatus::Revoked`] or not.  The latter (i.e.,
    /// `revoked(false)`) is equivalent to:
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::types::RevocationStatus;
    /// use sequoia_openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> Result<()> {
    /// let p = &StandardPolicy::new();
    ///
    /// # let (cert, _) =
    /// #     CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #     .generate()?;
    /// # let timestamp = None;
    /// let non_revoked_uas = cert
    ///     .user_attributes()
    ///     .with_policy(p, timestamp)
    ///     .filter(|ca| {
    ///         match ca.revocation_status() {
    ///             RevocationStatus::Revoked(_) =>
    ///                 // It's definitely revoked, skip it.
    ///                 false,
    ///             RevocationStatus::CouldBe(_) =>
    ///                 // There is a designated revoker that we
    ///                 // should check, but don't (or can't).  To
    ///                 // avoid a denial of service arising from fake
    ///                 // revocations, we assume that the component has not
    ///                 // been revoked and return it.
    ///                 true,
    ///             RevocationStatus::NotAsFarAsWeKnow =>
    ///                 // We have no evidence to suggest that the component
    ///                 // is revoked.
    ///                 true,
    ///         }
    ///     })
    ///     .collect::<Vec<_>>();
    /// #     Ok(())
    /// # }
    /// ```
    ///
    /// As the example shows, this filter is significantly less
    /// flexible than using `ValidComponentAmalgamation::revocation_status`.
    /// However, this filter implements a typical policy, and does not
    /// preclude using `filter` to realize alternative policies.
    ///
    /// [`RevocationStatus::Revoked`]: crate::types::RevocationStatus::Revoked
    pub fn revoked<T>(mut self, revoked: T) -> Self
        where T: Into<Option<bool>>
    {
        self.revoked = revoked.into();
        self
    }
}
