//! `redox-users` is designed to be a small, low-ish level interface
//! to system user and group information, as well as user password
//! authentication.
//!
//! # Permissions
//! Because this is a system level tool dealing with password
//! authentication, programs are often required to run with
//! escalated priveleges. The implementation of the crate is
//! privelege unaware. The only privelege requirements are those
//! laid down by the system administrator over these files:
//! - `/etc/group`
//!   - Read: Required to access group information
//!   - Write: Required to change group information
//! - `/etc/passwd`
//!   - Read: Required to access user information
//!   - Write: Required to change user information
//! - `/etc/shadow`
//!   - Read: Required to authenticate users
//!   - Write: Required to set user passwords
//!
//! # Reimplementation
//! This crate is designed to be as small as possible without
//! sacrificing critical functionality. The idea is that a small
//! enough redox-users will allow easy re-implementation based on
//! the same flexible API. This would allow more complicated authentication
//! schemes for redox in future without breakage of existing
//! software.

#[cfg(feature = "auth")]
extern crate argon2;
extern crate getrandom;
extern crate syscall;

use std::convert::From;
use std::error::Error;
use std::fmt::{self, Debug, Display};
use std::fs::OpenOptions;
use std::io::{Read, Write};
#[cfg(target_os = "redox")]
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::slice::{Iter, IterMut};
use std::str::FromStr;
#[cfg(not(test))]
#[cfg(feature = "auth")]
use std::thread;
use std::time::Duration;

#[cfg(target_os = "redox")]
use syscall::flag::{O_EXLOCK, O_SHLOCK};
use syscall::Error as SyscallError;

const PASSWD_FILE: &'static str = "/etc/passwd";
const GROUP_FILE: &'static str = "/etc/group";
const SHADOW_FILE: &'static str = "/etc/shadow";

#[cfg(target_os = "redox")]
const DEFAULT_SCHEME: &'static str = "file:";
#[cfg(not(target_os = "redox"))]
const DEFAULT_SCHEME: &'static str = "";

const MIN_ID: usize = 1000;
const MAX_ID: usize = 6000;
const DEFAULT_TIMEOUT: u64 = 3;

pub type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// Errors that might happen while using this crate
#[derive(Debug, PartialEq)]
pub enum UsersError {
    Os { reason: String },
    Parsing { reason: String },
    NotFound,
    AlreadyExists
}

impl fmt::Display for UsersError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UsersError::Os { reason } => write!(f, "os error: code {}", reason),
            UsersError::Parsing { reason } => {
                write!(f, "parse error: {}", reason)
            },
            UsersError::NotFound => write!(f, "user/group not found"),
            UsersError::AlreadyExists => write!(f, "user/group already exists")
        }
    }
}

impl Error for UsersError {
    fn description(&self) -> &str { "UsersError" }

    fn cause(&self) -> Option<&dyn Error> { None }
}

#[inline]
fn parse_error(reason: &str) -> UsersError {
    UsersError::Parsing {
        reason: reason.into()
    }
}

#[inline]
fn os_error(reason: &str) -> UsersError {
    UsersError::Os {
        reason: reason.into()
    }
}

impl From<SyscallError> for UsersError {
    fn from(syscall_error: SyscallError) -> UsersError {
        UsersError::Os {
            reason: format!("{}", syscall_error)
        }
    }
}

fn read_locked_file(file: impl AsRef<Path>) -> Result<String> {
    #[cfg(test)]
    println!("Reading file: {}", file.as_ref().display());

    #[cfg(target_os = "redox")]
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(O_SHLOCK as i32)
        .open(file)?;
    #[cfg(not(target_os = "redox"))]
    #[cfg_attr(rustfmt, rustfmt_skip)]
    let mut file = OpenOptions::new()
        .read(true)
        .open(file)?;

    let len = file.metadata()?.len();
    let mut file_data = String::with_capacity(len as usize);
    file.read_to_string(&mut file_data)?;
    Ok(file_data)
}

fn write_locked_file(file: impl AsRef<Path>, data: String) -> Result<()> {
    #[cfg(test)]
    println!("Writing file: {}", file.as_ref().display());

    #[cfg(target_os = "redox")]
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .custom_flags(O_EXLOCK as i32)
        .open(file)?;
    #[cfg(not(target_os = "redox"))]
    #[cfg_attr(rustfmt, rustfmt_skip)]
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(file)?;

    file.write(data.as_bytes())?;
    Ok(())
}

/// A struct representing a Redox user.
/// Currently maps to an entry in the `/etc/passwd` file.
///
/// # Unset vs. Blank Passwords
/// A note on unset passwords vs. blank passwords. A blank password
/// is a hash field that is completely blank (aka, `""`). According
/// to this crate, successful login is only allowed if the input
/// password is blank as well.
///
/// An unset password is one whose hash is not empty (`""`), but
/// also not a valid serialized argon2rs hashing session. This
/// hash always returns `false` upon attempted verification. The
/// most commonly used hash for an unset password is `"!"`, but
/// this crate makes no distinction. The most common way to unset
/// the password is to use [`unset_passwd`](struct.User.html#method.unset_passwd).
pub struct User {
    /// Username (login name)
    pub user: String,
    // Hashed password and Argon2 indicator, stored to simplify API
    hash: Option<(String, bool)>,
    /// User id
    pub uid: usize,
    /// Group id
    pub gid: usize,
    /// Real name (GECOS field)
    pub name: String,
    /// Home directory path
    pub home: String,
    /// Shell path
    pub shell: String,
    /// Failed login delay duration
    auth_delay: Duration
}

