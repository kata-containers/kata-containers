//! A cache for users and groups provided by the OS.
//!
//! Because the users table changes so infrequently, it's common for
//! short-running programs to cache the results instead of getting the most
//! up-to-date entries every time. The [`UsersCache`](struct.UsersCache.html)
//! type helps with this, providing methods that have the same name as the
//! others in this crate, only they store the results.
//!
//! ## Example
//!
//! ```no_run
//! use std::sync::Arc;
//! use users::{Users, UsersCache};
//!
//! let mut cache = UsersCache::new();
//! let user      = cache.get_user_by_uid(502).expect("User not found");
//! let same_user = cache.get_user_by_uid(502).unwrap();
//!
//! // The two returned values point to the same User
//! assert!(Arc::ptr_eq(&user, &same_user));
//! ```
//!
//! ## Caching, multiple threads, and mutability
//!
//! The `UsersCache` type is caught between a rock and a hard place when it
//! comes to providing references to users and groups.
//!
//! Instead of returning a fresh `User` struct each time, for example, it will
//! return a reference to the version it currently has in its cache. So you can
//! ask for User #501 twice, and you’ll get a reference to the same value both
//! time. Its methods are *idempotent* -- calling one multiple times has the
//! same effect as calling one once.
//!
//! This works fine in theory, but in practice, the cache has to update its own
//! state somehow: it contains several `HashMap`s that hold the result of user
//! and group lookups. Rust provides mutability in two ways:
//!
//! 1. Have its methods take `&mut self`, instead of `&self`, allowing the
//!   internal maps to be mutated (“inherited mutability”)
//! 2. Wrap the internal maps in a `RefCell`, allowing them to be modified
//!   (“interior mutability”).
//!
//! Unfortunately, Rust is also very protective of references to a mutable
//! value. In this case, switching to `&mut self` would only allow for one user
//! to be read at a time!
//!
//! ```no_run
//! use users::{Users, Groups, UsersCache};
//!
//! let mut cache = UsersCache::new();
//!
//! let uid   = cache.get_current_uid();                          // OK...
//! let user  = cache.get_user_by_uid(uid).unwrap();              // OK...
//! let group = cache.get_group_by_gid(user.primary_group_id());  // No!
//! ```
//!
//! When we get the `user`, it returns an optional reference (which we unwrap)
//! to the user’s entry in the cache. This is a reference to something contained
//! in a mutable value. Then, when we want to get the user’s primary group, it
//! will return *another* reference to the same mutable value. This is something
//! that Rust explicitly disallows!
//!
//! The compiler wasn’t on our side with Option 1, so let’s try Option 2:
//! changing the methods back to `&self` instead of `&mut self`, and using
//! `RefCell`s internally. However, Rust is smarter than this, and knows that
//! we’re just trying the same trick as earlier. A simplified implementation of
//! a user cache lookup would look something like this:
//!
//! ```text
//! fn get_user_by_uid(&self, uid: uid_t) -> Option<&User> {
//!     let users = self.users.borrow_mut();
//!     users.get(uid)
//! }
//! ```
//!
//! Rust won’t allow us to return a reference like this because the `Ref` of the
//! `RefCell` just gets dropped at the end of the method, meaning that our
//! reference does not live long enough.
//!
//! So instead of doing any of that, we use `Arc` everywhere in order to get
//! around all the lifetime restrictions. Returning reference-counted users and
//! groups mean that we don’t have to worry about further uses of the cache, as
//! the values themselves don’t count as being stored *in* the cache anymore. So
//! it can be queried multiple times or go out of scope and the values it
//! produces are not affected.

use libc::{uid_t, gid_t};
use std::cell::{Cell, RefCell};
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::sync::Arc;

use base::{User, Group, all_users};
use traits::{Users, Groups};


/// A producer of user and group instances that caches every result.
///
/// For more information, see the [`users::cache` module documentation](index.html).
pub struct UsersCache {
    users:  BiMap<uid_t, User>,
    groups: BiMap<gid_t, Group>,

