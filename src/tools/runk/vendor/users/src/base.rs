//! Integration with the C library’s users and groups.
//!
//! This module uses `extern` functions and types from `libc` that integrate
//! with the system’s C library, which integrates with the OS itself to get user
//! and group information. It’s where the “core” user handling is done.
//!
//!
//! ## Name encoding rules
//!
//! Under Unix, usernames and group names are considered to be
//! null-terminated, UTF-8 strings. These are `CString`s in Rust, although in
//! this library, they are just `String` values. Why?
//!
//! The reason is that any user or group values with invalid `CString` data
//! can instead just be assumed to not exist:
//!
//! - If you try to search for a user with a null character in their name,
//!   such a user could not exist anyway — so it’s OK to return `None`.
//! - If the OS returns user information with a null character in a field,
//!   then that field will just be truncated instead, which is valid behaviour
//!   for a `CString`.
//!
//! The downside is that we use `from_utf8_lossy` instead, which has a small
//! runtime penalty when it calculates and scans the length of the string for
//! invalid characters. However, this should not be a problem when dealing with
//! usernames of a few bytes each.
//!
//! In short, if you want to check for null characters in user fields, your
//! best bet is to check for them yourself before passing strings into any
//! functions.

use std::ffi::{CStr, CString, OsStr, OsString};
use std::fmt;
use std::mem;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::ptr;
use std::sync::Arc;

#[cfg(feature = "logging")]
extern crate log;
#[cfg(feature = "logging")]
use self::log::trace;

use libc::{c_char, uid_t, gid_t, c_int};
use libc::passwd as c_passwd;
use libc::group as c_group;


/// Information about a particular user.
///
/// For more information, see the [module documentation](index.html).
#[derive(Clone)]
pub struct User {
    uid: uid_t,
    primary_group: gid_t,
    extras: os::UserExtras,
    pub(crate) name_arc: Arc<OsStr>,
}

impl User {

    /// Create a new `User` with the given user ID, name, and primary
    /// group ID, with the rest of the fields filled with dummy values.
    ///
    /// This method does not actually create a new user on the system — it
    /// should only be used for comparing users in tests.
    ///
    /// # Examples
    ///
    /// ```
    /// use users::User;
    ///
    /// let user = User::new(501, "stevedore", 100);
    /// ```
    pub fn new<S: AsRef<OsStr> + ?Sized>(uid: uid_t, name: &S, primary_group: gid_t) -> Self {
        let name_arc = Arc::from(name.as_ref());
        let extras = os::UserExtras::default();

        Self { uid, name_arc, primary_group, extras }
    }

    /// Returns this user’s ID.
    ///
    /// # Examples
    ///
    /// ```
    /// use users::User;
    ///
    /// let user = User::new(501, "stevedore", 100);
    /// assert_eq!(user.uid(), 501);
    /// ```
    pub fn uid(&self) -> uid_t {
        self.uid
    }

    /// Returns this user’s name.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::ffi::OsStr;
    /// use users::User;
    ///
    /// let user = User::new(501, "stevedore", 100);
    /// assert_eq!(user.name(), OsStr::new("stevedore"));
    /// ```
    pub fn name(&self) -> &OsStr {
        &*self.name_arc
    }

    /// Returns the ID of this user’s primary group.
    ///
    /// # Examples
    ///
    /// ```
    /// use users::User;
    ///
    /// let user = User::new(501, "stevedore", 100);
    /// assert_eq!(user.primary_group_id(), 100);
    /// ```
    pub fn primary_group_id(&self) -> gid_t {
        self.primary_group
    }

    /// Returns a list of groups this user is a member of. This involves
    /// loading the groups list, as it is _not_ contained within this type.
    ///
    /// # libc functions used
    ///
    /// - [`getgrouplist`](https://docs.rs/libc/*/libc/fn.getgrouplist.html)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use users::User;
    ///
    /// let user = User::new(501, "stevedore", 100);
    /// for group in user.groups().expect("User not found") {
    ///     println!("User is in group: {:?}", group.name());
    /// }
    /// ```
    pub fn groups(&self) -> Option<Vec<Group>> {
        get_user_groups(self.name(), self.primary_group_id())
    }
}

impl fmt::Debug for User {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if f.alternate() {
            f.debug_struct("User")
             .field("uid", &self.uid)
             .field("name_arc", &self.name_arc)
             .field("primary_group", &self.primary_group)
             .field("extras", &self.extras)
             .finish()
        }
        else {
            write!(f, "User({}, {})", self.uid(), self.name().to_string_lossy())
        }
    }
}