impl User {
    /// Set the password for a user. Make sure the password you have
    /// received is actually what the user wants as their password (this doesn't).
    ///
    /// To set the password blank, use `""` as the password parameter.
    ///
    /// # Panics
    /// If the User's hash fields are unpopulated, this function will `panic!`
    /// (see [`AllUsers`](struct.AllUsers.html#shadowfile-handling) for more info).
    #[cfg(feature = "auth")]
    pub fn set_passwd(&mut self, password: impl AsRef<str>) -> Result<()> {
        self.panic_if_unpopulated();
        let password = password.as_ref();

        self.hash = if password != "" {
            let mut buf = [0u8; 8];
            getrandom::getrandom(&mut buf)?;
            let salt = format!("{:X}", u64::from_ne_bytes(buf));
            let config = argon2::Config::default();
            let hash = argon2::hash_encoded(
                password.as_bytes(),
                salt.as_bytes(),
                &config
            )?;
            Some((hash, true))
        } else {
            Some(("".into(), false))
        };
        Ok(())
    }

    /// Unset the password (do not allow logins).
    ///
    /// # Panics
    /// If the User's hash fields are unpopulated, this function will `panic!`
    /// (see [`AllUsers`](struct.AllUsers.html#shadowfile-handling) for more info).
    pub fn unset_passwd(&mut self) {
        self.panic_if_unpopulated();
        self.hash = Some(("!".into(), false));
    }

    /// Verify the password. If the hash is empty, this only
    /// returns `true` if the password field is also empty.
    /// Note that this is a blocking operation if the password
    /// is incorrect. See [`Config::auth_delay`](struct.Config.html#method.auth_delay)
    /// to set the wait time. Default is 3 seconds.
    ///
    /// # Panics
    /// If the User's hash fields are unpopulated, this function will `panic!`
    /// (see [`AllUsers`](struct.AllUsers.html#shadowfile-handling) for more info).
    #[cfg(feature = "auth")]
    pub fn verify_passwd(&self, password: impl AsRef<str>) -> bool {
        self.panic_if_unpopulated();
        // Safe because it will have panicked already if self.hash.is_none()
        let &(ref hash, ref encoded) = self.hash.as_ref().unwrap();
        let password = password.as_ref();

        let verified = if *encoded {
            argon2::verify_encoded(&hash, password.as_bytes()).unwrap()
        } else {
            hash == "" && password == ""
        };

        if !verified {
            #[cfg(not(test))] // Make tests run faster
            thread::sleep(self.auth_delay);
        }
        verified
    }

    /// Determine if the hash for the password is blank
    /// (any user can log in as this user with no password).
    ///
    /// # Panics
    /// If the User's hash fields are unpopulated, this function will `panic!`
    /// (see [`AllUsers`](struct.AllUsers.html#shadowfile-handling) for more info).
    pub fn is_passwd_blank(&self) -> bool {
        self.panic_if_unpopulated();
        let &(ref hash, ref encoded) = self.hash.as_ref().unwrap();
        hash == "" && ! encoded
    }

    /// Determine if the hash for the password is unset
    /// ([`verify_passwd`](struct.User.html#method.verify_passwd)
    /// returns `false` regardless of input).
    ///
    /// # Panics
    /// If the User's hash fields are unpopulated, this function will `panic!`
    /// (see [`AllUsers`](struct.AllUsers.html#shadowfile-handling) for more info).
    pub fn is_passwd_unset(&self) -> bool {
        self.panic_if_unpopulated();
        let &(ref hash, ref encoded) = self.hash.as_ref().unwrap();
        hash != "" && ! encoded
    }

    /// Get a Command to run the user's default shell
    /// (see [`login_cmd`](struct.User.html#method.login_cmd) for more docs).
    pub fn shell_cmd(&self) -> Command { self.login_cmd(&self.shell) }

    /// Provide a login command for the user, which is any
    /// entry point for starting a user's session, whether
    /// a shell (use [`shell_cmd`](struct.User.html#method.shell_cmd) instead) or a graphical init.
    ///
    /// The `Command` will use the user's `uid` and `gid`, its `current_dir` will be
    /// set to the user's home directory, and the follwing enviroment variables will
    /// be populated:
    ///
    ///    - `USER` set to the user's `user` field.
    ///    - `UID` set to the user's `uid` field.
    ///    - `GROUPS` set the user's `gid` field.
    ///    - `HOME` set to the user's `home` field.
    ///    - `SHELL` set to the user's `shell` field.
    pub fn login_cmd<T>(&self, cmd: T) -> Command
        where T: std::convert::AsRef<std::ffi::OsStr> + AsRef<str>
    {
        let mut command = Command::new(cmd);
        command
            .uid(self.uid as u32)
            .gid(self.gid as u32)
            .current_dir(&self.home)
            .env("USER", &self.user)
            .env("UID", format!("{}", self.uid))
            .env("GROUPS", format!("{}", self.gid))
            .env("HOME", &self.home)
            .env("SHELL", &self.shell);
        command
    }

    /// This returns an entry for `/etc/shadow`
    /// Will panic!
    fn shadowstring(&self) -> String {
        self.panic_if_unpopulated();
        let hashstring = match self.hash {
            Some((ref hash, _)) => hash,
            None => panic!("Shadowfile not read!")
        };
        format!("{};{}", self.user, hashstring)
    }