    uid:  Cell<Option<uid_t>>,
    gid:  Cell<Option<gid_t>>,
    euid: Cell<Option<uid_t>>,
    egid: Cell<Option<gid_t>>,
}

/// A kinda-bi-directional `HashMap` that associates keys to values, and
/// then strings back to keys.
///
/// It doesn’t go the full route and offer *values*-to-keys lookup, because we
/// only want to search based on usernames and group names. There wouldn’t be
/// much point offering a “User to uid” map, as the uid is present in the
/// `User` struct!
struct BiMap<K, V> {
    forward:  RefCell< HashMap<K, Option<Arc<V>>> >,
    backward: RefCell< HashMap<Arc<OsStr>, Option<K>> >,
}


// Default has to be impl’d manually here, because there’s no
// Default impl on User or Group, even though those types aren’t
// needed to produce a default instance of any HashMaps...
impl Default for UsersCache {
    fn default() -> Self {
        Self {
            users: BiMap {
                forward:  RefCell::new(HashMap::new()),
                backward: RefCell::new(HashMap::new()),
            },

            groups: BiMap {
                forward:  RefCell::new(HashMap::new()),
                backward: RefCell::new(HashMap::new()),
            },

            uid:  Cell::new(None),
            gid:  Cell::new(None),
            euid: Cell::new(None),
            egid: Cell::new(None),
        }
    }
}


impl UsersCache {

    /// Creates a new empty cache.
    ///
    /// # Examples
    ///
    /// ```
    /// use users::cache::UsersCache;
    ///
    /// let cache = UsersCache::new();
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new cache that contains all the users present on the system.
    ///
    /// # Safety
    ///
    /// This is `unsafe` because we cannot prevent data races if two caches
    /// were attempted to be initialised on different threads at the same time.
    /// For more information, see the [`all_users` documentation](../fn.all_users.html).
    ///
    /// # Examples
    ///
    /// ```
    /// use users::cache::UsersCache;
    ///
    /// let cache = unsafe { UsersCache::with_all_users() };
    /// ```
    pub unsafe fn with_all_users() -> Self {
        let cache = Self::new();

        for user in all_users() {
            let uid = user.uid();
            let user_arc = Arc::new(user);
            cache.users.forward.borrow_mut().insert(uid, Some(Arc::clone(&user_arc)));
            cache.users.backward.borrow_mut().insert(Arc::clone(&user_arc.name_arc), Some(uid));
        }

        cache
    }
}


// TODO: stop using ‘Arc::from’ with entry API
// The ‘get_*_by_name’ functions below create a new Arc before even testing if
// the user exists in the cache, essentially creating an unnecessary Arc.
// https://internals.rust-lang.org/t/pre-rfc-abandonning-morals-in-the-name-of-performance-the-raw-entry-api/7043/51
// https://github.com/rust-lang/rfcs/pull/1769


impl Users for UsersCache {
    fn get_user_by_uid(&self, uid: uid_t) -> Option<Arc<User>> {
        let mut users_forward = self.users.forward.borrow_mut();

        let entry = match users_forward.entry(uid) {
            Vacant(e) => e,
            Occupied(e) => return e.get().as_ref().map(Arc::clone),
        };

        if let Some(user) = super::get_user_by_uid(uid) {
            let newsername = Arc::clone(&user.name_arc);
            let mut users_backward = self.users.backward.borrow_mut();
            users_backward.insert(newsername, Some(uid));

            let user_arc = Arc::new(user);
            entry.insert(Some(Arc::clone(&user_arc)));
            Some(user_arc)
        }
        else {
            entry.insert(None);
            None
        }
    }