/// Information about a particular group.
///
/// For more information, see the [module documentation](index.html).
#[derive(Clone)]
pub struct Group {
    gid: gid_t,
    extras: os::GroupExtras,
    pub(crate) name_arc: Arc<OsStr>,
}

impl Group {

    /// Create a new `Group` with the given group ID and name, with the
    /// rest of the fields filled in with dummy values.
    ///
    /// This method does not actually create a new group on the system — it
    /// should only be used for comparing groups in tests.
    ///
    /// # Examples
    ///
    /// ```
    /// use users::Group;
    ///
    /// let group = Group::new(102, "database");
    /// ```
    pub fn new<S: AsRef<OsStr> + ?Sized>(gid: gid_t, name: &S) -> Self {
        let name_arc = Arc::from(name.as_ref());
        let extras = os::GroupExtras::default();

        Self { gid, name_arc, extras }
    }

    /// Returns this group’s ID.
    ///
    /// # Examples
    ///
    /// ```
    /// use users::Group;
    ///
    /// let group = Group::new(102, "database");
    /// assert_eq!(group.gid(), 102);
    /// ```
    pub fn gid(&self) -> gid_t {
        self.gid
    }

    /// Returns this group’s name.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::ffi::OsStr;
    /// use users::Group;
    ///
    /// let group = Group::new(102, "database");
    /// assert_eq!(group.name(), OsStr::new("database"));
    /// ```
    pub fn name(&self) -> &OsStr {
        &*self.name_arc
    }
}

impl fmt::Debug for Group {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if f.alternate() {
            f.debug_struct("Group")
             .field("gid", &self.gid)
             .field("name_arc", &self.name_arc)
             .field("extras", &self.extras)
             .finish()
        }
        else {
            write!(f, "Group({}, {})", self.gid(), self.name().to_string_lossy())
        }
    }
}


/// Reads data from a `*char` field in `c_passwd` or `g_group`. The return
/// type will be an `Arc<OsStr>` if the text is meant to be shared in a cache,
/// or a plain `OsString` if it’s not.
///
/// The underlying buffer is managed by the C library, not by us, so we *need*
/// to move data out of it before the next user gets read.
unsafe fn from_raw_buf<'a, T>(p: *const c_char) -> T
where T: From<&'a OsStr>
{
    T::from(OsStr::from_bytes(CStr::from_ptr(p).to_bytes()))
}

/// Reads data from the `c_passwd` and returns it as a `User`.
unsafe fn passwd_to_user(passwd: c_passwd) -> User {
    #[cfg(feature = "logging")]
    trace!("Loading user with uid {}", passwd.pw_uid);

    let name = from_raw_buf(passwd.pw_name);

    User {
        uid:           passwd.pw_uid,
        name_arc:      name,
        primary_group: passwd.pw_gid,
        extras:        os::UserExtras::from_passwd(passwd),
    }
}

/// Reads data from the `c_group` and returns it as a `Group`.
unsafe fn struct_to_group(group: c_group) -> Group {
    #[cfg(feature = "logging")]
    trace!("Loading group with gid {}", group.gr_gid);

    let name = from_raw_buf(group.gr_name);

    Group {
        gid:      group.gr_gid,
        name_arc: name,
        extras:   os::GroupExtras::from_struct(group),
    }
}

/// Expand a list of group members to a vector of strings.
///
/// The list of members is, in true C fashion, a pointer to a pointer of
/// characters, terminated by a null pointer. We check `members[0]`, then
/// `members[1]`, and so on, until that null pointer is reached. It doesn’t
/// specify whether we should expect a null pointer or a pointer to a null
/// pointer, so we check for both here!
unsafe fn members(groups: *mut *mut c_char) -> Vec<OsString> {
    let mut members = Vec::new();

    for i in 0.. {
        let username = groups.offset(i);

        if username.is_null() || (*username).is_null() {
            break;
        }
        else {
            members.push(from_raw_buf(*username));
        }
    }

    members
}