    /// Give this a hash string (not a shadowfile entry!!!)
    fn populate_hash(&mut self, hash: &str) -> Result<()> {
        let encoded = match hash {
            "" => false,
            "!" => false,
            _ => true,
        };
        self.hash = Some((hash.to_string(), encoded));
        Ok(())
    }

    #[inline]
    fn panic_if_unpopulated(&self) {
        if self.hash.is_none() {
            panic!("Hash not populated!");
        }
    }
}

impl Name for User {
    fn name(&self) -> &str {
        &self.user
    }
}

impl Id for User {
    fn id(&self) -> usize {
        self.uid
    }
}

impl Debug for User {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,
            "User {{\n\tuser: {:?}\n\tuid: {:?}\n\tgid: {:?}\n\tname: {:?}
            home: {:?}\n\tshell: {:?}\n\tauth_delay: {:?}\n}}",
            self.user, self.uid, self.gid, self.name, self.home, self.shell, self.auth_delay
        )
    }
}

impl Display for User {
    /// Format this user as an entry in `/etc/passwd`. This
    /// is an implementation detail, do NOT rely on this trait
    /// being implemented in future.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        #[cfg_attr(rustfmt, rustfmt_skip)]
        write!(f, "{};{};{};{};{};{}",
            self.user, self.uid, self.gid, self.name, self.home, self.shell
        )
    }
}

impl FromStr for User {
    type Err = Box<dyn Error + Send + Sync>;

    /// Parse an entry from `/etc/passwd`. This
    /// is an implementation detail, do NOT rely on this trait
    /// being implemented in future.
    fn from_str(s: &str) -> Result<Self> {
        let mut parts = s.split(';');

        let user = parts
            .next()
            .ok_or(parse_error("expected user"))?;
        let uid = parts
            .next()
            .ok_or(parse_error("expected uid"))?
            .parse::<usize>()?;
        let gid = parts
            .next()
            .ok_or(parse_error("expected uid"))?
            .parse::<usize>()?;
        let name = parts
            .next()
            .ok_or(parse_error("expected real name"))?;
        let home = parts
            .next()
            .ok_or(parse_error("expected home dir path"))?;
        let shell = parts
            .next()
            .ok_or(parse_error("expected shell path"))?;

        Ok(User {
            user: user.into(),
            hash: None,
            uid,
            gid,
            name: name.into(),
            home: home.into(),
            shell: shell.into(),
            auth_delay: Duration::default(),
        })
    }
}

/// A struct representing a Redox user group.
/// Currently maps to an `/etc/group` file entry.
#[derive(Debug)]
pub struct Group {
    /// Group name
    pub group: String,
    /// Unique group id
    pub gid: usize,
    /// Group members usernames
    pub users: Vec<String>,
}

impl Name for Group {
    fn name(&self) -> &str {
        &self.group
    }
}

impl Id for Group {
    fn id(&self) -> usize {
        self.gid
    }
}

impl Display for Group {
    /// Format this group as an entry in `/etc/group`. This
    /// is an implementation detail, do NOT rely on this trait
    /// being implemented in future.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        #[cfg_attr(rustfmt, rustfmt_skip)]
        write!(f, "{};{};{}",
            self.group,
            self.gid,
            self.users.join(",").trim_matches(',')
        )
    }
}

impl FromStr for Group {
    type Err = Box<dyn Error + Send + Sync>;

    /// Parse an entry from `/etc/group`. This
    /// is an implementation detail, do NOT rely on this trait
    /// being implemented in future.
    fn from_str(s: &str) -> Result<Self> {
        let mut parts = s.split(';');

        let group = parts
            .next()
            .ok_or(parse_error("expected group"))?;
        let gid = parts
            .next()
            .ok_or(parse_error("expected gid"))?
            .parse::<usize>()?;
        //Allow for an empty users field. If there is a better way to do this, do it
        let users_str = parts.next().unwrap_or(" ");
        let users = users_str.split(',').map(|u| u.into()).collect();

        Ok(Group {
            group: group.into(),
            gid,
            users,
        })
    }
}

/// Gets the current process effective user ID.
///
/// This function issues the `geteuid` system call returning the process effective
/// user id.
///
/// # Examples
///
/// Basic usage:
///
/// ```no_run
/// # use redox_users::get_euid;
/// let euid = get_euid().unwrap();
/// ```
pub fn get_euid() -> Result<usize> {
    match syscall::geteuid() {
        Ok(euid) => Ok(euid),
        Err(syscall_error) => Err(From::from(os_error(syscall_error.text())))
    }
}

/// Gets the current process real user ID.
///
/// This function issues the `getuid` system call returning the process real
/// user id.
///
/// # Examples
///
/// Basic usage:
///
/// ```no_run
/// # use redox_users::get_uid;
/// let uid = get_uid().unwrap();
/// ```
pub fn get_uid() -> Result<usize> {
    match syscall::getuid() {
        Ok(uid) => Ok(uid),
        Err(syscall_error) => Err(From::from(os_error(syscall_error.text())))
    }
}

/// Gets the current process effective group ID.
///
/// This function issues the `getegid` system call returning the process effective
/// group id.
///
/// # Examples
///
/// Basic usage:
///
/// ```no_run
/// # use redox_users::get_egid;
/// let egid = get_egid().unwrap();
/// ```
pub fn get_egid() -> Result<usize> {
    match syscall::getegid() {
        Ok(egid) => Ok(egid),
        Err(syscall_error) => Err(From::from(os_error(syscall_error.text())))
    }
}

