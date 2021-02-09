[![Travis Build Status](https://travis-ci.org/havarnov/multimap.svg?branch=master)](https://travis-ci.org/havarnov/multimap)
[![crates.io](http://meritbadge.herokuapp.com/multimap)](https://crates.io/crates/multimap)

# Multimap implementation for Rust

This is a multimap implementation for Rust. Implemented as a thin wrapper around
std::collections::HashMap.

[Documentation](http://havarnov.github.io/multimap)

## Example

````rust
extern crate multimap;

use multimap::MultiMap;

fn main () {
    let mut map = MultiMap::new();

    map.insert("key1", 42);
    map.insert("key1", 1337);
    map.insert("key2", 2332);

    assert_eq!(map["key1"], 42);
    assert_eq!(map.get("key1"), Some(&42));
    assert_eq!(map.get_vec("key1"), Some(&vec![42, 1337]));
}
````

## License

Licensed under either of
 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
