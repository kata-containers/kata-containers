# rust-users [![users on crates.io][crates-badge]][crates-url] [![Minimum Rust Version 1.31.0][rustc-badge]][rustc-url] [![Build status][travis-badge]][travis-url]

[crates-badge]: https://meritbadge.herokuapp.com/users
[crates-url]: https://crates.io/crates/users
[travis-badge]: https://travis-ci.org/ogham/rust-users.svg?branch=master
[travis-url]: https://travis-ci.org/github/ogham/rust-users
[rustc-badge]: https://img.shields.io/badge/rustc-1.31+-lightgray.svg
[rustc-url]: https://blog.rust-lang.org/2018/12/06/Rust-1.31-and-rust-2018.html

This is a library for accessing Unix users and groups.
It supports getting the system users and groups, storing them in a cache, and creating your own mock tables.

### [View the Rustdoc](https://docs.rs/users)


# Installation

This crate works with [Cargo](https://crates.io). Add the following to your `Cargo.toml` dependencies section:

```toml
[dependencies]
users = "0.11"
```

The earliest version of Rust that this crate is tested against is [Rust v1.31.0][rustc-url].


# Usage

In Unix, each user has an individual *user ID*, and each process has an *effective user ID* that says which user’s permissions it is using.
Furthermore, users can be the members of *groups*, which also have names and IDs.
This functionality is exposed in libc, the C standard library, but as an unsafe Rust interface.
This wrapper library provides a safe interface, using `User` and `Group` types and functions such as `get_user_by_id` instead of low-level pointers and strings.
It also offers basic caching functionality.

It does not (yet) offer *editing* functionality; the values returned are read-only.


## Users

The function `get_current_uid` returns a `uid_t` value representing the user currently running the program, and the `get_user_by_uid` function scans the users database and returns a `User` with the user’s information.
This function returns `None` when there is no user for that ID.

A `User` has the following accessors:

- **uid:** The user’s ID
- **name:** The user’s name
- **primary_group:** The ID of this user’s primary group

Here is a complete example that prints out the current user’s name:

```rust
use users::{get_user_by_uid, get_current_uid};

let user = get_user_by_uid(get_current_uid()).unwrap();
println!("Hello, {}!", user.name());
```

This code assumes (with `unwrap()`) that the user hasn’t been deleted after the program has started running.
For arbitrary user IDs, this is **not** a safe assumption: it’s possible to delete a user while it’s running a program, or is the owner of files, or for that user to have never existed.
So always check the return values!

There is also a `get_current_username` function, as it’s such a common operation that it deserves special treatment.


## Caching

Despite the above warning, the users and groups database rarely changes.
While a short program may only need to get user information once, a long-running one may need to re-query the database many times, and a medium-length one may get away with caching the values to save on redundant system calls.

For this reason, this crate offers a caching interface to the database, which offers the same functionality while holding on to every result, caching the information so it can be re-used.

To introduce a cache, create a new `UsersCache` and call the same methods on it.
For example:

```rust
use users::{Users, Groups, UsersCache};

let mut cache = UsersCache::new();
let uid = cache.get_current_uid();
let user = cache.get_user_by_uid(uid).unwrap();
println!("Hello again, {}!", user.name());
```

This cache is **only additive**: it’s not possible to drop it, or erase selected entries, as when the database may have been modified, it’s best to start entirely afresh.
So to accomplish this, just start using a new `UsersCache`.


## Groups

Finally, it’s possible to get groups in a similar manner.
A `Group` has the following accessors:

- **gid:** The group’s ID
- **name:** The group’s name

And again, a complete example:

```rust
use users::{Users, Groups, UsersCache};

let mut cache = UsersCache::new();
let group = cache.get_group_by_name("admin").expect("No such group 'admin'!");
println!("The '{}' group has the ID {}", group.name(), group.gid());
```


## Logging

The `logging` feature, which is on by default, uses the `log` crate to record all interactions with the operating system at Trace log level.


## Caveats

You should be prepared for the users and groups tables to be completely broken: IDs shouldn’t be assumed to map to actual users and groups, and usernames and group names aren’t guaranteed to map either!


# Mockable users and groups

When you’re testing your code, you don’t want to actually rely on the system actually having various users and groups present - it’s much better to have a custom set of users that are *guaranteed* to be there, so you can test against them.

The `mock` module allows you to create these custom users and groups definitions, then access them using the same `Users` trait as in the main library, with few changes to your code.


## Creating mock users

The only thing a mock users table needs to know in advance is the UID of the current user.
Aside from that, you can add users and groups with `add_user` and `add_group` to the table:

```rust
use std::sync::Arc;
use users::mock::{MockUsers, User, Group};
use users::os::unix::{UserExt, GroupExt};

let mut users = MockUsers::with_current_uid(1000);
let bobbins = User::new(1000, "Bobbins", 1000).with_home_dir("/home/bobbins");
users.add_user(bobbins);
users.add_group(Group::new(100, "funkyppl"));
```

The exports get re-exported into the mock module, for simpler `use` lines.


## Using mock users

To set your program up to use either type of `Users` table, make your functions and structs accept a generic parameter that implements the `Users` trait.
Then, you can pass in a value of either OS or Mock type.

Here’s a complete example:

```rust
use std::sync::Arc;
use users::{Users, UsersCache, User};
use users::os::unix::UserExt;
use users::mock::MockUsers;

fn print_current_username<U: Users>(users: &mut U) {
    println!("Current user: {:?}", users.get_current_username());
}

let mut users = MockUsers::with_current_uid(1001);
users.add_user(User::new(1001, "fred", 101));
print_current_username(&mut users);

let mut actual_users = UsersCache::new();
print_current_username(&mut actual_users);
```