/// Gets the current process real group ID.
///
/// This function issues the `getegid` system call returning the process real
/// group id.
///
/// # Examples
///
/// Basic usage:
///
/// ```no_run
/// # use redox_users::get_gid;
/// let gid = get_gid().unwrap();
/// ```
pub fn get_gid() -> Result<usize> {
    match syscall::getgid() {
        Ok(gid) => Ok(gid),
        Err(syscall_error) => Err(From::from(os_error(syscall_error.text())))
    }
}

/// A generic configuration that allows better control of
/// `AllUsers` or `AllGroups` than might otherwise be possible.
///
/// The use of the fields of this struct is completely optional
/// depending on what constructor it is passed to. For example,
/// `AllGroups` doesn't care if auth is enabled or not, or what
/// the duration is.
///
/// In most situations, `Config::default()` will work just fine.
/// The other methods on this struct are usually for finer control
/// of an `AllUsers` or `AllGroups` if it is required.
#[derive(Clone)]
pub struct Config {
    auth_enabled: bool,
    scheme: String,
    auth_delay: Duration,
    min_id: usize,
    max_id: usize,
}

impl Config {
    /// An alternative to the default constructor, this indicates that
    /// authentication should be enabled.
    pub fn with_auth() -> Config {
        Config {
            auth_enabled: true,
            ..Default::default()
        }
    }

    /// Builder pattern version of `Self::with_auth`.
    pub fn auth(mut self, auth: bool) -> Config {
        self.auth_enabled = auth;
        self
    }

    /// Set the delay for a failed authentication. Default is 3 seconds.
    pub fn auth_delay(mut self, delay: Duration) -> Config {
        self.auth_delay = delay;
        self
    }

    /// Set the smallest ID possible to use when finding an unused ID.
    pub fn min_id(mut self, id: usize) -> Config {
        self.min_id = id;
        self
    }

    /// Set the largest possible ID to use when finding an unused ID.
    pub fn max_id(mut self, id: usize) -> Config {
        self.max_id = id;
        self
    }

    /// Set the scheme relative to which the `AllUsers` or `AllGroups`
    /// should be looking for its data files. This is a compromise between
    /// exposing implementation details and providing fine enough
    /// control over the behavior of this API.
    pub fn scheme(mut self, scheme: String) -> Config {
        self.scheme = scheme;
        self
    }

    // Prepend a path with the scheme in this Config
    fn in_scheme(&self, path: impl AsRef<Path>) -> PathBuf {
        let mut canonical_path = PathBuf::from(&self.scheme);
        // Should be a little careful here, not sure I want this behavior
        if path.as_ref().is_absolute() {
            // This is nasty
            canonical_path.push(path.as_ref().to_string_lossy()[1..].to_string());
        } else {
            canonical_path.push(path);
        }
        canonical_path
    }
}

impl Default for Config {
    /// Authentication is not enabled; The default base scheme is `file`.
    fn default() -> Config {
        Config {
            auth_enabled: false,
            scheme: String::from(DEFAULT_SCHEME),
            auth_delay: Duration::new(DEFAULT_TIMEOUT, 0),
            min_id: MIN_ID,
            max_id: MAX_ID,
        }
    }
}

// Nasty hack to prevent the compiler complaining about
// "leaking" `AllInner`
mod sealed {
    use Config;

    pub trait Name {
        fn name(&self) -> &str;
    }

    pub trait Id {
        fn id(&self) -> usize;
    }

    pub trait AllInner {
        // Group+User, thanks Dad
        type Gruser: Name + Id;

        /// These functions grab internal elements so that the other
        /// methods of `All` can manipulate them.
        fn list(&self) -> &Vec<Self::Gruser>;
        fn list_mut(&mut self) -> &mut Vec<Self::Gruser>;
        fn config(&self) -> &Config;
    }
}

use sealed::{AllInner, Id, Name};

/// This trait is used to remove repetitive API items from
/// [`AllGroups`] and [`AllUsers`]. It uses a hidden trait
/// so that the implementations of functions can be implemented
/// at the trait level. Do not try to implement this trait.
pub trait All: AllInner {
    /// Get an iterator borrowing all [`User`](struct.User.html)'s
    /// or [`Group`](struct.Group.html)'s on the system.
    fn iter(&self) -> Iter<<Self as AllInner>::Gruser> {
        self.list().iter()
    }

    /// Get an iterator mutably borrowing all [`User`](struct.User.html)'s
    /// or [`Group`](struct.Group.html)'s on the system.
    fn iter_mut(&mut self) -> IterMut<<Self as AllInner>::Gruser> {
        self.list_mut().iter_mut()
    }

    /// Borrow the [`User`](struct.User.html) or [`Group`](struct.Group.html)
    /// with a given name.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// # use redox_users::{All, AllUsers, Config};
    /// let users = AllUsers::new(Config::default()).unwrap();
    /// let user = users.get_by_name("root").unwrap();
    /// ```
    fn get_by_name(&self, name: impl AsRef<str>) -> Option<&<Self as AllInner>::Gruser> {
        self.iter()
            .find(|gruser| gruser.name() == name.as_ref() )
    }

    /// Mutable version of [`get_by_name`](trait.All.html#method.get_by_name).
    fn get_mut_by_name(&mut self, name: impl AsRef<str>) -> Option<&mut <Self as AllInner>::Gruser> {
        self.iter_mut()
            .find(|gruser| gruser.name() == name.as_ref() )
    }