/// Searches for a `User` with the given ID in the system’s user database.
/// Returns it if one is found, otherwise returns `None`.
///
/// # libc functions used
///
/// - [`getpwuid_r`](https://docs.rs/libc/*/libc/fn.getpwuid_r.html)
///
/// # Examples
///
/// ```
/// use users::get_user_by_uid;
///
/// match get_user_by_uid(501) {
///     Some(user) => println!("Found user {:?}", user.name()),
///     None       => println!("User not found"),
/// }
/// ```
pub fn get_user_by_uid(uid: uid_t) -> Option<User> {
    let mut passwd = unsafe { mem::zeroed::<c_passwd>() };
    let mut buf = vec![0; 2048];
    let mut result = ptr::null_mut::<c_passwd>();

    #[cfg(feature = "logging")]
    trace!("Running getpwuid_r for user #{}", uid);

    loop {
        let r = unsafe {
            libc::getpwuid_r(uid, &mut passwd, buf.as_mut_ptr(), buf.len(), &mut result)
        };

        if r != libc::ERANGE {
            break;
        }

        let newsize = buf.len().checked_mul(2)?;
        buf.resize(newsize, 0);
    }

    if result.is_null() {
        // There is no such user, or an error has occurred.
        // errno gets set if there’s an error.
        return None;
    }

    if result != &mut passwd {
        // The result of getpwuid_r should be its input passwd.
        return None;
    }

    let user = unsafe { passwd_to_user(result.read()) };
    Some(user)
}

/// Searches for a `User` with the given username in the system’s user database.
/// Returns it if one is found, otherwise returns `None`.
///
/// # libc functions used
///
/// - [`getpwnam_r`](https://docs.rs/libc/*/libc/fn.getpwnam_r.html)
///
/// # Examples
///
/// ```
/// use users::get_user_by_name;
///
/// match get_user_by_name("stevedore") {
///     Some(user) => println!("Found user #{}", user.uid()),
///     None       => println!("User not found"),
/// }
/// ```
pub fn get_user_by_name<S: AsRef<OsStr> + ?Sized>(username: &S) -> Option<User> {
    let username = match CString::new(username.as_ref().as_bytes()) {
        Ok(u)  => u,
        Err(_) => {
            // The username that was passed in contained a null character,
            // which will match no usernames.
            return None;
        }
    };

    let mut passwd = unsafe { mem::zeroed::<c_passwd>() };
    let mut buf = vec![0; 2048];
    let mut result = ptr::null_mut::<c_passwd>();

    #[cfg(feature = "logging")]
    trace!("Running getpwnam_r for user {:?}", username.as_ref());

    loop {
        let r = unsafe {
            libc::getpwnam_r(username.as_ptr(), &mut passwd, buf.as_mut_ptr(), buf.len(), &mut result)
        };

        if r != libc::ERANGE {
            break;
        }

        let newsize = buf.len().checked_mul(2)?;
        buf.resize(newsize, 0);
    }

    if result.is_null() {
        // There is no such user, or an error has occurred.
        // errno gets set if there’s an error.
        return None;
    }

    if result != &mut passwd {
        // The result of getpwnam_r should be its input passwd.
        return None;
    }

    let user = unsafe { passwd_to_user(result.read()) };
    Some(user)
}

/// Searches for a `Group` with the given ID in the system’s group database.
/// Returns it if one is found, otherwise returns `None`.
///
/// # libc functions used
///
/// - [`getgrgid_r`](https://docs.rs/libc/*/libc/fn.getgrgid_r.html)
///
/// # Examples
///
/// ```
/// use users::get_group_by_gid;
///
/// match get_group_by_gid(102) {
///     Some(group) => println!("Found group {:?}", group.name()),
///     None        => println!("Group not found"),
/// }
/// ```
pub fn get_group_by_gid(gid: gid_t) -> Option<Group> {
    let mut passwd = unsafe { mem::zeroed::<c_group>() };
    let mut buf = vec![0; 2048];
    let mut result = ptr::null_mut::<c_group>();

    #[cfg(feature = "logging")]
    trace!("Running getgruid_r for group #{}", gid);

    loop {
        let r = unsafe {
            libc::getgrgid_r(gid, &mut passwd, buf.as_mut_ptr(), buf.len(), &mut result)
        };

        if r != libc::ERANGE {
            break;
        }

        let newsize = buf.len().checked_mul(2)?;
        buf.resize(newsize, 0);
    }

    if result.is_null() {
        // There is no such group, or an error has occurred.
        // errno gets set if there’s an error.
        return None;
    }

    if result != &mut passwd {
        // The result of getgrgid_r should be its input struct.
        return None;
    }

    let group = unsafe { struct_to_group(result.read()) };
    Some(group)
}

