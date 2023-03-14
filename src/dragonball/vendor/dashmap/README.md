# DashMap

Blazingly fast concurrent map in Rust.

DashMap is an implementation of a concurrent associative array/hashmap in Rust.

DashMap tries to implement an easy to use API similar to `std::collections::HashMap`
with some slight changes to handle concurrency.

DashMap tries to be very simple to use and to be a direct replacement for `RwLock<HashMap<K, V>>`.
To accomplish these all methods take `&self` instead modifying methods taking `&mut self`.
This allows you to put a DashMap in an `Arc<T>` and share it between threads while being able to modify it.

DashMap puts great effort into performance and aims to be as fast as possible.
If you have any suggestions or tips do not hesitate to open an issue or a PR.

[![version](https://img.shields.io/crates/v/dashmap)](https://crates.io/crates/dashmap)

[![documentation](https://docs.rs/dashmap/badge.svg)](https://docs.rs/dashmap)

[![downloads](https://img.shields.io/crates/d/dashmap)](https://crates.io/crates/dashmap)

[![minimum rustc version](https://img.shields.io/badge/rustc-1.44.1-orange.svg)](https://crates.io/crates/dashmap)

## Cargo features

- `serde` - Enables serde support.

- `raw-api` - Enables the unstable raw-shard api.

- `rayon` - Enables rayon support.

## Support me

[![Foo](https://c5.patreon.com/external/logo/become_a_patron_button@2x.png)](https://patreon.com/acrimon)

Creating and testing open-source software like DashMap takes up a large portion of my time
and comes with costs such as test hardware. Please consider supporting me and everything I make for the public
to enable me to continue doing this.

If you want to support me please head over and take a look at my [patreon](https://www.patreon.com/acrimon).

## Contributing

DashMap is gladly accepts contributions!
Do not hesitate to open issues or PR's.

I will take a look as soon as I have time for it.

That said I do not get paid (yet) to work on open-source. This means
that my time is limited and my work here comes after my personal life.

## Performance

A comprehensive benchmark suite including DashMap can be found [here](https://github.com/xacrimon/conc-map-bench).

## Special thanks

- [Jon Gjengset](https://github.com/jonhoo)

- [Yato](https://github.com/RustyYato) 

- [Karl Bergström](https://github.com/kabergstrom)

- [Dylan DPC](https://github.com/Dylan-DPC)

- [Lokathor](https://github.com/Lokathor)

- [namibj](https://github.com/namibj)

## License

This project is licensed under MIT.