    /// Borrow the [`User`](struct.User.html) or [`Group`](struct.Group.html)
    /// with the given ID.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```no_run
    /// # use redox_users::{All, AllUsers, Config};
    /// let users = AllUsers::new(Config::default()).unwrap();
    /// let user = users.get_by_id(0).unwrap();
    /// ```
    fn get_by_id(&self, id: usize) -> Option<&<Self as AllInner>::Gruser> {
        self.iter()
            .find(|gruser| gruser.id() == id )
    }

    /// Mutable version of [`get_by_id`](trait.All.html#method.get_by_id).
    fn get_mut_by_id(&mut self, id: usize) -> Option<&mut <Self as AllInner>::Gruser> {
        self.iter_mut()
            .find(|gruser| gruser.id() == id )
    }

    /// Provides an unused id based on the min and max values in
    /// the [`Config`](struct.Config.html) passed to the `All`'s constructor.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use redox_users::{All, AllUsers, Config};
    /// let users = AllUsers::new(Config::default()).unwrap();
    /// let uid = users.get_unique_id().expect("no available uid");
    /// ```
    fn get_unique_id(&self) -> Option<usize> {
        for id in self.config().min_id..self.config().max_id {
            if !self.iter().any(|gruser| gruser.id() == id ) {
                return Some(id)
            }
        }
        None
    }

    /// Remove a [`User`](struct.User.html) or [`Group`](struct.Group.html)
    /// from this `All` given it's name. This won't provide an indication
    /// of whether the user was removed or not, but is guaranteed to work
    /// if a user with the specified name exists.
    fn remove_by_name(&mut self, name: impl AsRef<str>) {
        // Significantly more elegant than other possible solutions.
        // I wish it could indicate if it removed anything.
        self.list_mut()
            .retain(|gruser| gruser.name() != name.as_ref() );
    }

    /// Id version of [`remove_by_name`](trait.All.html#method.remove_by_name).
    fn remove_by_id(&mut self, id: usize) {
        self.list_mut()
            .retain(|gruser| gruser.id() != id );
    }
}

/// [`AllUsers`](struct.AllUsers.html) provides
/// (borrowed) access to all the users on the system.
/// Note that this struct implements [`All`](trait.All.html) for
/// a bunch of convenient access functions.
///
/// # Notes
/// Note that everything in this section also applies to
/// [`AllGroups`](struct.AllGroups.html)
///
/// * If you mutate anything owned by an `AllUsers`,
///   you must call the [`save`](struct.AllUsers.html#method.save)
///   method in order for those changes to be applied to the system.
/// * The API here is kept small on purpose in order to reduce the
///   surface area for security exploitation. Most mutating actions
///   can be accomplished via the [`get_mut_by_id`](struct.AllUsers.html#method.get_mut_by_id)
///   and [`get_mut_by_name`](struct.AllUsers.html#method.get_mut_by_name)
///   functions.
///
/// # Shadowfile handling
/// This implementation of redox-users uses a shadowfile implemented primarily
/// by this struct. `AllUsers` respects the `auth_enabled` status of the `Config`
/// that is was passed. If auth is enabled, it populates the
/// hash fields of each user struct that it parses from `/etc/passwd` with
/// info from `/et/shadow`. If a caller attempts to perform an action that
/// requires this info with an `AllUsers` config that does not have auth enabled,
/// the `User` handling action will panic.
pub struct AllUsers {
    users: Vec<User>,
    config: Config,
}

impl AllUsers {
    /// See [Shadowfile Handling](struct.AllUsers.html#shadowfile-handling) for
    /// configuration information regarding this constructor.
    //TODO: Indicate if parsing an individual line failed or not
    pub fn new(config: Config) -> Result<AllUsers> {
        let passwd_cntnt = read_locked_file(config.in_scheme(PASSWD_FILE))?;

        let mut passwd_entries: Vec<User> = Vec::new();
        for line in passwd_cntnt.lines() {
            if let Ok(mut user) = User::from_str(line) {
                user.auth_delay = config.auth_delay;
                passwd_entries.push(user);
            }
        }

        if config.auth_enabled {
            let shadow_cntnt = read_locked_file(config.in_scheme(SHADOW_FILE))?;
            let shadow_entries: Vec<&str> = shadow_cntnt.lines().collect();
            for entry in shadow_entries.iter() {
                let mut entry = entry.split(';');
                let name = entry.next().ok_or(parse_error(
                    "error parsing shadowfile: expected username"
                ))?;
                let hash = entry.next().ok_or(parse_error(
                    "error parsing shadowfile: expected hash"
                ))?;
                passwd_entries
                    .iter_mut()
                    .find(|user| user.user == name)
                    .ok_or(parse_error(
                        "error parsing shadowfile: unkown user"
                    ))?
                    .populate_hash(hash)?;
            }
        }

        Ok(AllUsers {
            users: passwd_entries,
            config
        })
    }