/// Searches for a `Group` with the given group name in the system’s group database.
/// Returns it if one is found, otherwise returns `None`.
///
/// # libc functions used
///
/// - [`getgrnam_r`](https://docs.rs/libc/*/libc/fn.getgrnam_r.html)
///
/// # Examples
///
/// ```
/// use users::get_group_by_name;
///
/// match get_group_by_name("db-access") {
///     Some(group) => println!("Found group #{}", group.gid()),
///     None        => println!("Group not found"),
/// }
/// ```
pub fn get_group_by_name<S: AsRef<OsStr> + ?Sized>(groupname: &S) -> Option<Group> {
    let groupname = match CString::new(groupname.as_ref().as_bytes()) {
        Ok(u)  => u,
        Err(_) => {
            // The groupname that was passed in contained a null character,
            // which will match no usernames.
            return None;
        }
    };

    let mut group = unsafe { mem::zeroed::<c_group>() };
    let mut buf = vec![0; 2048];
    let mut result = ptr::null_mut::<c_group>();

    #[cfg(feature = "logging")]
    trace!("Running getgrnam_r for group {:?}", groupname.as_ref());

    loop {
        let r = unsafe {
            libc::getgrnam_r(groupname.as_ptr(), &mut group, buf.as_mut_ptr(), buf.len(), &mut result)
        };

        if r != libc::ERANGE {
            break;
        }

        let newsize = buf.len().checked_mul(2)?;
        buf.resize(newsize, 0);
    }

    if result.is_null() {
        // There is no such group, or an error has occurred.
        // errno gets set if there’s an error.
        return None;
    }

    if result != &mut group {
        // The result of getgrnam_r should be its input struct.
        return None;
    }

    let group = unsafe { struct_to_group(result.read()) };
    Some(group)
}

/// Returns the user ID for the user running the process.
///
/// # libc functions used
///
/// - [`getuid`](https://docs.rs/libc/*/libc/fn.getuid.html)
///
/// # Examples
///
/// ```
/// use users::get_current_uid;
///
/// println!("The ID of the current user is {}", get_current_uid());
/// ```
pub fn get_current_uid() -> uid_t {
    #[cfg(feature = "logging")]
    trace!("Running getuid");

    unsafe { libc::getuid() }
}

/// Returns the username of the user running the process.
///
/// This function to return `None` if the current user does not exist, which
/// could happed if they were deleted after the program started running.
///
/// # libc functions used
///
/// - [`getuid`](https://docs.rs/libc/*/libc/fn.getuid.html)
/// - [`getpwuid_r`](https://docs.rs/libc/*/libc/fn.getpwuid_r.html)
///
/// # Examples
///
/// ```
/// use users::get_current_username;
///
/// match get_current_username() {
///     Some(uname) => println!("Running as user with name {:?}", uname),
///     None        => println!("The current user does not exist!"),
/// }
/// ```
pub fn get_current_username() -> Option<OsString> {
    let uid = get_current_uid();
    let user = get_user_by_uid(uid)?;

    Some(OsString::from(&*user.name_arc))
}

/// Returns the user ID for the effective user running the process.
///
/// # libc functions used
///
/// - [`geteuid`](https://docs.rs/libc/*/libc/fn.geteuid.html)
///
/// # Examples
///
/// ```
/// use users::get_effective_uid;
///
/// println!("The ID of the effective user is {}", get_effective_uid());
/// ```
pub fn get_effective_uid() -> uid_t {
    #[cfg(feature = "logging")]
    trace!("Running geteuid");

    unsafe { libc::geteuid() }
}

/// Returns the username of the effective user running the process.
///
/// # libc functions used
///
/// - [`geteuid`](https://docs.rs/libc/*/libc/fn.geteuid.html)
/// - [`getpwuid_r`](https://docs.rs/libc/*/libc/fn.getpwuid_r.html)
///
/// # Examples
///
/// ```
/// use users::get_effective_username;
///
/// match get_effective_username() {
///     Some(uname) => println!("Running as effective user with name {:?}", uname),
///     None        => println!("The effective user does not exist!"),
/// }
/// ```
pub fn get_effective_username() -> Option<OsString> {
    let uid = get_effective_uid();
    let user = get_user_by_uid(uid)?;

    Some(OsString::from(&*user.name_arc))
}

/// Returns the group ID for the user running the process.
///
/// # libc functions used
///
/// - [`getgid`](https://docs.rs/libc/*/libc/fn.getgid.html)
///
/// # Examples
///
/// ```
/// use users::get_current_gid;
///
/// println!("The ID of the current group is {}", get_current_gid());
/// ```
pub fn get_current_gid() -> gid_t {
    #[cfg(feature = "logging")]
    trace!("Running getgid");

    unsafe { libc::getgid() }
}

/// Returns the groupname of the user running the process.
///
/// # libc functions used
///
/// - [`getgid`](https://docs.rs/libc/*/libc/fn.getgid.html)
/// - [`getgrgid`](https://docs.rs/libc/*/libc/fn.getgrgid.html)
///
/// # Examples
///
/// ```
/// use users::get_current_groupname;
///
/// match get_current_groupname() {
///     Some(gname) => println!("Running as group with name {:?}", gname),
///     None        => println!("The current group does not exist!"),
/// }
/// ```
pub fn get_current_groupname() -> Option<OsString> {
    let gid = get_current_gid();
    let group = get_group_by_gid(gid)?;

    Some(OsString::from(&*group.name_arc))
}