    fn get_user_by_name<S: AsRef<OsStr> + ?Sized>(&self, username: &S) -> Option<Arc<User>> {
        let mut users_backward = self.users.backward.borrow_mut();

        let entry = match users_backward.entry(Arc::from(username.as_ref())) {
            Vacant(e) => e,
            Occupied(e) => {
                return (*e.get()).and_then(|uid| {
                    let users_forward = self.users.forward.borrow_mut();
                    users_forward[&uid].as_ref().map(Arc::clone)
                })
            }
        };

        if let Some(user) = super::get_user_by_name(username) {
            let uid = user.uid();
            let user_arc = Arc::new(user);

            let mut users_forward = self.users.forward.borrow_mut();
            users_forward.insert(uid, Some(Arc::clone(&user_arc)));
            entry.insert(Some(uid));

            Some(user_arc)
        }
        else {
            entry.insert(None);
            None
        }
    }

    fn get_current_uid(&self) -> uid_t {
        self.uid.get().unwrap_or_else(|| {
            let uid = super::get_current_uid();
            self.uid.set(Some(uid));
            uid
        })
    }

    fn get_current_username(&self) -> Option<Arc<OsStr>> {
        let uid = self.get_current_uid();
        self.get_user_by_uid(uid).map(|u| Arc::clone(&u.name_arc))
    }

    fn get_effective_uid(&self) -> uid_t {
        self.euid.get().unwrap_or_else(|| {
            let uid = super::get_effective_uid();
            self.euid.set(Some(uid));
            uid
        })
    }

    fn get_effective_username(&self) -> Option<Arc<OsStr>> {
        let uid = self.get_effective_uid();
        self.get_user_by_uid(uid).map(|u| Arc::clone(&u.name_arc))
    }
}


impl Groups for UsersCache {
    fn get_group_by_gid(&self, gid: gid_t) -> Option<Arc<Group>> {
        let mut groups_forward = self.groups.forward.borrow_mut();

        let entry = match groups_forward.entry(gid) {
            Vacant(e) => e,
            Occupied(e) => return e.get().as_ref().map(Arc::clone),
        };

        if let Some(group) = super::get_group_by_gid(gid) {
            let new_group_name = Arc::clone(&group.name_arc);
            let mut groups_backward = self.groups.backward.borrow_mut();
            groups_backward.insert(new_group_name, Some(gid));

            let group_arc = Arc::new(group);
            entry.insert(Some(Arc::clone(&group_arc)));
            Some(group_arc)
        }
        else {
            entry.insert(None);
            None
        }
    }

    fn get_group_by_name<S: AsRef<OsStr> + ?Sized>(&self, group_name: &S) -> Option<Arc<Group>> {
        let mut groups_backward = self.groups.backward.borrow_mut();

        let entry = match groups_backward.entry(Arc::from(group_name.as_ref())) {
            Vacant(e) => e,
            Occupied(e) => {
                return (*e.get()).and_then(|gid| {
                    let groups_forward = self.groups.forward.borrow_mut();
                    groups_forward[&gid].as_ref().cloned()
                });
            }
        };

        if let Some(group) = super::get_group_by_name(group_name) {
            let group_arc = Arc::new(group.clone());
            let gid = group.gid();

            let mut groups_forward = self.groups.forward.borrow_mut();
            groups_forward.insert(gid, Some(Arc::clone(&group_arc)));
            entry.insert(Some(gid));

            Some(group_arc)
        }
        else {
            entry.insert(None);
            None
        }
    }

    fn get_current_gid(&self) -> gid_t {
        self.gid.get().unwrap_or_else(|| {
            let gid = super::get_current_gid();
            self.gid.set(Some(gid));
            gid
        })
    }

    fn get_current_groupname(&self) -> Option<Arc<OsStr>> {
        let gid = self.get_current_gid();
        self.get_group_by_gid(gid).map(|g| Arc::clone(&g.name_arc))
    }

    fn get_effective_gid(&self) -> gid_t {
        self.egid.get().unwrap_or_else(|| {
            let gid = super::get_effective_gid();
            self.egid.set(Some(gid));
            gid
        })
    }

    fn get_effective_groupname(&self) -> Option<Arc<OsStr>> {
        let gid = self.get_effective_gid();
        self.get_group_by_gid(gid).map(|g| Arc::clone(&g.name_arc))
    }
}