    /// Adds a user with the specified attributes to the `AllUsers`
    /// instance. Note that the user's password is set unset (see
    /// [Unset vs Blank Passwords](struct.User.html#unset-vs-blank-passwords))
    /// during this call.
    ///
    /// This function is classified as a mutating operation,
    /// and users must therefore call [`save`](struct.AllUsers.html#method.save)
    /// in order for the new user to be applied to the system.
    ///
    /// # Panics
    /// This function will `panic!` if the [`Config`](struct.Config.html)
    /// passed to [`AllUsers::new`](struct.AllUsers.html#method.new)
    /// does not have authentication enabled (see
    /// [`Shadowfile handling`](struct.AllUsers.html#shadowfile-handling)).
    //TODO: Take uid/gid as Option<usize> and if none, find an unused ID.
    pub fn add_user(
        &mut self,
        login: &str,
        uid: usize,
        gid: usize,
        name: &str,
        home: &str,
        shell: &str
    ) -> Result<()> {
        if self.iter()
            .any(|user| user.user == login || user.uid == uid)
        {
            return Err(From::from(UsersError::AlreadyExists))
        }

        if !self.config.auth_enabled {
            panic!("Attempt to create user without access to the shadowfile");
        }

        self.users.push(User {
            user: login.into(),
            hash: Some(("!".into(), false)),
            uid,
            gid,
            name: name.into(),
            home: home.into(),
            shell: shell.into(),
            auth_delay: self.config.auth_delay
        });
        Ok(())
    }

    /// Syncs the data stored in the `AllUsers` instance to the filesystem.
    /// To apply changes to the system from an `AllUsers`, you MUST call this function!
    pub fn save(&self) -> Result<()> {
        let mut userstring = String::new();
        let mut shadowstring = String::new();
        for user in &self.users {
            userstring.push_str(&format!("{}\n", user.to_string().as_str()));
            if self.config.auth_enabled {
                shadowstring.push_str(&format!("{}\n", user.shadowstring()));
            }
        }

        write_locked_file(self.config.in_scheme(PASSWD_FILE), userstring)?;
        if self.config.auth_enabled {
            write_locked_file(self.config.in_scheme(SHADOW_FILE), shadowstring)?;
        }
        Ok(())
    }
}

impl AllInner for AllUsers {
    type Gruser = User;

    fn list(&self) -> &Vec<Self::Gruser> {
        &self.users
    }

    fn list_mut(&mut self) -> &mut Vec<Self::Gruser> {
        &mut self.users
    }

    fn config(&self) -> &Config {
        &self.config
    }
}

impl All for AllUsers {}

impl Debug for AllUsers {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "AllUsers {{\nusers: {:?}\n}}", self.users)
    }
}

/// [`AllGroups`](struct.AllGroups.html) provides
/// (borrowed) access to all groups on the system. Note that this
/// struct implements [`All`](trait.All.html), for a bunch of convenience
/// functions.
///
/// General notes that also apply to this struct may be found with
/// [`AllUsers`](struct.AllUsers.html).
pub struct AllGroups {
    groups: Vec<Group>,
    config: Config,
}

impl AllGroups {
    /// Create a new `AllGroups`.
    //TODO: Indicate if parsing an individual line failed or not
    pub fn new(config: Config) -> Result<AllGroups> {
        let group_cntnt = read_locked_file(config.in_scheme(GROUP_FILE))?;

        let mut entries: Vec<Group> = Vec::new();
        for line in group_cntnt.lines() {
            if let Ok(group) = Group::from_str(line) {
                entries.push(group);
            }
        }

        Ok(AllGroups {
            groups: entries,
            config,
        })
    }

    /// Adds a group with the specified attributes to this `AllGroups`.
    ///
    /// This function is classified as a mutating operation,
    /// and users must therefore call [`save`](struct.AllGroups.html#method.save)
    /// in order for the new group to be applied to the system.
    //TODO: Take Option<usize> for gid and find unused ID if None
    pub fn add_group(
        &mut self,
        name: &str,
        gid: usize,
        users: &[&str]
    ) -> Result<()> {
        if self.iter()
            .any(|group| group.group == name || group.gid == gid)
        {
            return Err(From::from(UsersError::AlreadyExists))
        }

        //Might be cleaner... Also breaks...
        //users: users.iter().map(String::to_string).collect()
        self.groups.push(Group {
            group: name.into(),
            gid,
            users: users
                .iter()
                .map(|user| user.to_string())
                .collect()
        });

        Ok(())
    }

    /// Syncs the data stored in this `AllGroups` instance to the filesystem.
    /// To apply changes from an `AllGroups`, you MUST call this function!
    pub fn save(&self) -> Result<()> {
        let mut groupstring = String::new();
        for group in &self.groups {
            groupstring.push_str(&format!("{}\n", group.to_string().as_str()));
        }

        write_locked_file(self.config.in_scheme(GROUP_FILE), groupstring)
    }
}

impl AllInner for AllGroups {
    type Gruser = Group;

    fn list(&self) -> &Vec<Self::Gruser> {
        &self.groups
    }

    fn list_mut(&mut self) -> &mut Vec<Self::Gruser> {
        &mut self.groups
    }

    fn config(&self) -> &Config {
        &self.config
    }
}

impl All for AllGroups {}

#[cfg(test)]
mod test {
    use super::*;

    const TEST_PREFIX: &'static str = "tests";

    /// Needed for the file checks, this is done by the library
    fn test_prefix(filename: &str) -> String {
        let mut complete = String::from(TEST_PREFIX);
        complete.push_str(filename);
        complete
    }

    fn test_cfg() -> Config {
        Config::default()
            // Since all this really does is prepend `sheme` to the consts
            .scheme(TEST_PREFIX.to_string())
    }

    fn test_auth_cfg() -> Config {
        test_cfg().auth(true)
    }