/// Returns the group ID for the effective user running the process.
///
/// # libc functions used
///
/// - [`getegid`](https://docs.rs/libc/*/libc/fn.getegid.html)
///
/// # Examples
///
/// ```
/// use users::get_effective_gid;
///
/// println!("The ID of the effective group is {}", get_effective_gid());
/// ```
pub fn get_effective_gid() -> gid_t {
    #[cfg(feature = "logging")]
    trace!("Running getegid");

    unsafe { libc::getegid() }
}

/// Returns the groupname of the effective user running the process.
///
/// # libc functions used
///
/// - [`getegid`](https://docs.rs/libc/*/libc/fn.getegid.html)
/// - [`getgrgid`](https://docs.rs/libc/*/libc/fn.getgrgid.html)
///
/// # Examples
///
/// ```
/// use users::get_effective_groupname;
///
/// match get_effective_groupname() {
///     Some(gname) => println!("Running as effective group with name {:?}", gname),
///     None        => println!("The effective group does not exist!"),
/// }
/// ```
pub fn get_effective_groupname() -> Option<OsString> {
    let gid = get_effective_gid();
    let group = get_group_by_gid(gid)?;

    Some(OsString::from(&*group.name_arc))
}

/// Returns the group access list for the current process.
///
/// # libc functions used
///
/// - [`getgroups`](https://docs.rs/libc/*/libc/fn.getgroups.html)
///
/// # Errors
///
/// This function will return `Err` when an I/O error occurs during the
/// `getgroups` call.
///
/// # Examples
///
/// ```no_run
/// use users::group_access_list;
///
/// for group in group_access_list().expect("Error looking up groups") {
///     println!("Process can access group #{} ({:?})", group.gid(), group.name());
/// }
/// ```
pub fn group_access_list() -> io::Result<Vec<Group>> {
    let mut buff: Vec<gid_t> = vec![0; 1024];

    #[cfg(feature = "logging")]
    trace!("Running getgroups");

    let res = unsafe {
        libc::getgroups(1024, buff.as_mut_ptr())
    };

    if res < 0 {
        Err(io::Error::last_os_error())
    }
    else {
        let mut groups = buff.into_iter()
                             .filter_map(get_group_by_gid)
                             .collect::<Vec<_>>();
        groups.dedup_by_key(|i| i.gid());
        Ok(groups)
    }
}

/// Returns groups for a provided user name and primary group id.
///
/// # libc functions used
///
/// - [`getgrouplist`](https://docs.rs/libc/*/libc/fn.getgrouplist.html)
///
/// # Examples
///
/// ```no_run
/// use users::get_user_groups;
///
/// for group in get_user_groups("stevedore", 1001).expect("Error looking up groups") {
///     println!("User is a member of group #{} ({:?})", group.gid(), group.name());
/// }
/// ```
pub fn get_user_groups<S: AsRef<OsStr> + ?Sized>(username: &S, gid: gid_t) -> Option<Vec<Group>> {
    // MacOS uses i32 instead of gid_t in getgrouplist for unknown reasons
    #[cfg(all(unix, target_os="macos"))]
    let mut buff: Vec<i32> = vec![0; 1024];
    #[cfg(all(unix, not(target_os="macos")))]
    let mut buff: Vec<gid_t> = vec![0; 1024];

    let name = CString::new(username.as_ref().as_bytes()).unwrap();
    let mut count = buff.len() as c_int;

    #[cfg(feature = "logging")]
    trace!("Running getgrouplist for user {:?} and group #{}", username.as_ref(), gid);

    // MacOS uses i32 instead of gid_t in getgrouplist for unknown reasons
    #[cfg(all(unix, target_os="macos"))]
    let res = unsafe {
        libc::getgrouplist(name.as_ptr(), gid as i32, buff.as_mut_ptr(), &mut count)
    };

    #[cfg(all(unix, not(target_os="macos")))]
    let res = unsafe {
        libc::getgrouplist(name.as_ptr(), gid, buff.as_mut_ptr(), &mut count)
    };

    if res < 0 {
        None
    }
    else {
        buff.dedup();
        buff.into_iter()
            .filter_map(|i| get_group_by_gid(i as gid_t))
            .collect::<Vec<_>>()
            .into()
    }
}



/// An iterator over every user present on the system.
struct AllUsers;

