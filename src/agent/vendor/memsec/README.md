# memsec
[![travis-ci](https://travis-ci.org/quininer/memsec.svg?branch=master)](https://travis-ci.org/quininer/memsec)
[![appveyor](https://ci.appveyor.com/api/projects/status/1w0qtl0grjfu0uac?svg=true)](https://ci.appveyor.com/project/quininer/memsec)
[![crates](https://img.shields.io/crates/v/memsec.svg)](https://crates.io/crates/memsec)
[![license](https://img.shields.io/github/license/quininer/memsec.svg)](https://github.com/quininer/memsec/blob/master/LICENSE)
[![docs.rs](https://docs.rs/memsec/badge.svg)](https://docs.rs/memsec/)

Rust implementation `libsodium/utils`.

* [x] `memeq`/`memcmp`
* [x] `memset`/`memzero`
* [x] `mlock`/`munlock`
* [x] `alloc`/`free`/`mprotect`

ref
---

* [Securing memory allocations](https://download.libsodium.org/doc/helpers/memory_management.html)
* [rlibc](https://github.com/alexcrichton/rlibc)
* [aligned\_alloc.rs](https://github.com/jonas-schievink/aligned_alloc.rs)
* [cst\_time\_memcmp](https://github.com/chmike/cst_time_memcmp)