    // *** struct.User ***
    #[cfg(feature = "auth")]
    #[test]
    #[should_panic(expected = "Hash not populated!")]
    fn wrong_attempt_set_password() {
        let mut users = AllUsers::new(test_cfg()).unwrap();
        let user = users.get_mut_by_id(1000).unwrap();
        user.set_passwd("").unwrap();
    }

    #[test]
    #[should_panic(expected = "Hash not populated!")]
    fn wrong_attempt_unset_password() {
        let mut users = AllUsers::new(test_cfg()).unwrap();
        let user = users.get_mut_by_id(1000).unwrap();
        user.unset_passwd();
    }

    #[cfg(feature = "auth")]
    #[test]
    #[should_panic(expected = "Hash not populated!")]
    fn wrong_attempt_verify_password() {
        let mut users = AllUsers::new(test_cfg()).unwrap();
        let user = users.get_mut_by_id(1000).unwrap();
        user.verify_passwd("hi folks");
    }

    #[test]
    #[should_panic(expected = "Hash not populated!")]
    fn wrong_attempt_is_password_blank() {
        let mut users = AllUsers::new(test_cfg()).unwrap();
        let user = users.get_mut_by_id(1000).unwrap();
        user.is_passwd_blank();
    }

    #[test]
    #[should_panic(expected = "Hash not populated!")]
    fn wrong_attempt_is_password_unset() {
        let mut users = AllUsers::new(test_cfg()).unwrap();
        let user = users.get_mut_by_id(1000).unwrap();
        user.is_passwd_unset();
    }

    #[cfg(feature = "auth")]
    #[test]
    fn attempt_user_api() {
        let mut users = AllUsers::new(test_auth_cfg()).unwrap();
        let user = users.get_mut_by_id(1000).unwrap();

        assert_eq!(user.is_passwd_blank(), true);
        assert_eq!(user.is_passwd_unset(), false);
        assert_eq!(user.verify_passwd(""), true);
        assert_eq!(user.verify_passwd("Something"), false);

        user.set_passwd("hi,i_am_passwd").unwrap();

        assert_eq!(user.is_passwd_blank(), false);
        assert_eq!(user.is_passwd_unset(), false);
        assert_eq!(user.verify_passwd(""), false);
        assert_eq!(user.verify_passwd("Something"), false);
        assert_eq!(user.verify_passwd("hi,i_am_passwd"), true);

        user.unset_passwd();

        assert_eq!(user.is_passwd_blank(), false);
        assert_eq!(user.is_passwd_unset(), true);
        assert_eq!(user.verify_passwd(""), false);
        assert_eq!(user.verify_passwd("Something"), false);
        assert_eq!(user.verify_passwd("hi,i_am_passwd"), false);

        user.set_passwd("").unwrap();

        assert_eq!(user.is_passwd_blank(), true);
        assert_eq!(user.is_passwd_unset(), false);
        assert_eq!(user.verify_passwd(""), true);
        assert_eq!(user.verify_passwd("Something"), false);
    }

    // *** struct.AllUsers ***
    #[test]
    fn get_user() {
        let users = AllUsers::new(test_auth_cfg()).unwrap();

        let root = users.get_by_id(0).expect("'root' user missing");
        assert_eq!(root.user, "root".to_string());
        let &(ref hashstring, ref encoded) = root.hash.as_ref().expect("'root' hash is None");
        assert_eq!(hashstring,
            &"$argon2i$m=4096,t=10,p=1$Tnc4UVV0N00$ML9LIOujd3nmAfkAwEcSTMPqakWUF0OUiLWrIy0nGLk".to_string());
        assert_eq!(root.uid, 0);
        assert_eq!(root.gid, 0);
        assert_eq!(root.name, "root".to_string());
        assert_eq!(root.home, "file:/root".to_string());
        assert_eq!(root.shell, "file:/bin/ion".to_string());
        match encoded {
            true => (),
            false => panic!("Expected encoded argon hash!")
        }

        let user = users.get_by_name("user").expect("'user' user missing");
        assert_eq!(user.user, "user".to_string());
        let &(ref hashstring, ref encoded) = user.hash.as_ref().expect("'user' hash is None");
        assert_eq!(hashstring, &"".to_string());
        assert_eq!(user.uid, 1000);
        assert_eq!(user.gid, 1000);
        assert_eq!(user.name, "user".to_string());
        assert_eq!(user.home, "file:/home/user".to_string());
        assert_eq!(user.shell, "file:/bin/ion".to_string());
        match encoded {
            true => panic!("Should not be an argon hash!"),
            false => ()
        }
        println!("{:?}", users);

        let li = users.get_by_name("li").expect("'li' user missing");
        println!("got li");
        assert_eq!(li.user, "li");
        let &(ref hashstring, ref encoded) = li.hash.as_ref().expect("'li' hash is None");
        assert_eq!(hashstring, &"!".to_string());
        assert_eq!(li.uid, 1007);
        assert_eq!(li.gid, 1007);
        assert_eq!(li.name, "Lorem".to_string());
        assert_eq!(li.home, "file:/home/lorem".to_string());
        assert_eq!(li.shell, "file:/bin/ion".to_string());
        match encoded {
            true => panic!("Should not be an argon hash!"),
            false => ()
        }
    }