/// Creates a new iterator over every user present on the system.
///
/// # libc functions used
///
/// - [`getpwent`](https://docs.rs/libc/*/libc/fn.getpwent.html)
/// - [`setpwent`](https://docs.rs/libc/*/libc/fn.setpwent.html)
/// - [`endpwent`](https://docs.rs/libc/*/libc/fn.endpwent.html)
///
/// # Safety
///
/// This constructor is marked as `unsafe`, which is odd for a crate
/// that’s meant to be a safe interface. It *has* to be unsafe because
/// we cannot guarantee that the underlying C functions,
/// `getpwent`/`setpwent`/`endpwent` that iterate over the system’s
/// `passwd` entries, are called in a thread-safe manner.
///
/// These functions [modify a global
/// state](http://man7.org/linux/man-pages/man3/getpwent.3.html#ATTRIBUTES),
/// and if any are used at the same time, the state could be reset,
/// resulting in a data race. We cannot even place it behind an internal
/// `Mutex`, as there is nothing stopping another `extern` function
/// definition from calling it!
///
/// So to iterate all users, construct the iterator inside an `unsafe`
/// block, then make sure to not make a new instance of it until
/// iteration is over.
///
/// # Examples
///
/// ```
/// use users::all_users;
///
/// let iter = unsafe { all_users() };
/// for user in iter {
///     println!("User #{} ({:?})", user.uid(), user.name());
/// }
/// ```
pub unsafe fn all_users() -> impl Iterator<Item=User> {
    #[cfg(feature = "logging")]
    trace!("Running setpwent");

    #[cfg(not(target_os = "android"))]
    libc::setpwent();
    AllUsers
}

impl Drop for AllUsers {
    #[cfg(target_os = "android")]
    fn drop(&mut self) {
        // nothing to do here
    }

    #[cfg(not(target_os = "android"))]
    fn drop(&mut self) {
        #[cfg(feature = "logging")]
        trace!("Running endpwent");

        unsafe { libc::endpwent() };
    }
}

impl Iterator for AllUsers {
    type Item = User;

    #[cfg(target_os = "android")]
    fn next(&mut self) -> Option<User> {
        None
    }

    #[cfg(not(target_os = "android"))]
    fn next(&mut self) -> Option<User> {
        #[cfg(feature = "logging")]
        trace!("Running getpwent");

        let result = unsafe { libc::getpwent() };

        if result.is_null() {
            None
        }
        else {
            let user = unsafe { passwd_to_user(result.read()) };
            Some(user)
        }
    }
}



/// OS-specific extensions to users and groups.
///
/// Every OS has a different idea of what data a user or a group comes with.
/// Although they all provide a *username*, some OS’ users have an *actual name*
/// too, or a set of permissions or directories or timestamps associated with
/// them.
///
/// This module provides extension traits for users and groups that allow
/// implementors of this library to access this data *as long as a trait is
/// available*, which requires the OS they’re using to support this data.
///
/// It’s the same method taken by `Metadata` in the standard Rust library,
/// which has a few cross-platform fields and many more OS-specific fields:
/// traits in `std::os` provides access to any data that is not guaranteed to
/// be there in the actual struct.
pub mod os {

    /// Extensions to users and groups for Unix platforms.
    ///
    /// Although the `passwd` struct is common among Unix systems, its actual
    /// format can vary. See the definitions in the `base` module to check which
    /// fields are actually present.
    #[cfg(any(target_os = "linux", target_os = "android", target_os = "macos", target_os = "freebsd", target_os = "dragonfly", target_os = "openbsd", target_os = "netbsd", target_os = "solaris"))]
    pub mod unix {
        use std::ffi::{OsStr, OsString};
        use std::path::{Path, PathBuf};

        use super::super::{c_passwd, c_group, members, from_raw_buf, Group};

        /// Unix-specific extensions for `User`s.
        pub trait UserExt {

            /// Returns a path to this user’s home directory.
            fn home_dir(&self) -> &Path;

            /// Sets this user value’s home directory to the given string.
            /// Can be used to construct test users, which by default come with a
            /// dummy home directory string.
            fn with_home_dir<S: AsRef<OsStr> + ?Sized>(self, home_dir: &S) -> Self;

            /// Returns a path to this user’s shell.
            fn shell(&self) -> &Path;

            /// Sets this user’s shell path to the given string.
            /// Can be used to construct test users, which by default come with a
            /// dummy shell field.
            fn with_shell<S: AsRef<OsStr> + ?Sized>(self, shell: &S) -> Self;

            /// Returns the user’s encrypted password.
            fn password(&self) -> &OsStr;

