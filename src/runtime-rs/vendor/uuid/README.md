uuid
====

[![Build Status](https://travis-ci.org/rust-lang-nursery/uuid.svg?branch=master)](https://travis-ci.org/rust-lang-nursery/uuid)

A Rust library to generate and parse UUIDs.

Provides support for Universally Unique Identifiers (UUIDs). A UUID is a unique
128-bit number, stored as 16 octets. UUIDs are used to assign unique identifiers
to entities without requiring a central allocating authority.

They are particularly useful in distributed systems, though can be used in
disparate areas, such as databases and network protocols. Typically a UUID is
displayed in a readable string form as a sequence of hexadecimal digits,
separated into groups by hyphens.

The uniqueness property is not strictly guaranteed, however for all practical
purposes, it can be assumed that an unintentional collision would be extremely
unlikely.

[Documentation](https://doc.rust-lang.org/uuid)

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
uuid = "0.3"
```

and this to your crate root:

```rust
extern crate uuid;
```

## Examples

To parse a simple UUID, then print the version and urn string format:

```rust
extern crate uuid;
use uuid::Uuid;

fn main() {
    let my_uuid = Uuid::parse_str("936DA01F9ABD4d9d80C702AF85C822A8").unwrap();
    println!("Parsed a version {} UUID.", my_uuid.get_version_num());
    println!("{}", my_uuid);
}
```

The library supports 5 versions of UUID:

Name     | Version
---------|----------
Mac      | Version 1: MAC address
Dce      | Version 2: DCE Security
Md5      | Version 3: MD5 hash
Random   | Version 4: Random
Sha1     | Version 5: SHA-1 hash

To create a new random (V4) UUID and print it out in hexadecimal form, first
you'll need to change how you depend on `uuid`:

```toml
[dependencies]
uuid = { version = "0.2", features = ["v4"] }
```

Next, you'll write:

```rust
extern crate uuid;
use uuid::Uuid;

fn main() {
    let my_uuid = Uuid::new_v4();
    println!("{}", my_uuid);
}
```

To create a new sha1-hash based (V5) UUID and print it out in hexadecimal form,
you'll also need to change how you depend on `uuid`:

```toml
[dependencies]
uuid = { version = "0.2", features = ["v5"] }
```

Next, you'll write:

```rust
extern crate uuid;
use uuid::Uuid;

fn main() {
    let my_uuid = Uuid::new_v5(&uuid::NAMESPACE_DNS, "foo");
    println!("{}", my_uuid);
}
```

## References

[Wikipedia: Universally Unique Identifier](https://en.wikipedia.org/wiki/Universally_unique_identifier)
