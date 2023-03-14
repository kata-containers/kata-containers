// -*- mode: rust; -*-
//
// This file is part of x25519-dalek.
// Copyright (c) 2017-2021 isis lovecruft
// Copyright (c) 2019-2021 DebugSteven
// See LICENSE for licensing information.
//
// Authors:
// - isis agora lovecruft <isis@patternsinthevoid.net>
// - DebugSteven <debugsteven@gmail.com>

// Refuse to compile if documentation is missing, but only on nightly.
//
// This means that missing docs will still fail CI, but means we can use
// README.md as the crate documentation.

#![no_std]
#![cfg_attr(feature = "bench", feature(test))]
#![cfg_attr(feature = "nightly", feature(external_doc))]
#![cfg_attr(feature = "nightly", deny(missing_docs))]
#![cfg_attr(feature = "nightly", doc(include = "../README.md"))]
#![doc(html_logo_url = "https://doc.dalek.rs/assets/dalek-logo-clear.png")]
#![doc(html_root_url = "https://docs.rs/x25519-dalek/1.1.1")]

//! Note that docs will only build on nightly Rust until
//! `feature(external_doc)` is stabilized.

extern crate curve25519_dalek;

extern crate rand_core;

extern crate zeroize;

mod x25519;

pub use crate::x25519::*;