            /// Sets this user’s password to the given string.
            /// Can be used to construct tests users, which by default come with a
            /// dummy password field.
            fn with_password<S: AsRef<OsStr> + ?Sized>(self, password: &S) -> Self;
        }

        /// Unix-specific extensions for `Group`s.
        pub trait GroupExt {

            /// Returns a slice of the list of users that are in this group as
            /// their non-primary group.
            fn members(&self) -> &[OsString];

            /// Adds a new member to this group.
            fn add_member<S: AsRef<OsStr> + ?Sized>(self, name: &S) -> Self;
        }

        /// Unix-specific fields for `User`s.
        #[derive(Clone, Debug)]
        pub struct UserExtras {

            /// The path to the user’s home directory.
            pub home_dir: PathBuf,

            /// The path to the user’s shell.
            pub shell: PathBuf,

            /// The user’s encrypted password.
            pub password: OsString,
        }

        impl Default for UserExtras {
            fn default() -> Self {
                Self {
                    home_dir: "/var/empty".into(),
                    shell:    "/bin/false".into(),
                    password: "*".into(),
                }
            }
        }

        impl UserExtras {
            /// Extract the OS-specific fields from the C `passwd` struct that
            /// we just read.
            pub(crate) unsafe fn from_passwd(passwd: c_passwd) -> Self {
                #[cfg(target_os = "android")]
                {
                    Default::default()
                }
                #[cfg(not(target_os = "android"))]
                {
                    let home_dir = from_raw_buf::<OsString>(passwd.pw_dir).into();
                    let shell    = from_raw_buf::<OsString>(passwd.pw_shell).into();
                    let password = from_raw_buf::<OsString>(passwd.pw_passwd);

                    Self { home_dir, shell, password }
                }
            }
        }

        #[cfg(any(target_os = "linux", target_os = "android", target_os = "solaris"))]
        use super::super::User;

        #[cfg(any(target_os = "linux", target_os = "android", target_os = "solaris"))]
        impl UserExt for User {
            fn home_dir(&self) -> &Path {
                Path::new(&self.extras.home_dir)
            }

            fn with_home_dir<S: AsRef<OsStr> + ?Sized>(mut self, home_dir: &S) -> Self {
                self.extras.home_dir = home_dir.into();
                self
            }

            fn shell(&self) -> &Path {
                Path::new(&self.extras.shell)
            }

            fn with_shell<S: AsRef<OsStr> + ?Sized>(mut self, shell: &S) -> Self {
                self.extras.shell = shell.into();
                self
            }

            fn password(&self) -> &OsStr {
                &self.extras.password
            }

            fn with_password<S: AsRef<OsStr> + ?Sized>(mut self, password: &S) -> Self {
                self.extras.password = password.into();
                self
            }
        }

        /// Unix-specific fields for `Group`s.
        #[derive(Clone, Default, Debug)]
        pub struct GroupExtras {

            /// Vector of usernames that are members of this group.
            pub members: Vec<OsString>,
        }

        impl GroupExtras {
            /// Extract the OS-specific fields from the C `group` struct that
            /// we just read.
            pub(crate) unsafe fn from_struct(group: c_group) -> Self {
                Self { members: members(group.gr_mem) }
            }
        }

        impl GroupExt for Group {
            fn members(&self) -> &[OsString] {
                &*self.extras.members
            }

            fn add_member<S: AsRef<OsStr> + ?Sized>(mut self, member: &S) -> Self {
                self.extras.members.push(member.into());
                self
            }
        }
    }

    /// Extensions to users and groups for BSD platforms.
    ///
    /// These platforms have `change` and `expire` fields in their `passwd`
    /// C structs.
    #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "dragonfly", target_os = "openbsd", target_os = "netbsd"))]
    pub mod bsd {
        use std::ffi::OsStr;
        use std::path::Path;
        use libc::time_t;
        use super::super::{c_passwd, User};

        /// BSD-specific fields for `User`s.
        #[derive(Clone, Debug)]
        pub struct UserExtras {

            /// Fields specific to Unix, rather than just BSD. (This struct is
            /// a superset, so it has to have all the other fields in it, too).
            pub extras: super::unix::UserExtras,

            /// Password change time.
            pub change: time_t,

            /// Password expiry time.
            pub expire: time_t,
        }

        impl UserExtras {
            /// Extract the OS-specific fields from the C `passwd` struct that
            /// we just read.
            pub(crate) unsafe fn from_passwd(passwd: c_passwd) -> Self {
                Self {
                    change: passwd.pw_change,
                    expire: passwd.pw_expire,
                    extras: super::unix::UserExtras::from_passwd(passwd),
                }
            }
        }

        impl super::unix::UserExt for User {
            fn home_dir(&self) -> &Path {
                Path::new(&self.extras.extras.home_dir)
            }