    #[cfg(feature = "auth")]
    #[test]
    fn manip_user() {
        let mut users = AllUsers::new(test_auth_cfg()).unwrap();
        // NOT testing `get_unique_id`
        let id = 7099;
        users
            .add_user("fb", id, id, "FooBar", "/home/foob", "/bin/zsh")
            .expect("failed to add user 'fb'");
        //                                            weirdo ^^^^^^^^ :P
        users.save().unwrap();
        let p_file_content = read_locked_file(test_prefix(PASSWD_FILE)).unwrap();
        assert_eq!(
            p_file_content,
            concat!(
                "root;0;0;root;file:/root;file:/bin/ion\n",
                "user;1000;1000;user;file:/home/user;file:/bin/ion\n",
                "li;1007;1007;Lorem;file:/home/lorem;file:/bin/ion\n",
                "fb;7099;7099;FooBar;/home/foob;/bin/zsh\n"
            )
        );
        let s_file_content = read_locked_file(test_prefix(SHADOW_FILE)).unwrap();
        assert_eq!(s_file_content, concat!(
            "root;$argon2i$m=4096,t=10,p=1$Tnc4UVV0N00$ML9LIOujd3nmAfkAwEcSTMPqakWUF0OUiLWrIy0nGLk\n",
            "user;\n",
            "li;!\n",
            "fb;!\n"
        ));

        {
            println!("{:?}", users);
            let fb = users.get_mut_by_name("fb").expect("'fb' user missing");
            fb.shell = "/bin/fish".to_string(); // That's better
            fb.set_passwd("").unwrap();
        }
        users.save().unwrap();
        let p_file_content = read_locked_file(test_prefix(PASSWD_FILE)).unwrap();
        assert_eq!(
            p_file_content,
            concat!(
                "root;0;0;root;file:/root;file:/bin/ion\n",
                "user;1000;1000;user;file:/home/user;file:/bin/ion\n",
                "li;1007;1007;Lorem;file:/home/lorem;file:/bin/ion\n",
                "fb;7099;7099;FooBar;/home/foob;/bin/fish\n"
            )
        );
        let s_file_content = read_locked_file(test_prefix(SHADOW_FILE)).unwrap();
        assert_eq!(s_file_content, concat!(
            "root;$argon2i$m=4096,t=10,p=1$Tnc4UVV0N00$ML9LIOujd3nmAfkAwEcSTMPqakWUF0OUiLWrIy0nGLk\n",
            "user;\n",
            "li;!\n",
            "fb;\n"
        ));

        users.remove_by_id(id);
        users.save().unwrap();
        let file_content = read_locked_file(test_prefix(PASSWD_FILE)).unwrap();
        assert_eq!(
            file_content,
            concat!(
                "root;0;0;root;file:/root;file:/bin/ion\n",
                "user;1000;1000;user;file:/home/user;file:/bin/ion\n",
                "li;1007;1007;Lorem;file:/home/lorem;file:/bin/ion\n"
            )
        );
    }

    #[test]
    fn get_group() {
        let groups = AllGroups::new(test_cfg()).unwrap();
        let user = groups.get_by_name("user").unwrap();
        assert_eq!(user.group, "user");
        assert_eq!(user.gid, 1000);
        assert_eq!(user.users, vec!["user"]);

        let wheel = groups.get_by_id(1).unwrap();
        assert_eq!(wheel.group, "wheel");
        assert_eq!(wheel.gid, 1);
        assert_eq!(wheel.users, vec!["user", "root"]);
    }

    #[test]
    fn manip_group() {
        let mut groups = AllGroups::new(test_cfg()).unwrap();
        // NOT testing `get_unique_id`
        let id = 7099;

        groups.add_group("fb", id, &["fb"]).unwrap();
        groups.save().unwrap();
        let file_content = read_locked_file(test_prefix(GROUP_FILE)).unwrap();
        assert_eq!(
            file_content,
            concat!(
                "root;0;root\n",
                "user;1000;user\n",
                "wheel;1;user,root\n",
                "li;1007;li\n",
                "fb;7099;fb\n"
            )
        );

        {
            let fb = groups.get_mut_by_name("fb").unwrap();
            fb.users.push("user".to_string());
        }
        groups.save().unwrap();
        let file_content = read_locked_file(test_prefix(GROUP_FILE)).unwrap();
        assert_eq!(
            file_content,
            concat!(
                "root;0;root\n",
                "user;1000;user\n",
                "wheel;1;user,root\n",
                "li;1007;li\n",
                "fb;7099;fb,user\n"
            )
        );

        groups.remove_by_id(id);
        groups.save().unwrap();
        let file_content = read_locked_file(test_prefix(GROUP_FILE)).unwrap();
        assert_eq!(
            file_content,
            concat!(
                "root;0;root\n",
                "user;1000;user\n",
                "wheel;1;user,root\n",
                "li;1007;li\n"
            )
        );
    }

    // *** Misc ***
    #[test]
    fn users_get_unused_ids() {
        let users = AllUsers::new(test_cfg()).unwrap();
        let id = users.get_unique_id().unwrap();
        if id < users.config.min_id || id > users.config.max_id {
            panic!("User ID is not between allowed margins")
        } else if let Some(_) = users.get_by_id(id) {
            panic!("User ID is used!");
        }
    }

    #[test]
    fn groups_get_unused_ids() {
        let groups = AllGroups::new(test_cfg()).unwrap();
        let id = groups.get_unique_id().unwrap();
        if id < groups.config.min_id || id > groups.config.max_id {
            panic!("Group ID is not between allowed margins")
        } else if let Some(_) = groups.get_by_id(id) {
            panic!("Group ID is used!");
        }
    }
}