            fn with_home_dir<S: AsRef<OsStr> + ?Sized>(mut self, home_dir: &S) -> Self {
                self.extras.extras.home_dir = home_dir.into();
                self
            }

            fn shell(&self) -> &Path {
                Path::new(&self.extras.extras.shell)
            }

            fn with_shell<S: AsRef<OsStr> + ?Sized>(mut self, shell: &S) -> Self {
                self.extras.extras.shell = shell.into();
                self
            }

            fn password(&self) -> &OsStr {
                &self.extras.extras.password
            }

            fn with_password<S: AsRef<OsStr> + ?Sized>(mut self, password: &S) -> Self {
                self.extras.extras.password = password.into();
                self
            }
        }

        /// BSD-specific accessors for `User`s.
        pub trait UserExt {

            /// Returns this user’s password change timestamp.
            fn password_change_time(&self) -> time_t;

            /// Returns this user’s password expiry timestamp.
            fn password_expire_time(&self) -> time_t;
        }

        impl UserExt for User {
            fn password_change_time(&self) -> time_t {
                self.extras.change
            }

            fn password_expire_time(&self) -> time_t {
                self.extras.expire
            }
        }

        impl Default for UserExtras {
            fn default() -> Self {
                Self {
                    extras: super::unix::UserExtras::default(),
                    change: 0,
                    expire: 0,
                }
            }
        }
    }

    /// Any extra fields on a `User` specific to the current platform.
    #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "dragonfly", target_os = "openbsd", target_os = "netbsd"))]
    pub type UserExtras = bsd::UserExtras;

    /// Any extra fields on a `User` specific to the current platform.
    #[cfg(any(target_os = "linux", target_os = "android", target_os = "solaris"))]
    pub type UserExtras = unix::UserExtras;

    /// Any extra fields on a `Group` specific to the current platform.
    #[cfg(any(target_os = "linux", target_os = "android", target_os = "macos", target_os = "freebsd", target_os = "dragonfly", target_os = "openbsd", target_os = "netbsd", target_os = "solaris"))]
    pub type GroupExtras = unix::GroupExtras;
}


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn uid() {
        get_current_uid();
    }

    #[test]
    fn username() {
        let uid = get_current_uid();
        assert_eq!(&*get_current_username().unwrap(), &*get_user_by_uid(uid).unwrap().name());
    }

    #[test]
    fn uid_for_username() {
        let uid = get_current_uid();
        let user = get_user_by_uid(uid).unwrap();
        assert_eq!(user.uid, uid);
    }

    #[test]
    fn username_for_uid_for_username() {
        let uid = get_current_uid();
        let user = get_user_by_uid(uid).unwrap();
        let user2 = get_user_by_uid(user.uid).unwrap();
        assert_eq!(user2.uid, uid);
    }

    #[test]
    fn user_info() {
        use base::os::unix::UserExt;

        let uid = get_current_uid();
        let user = get_user_by_uid(uid).unwrap();
        // Not a real test but can be used to verify correct results
        // Use with --nocapture on test executable to show output
        println!("HOME={:?}, SHELL={:?}, PASSWD={:?}",
            user.home_dir(), user.shell(), user.password());
    }

    #[test]
    fn user_by_name() {
        // We cannot really test for arbitrary user as they might not exist on the machine
        // Instead the name of the current user is used
        let name = get_current_username().unwrap();
        let user_by_name = get_user_by_name(&name);
        assert!(user_by_name.is_some());
        assert_eq!(user_by_name.unwrap().name(), &*name);

        // User names containing '\0' cannot be used (for now)
        let user = get_user_by_name("user\0");
        assert!(user.is_none());
    }

    #[test]
    fn user_get_groups() {
        let uid = get_current_uid();
        let user = get_user_by_uid(uid).unwrap();
        let groups = user.groups().unwrap();
        println!("Groups: {:?}", groups);
        assert!(groups.len() > 0);
    }

    #[test]
    fn group_by_name() {
        // We cannot really test for arbitrary groups as they might not exist on the machine
        // Instead the primary group of the current user is used
        let cur_uid = get_current_uid();
        let cur_user = get_user_by_uid(cur_uid).unwrap();
        let cur_group = get_group_by_gid(cur_user.primary_group).unwrap();
        let group_by_name = get_group_by_name(&cur_group.name());

        assert!(group_by_name.is_some());
        assert_eq!(group_by_name.unwrap().name(), cur_group.name());

        // Group names containing '\0' cannot be used (for now)
        let group = get_group_by_name("users\0");
        assert!(group.is_none());
    }
}
