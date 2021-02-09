#![recursion_limit = "1000"]
#![cfg_attr(rustc_nightly, feature(test))]

#![doc(html_root_url = "https://docs.rs/procinfo/0.4.2")]

#![allow(dead_code)] // TODO: remove

#[macro_use]
extern crate nom;

extern crate byteorder;
extern crate libc;

#[macro_use]
mod parsers;

mod loadavg;
pub mod pid;
pub mod sys;
pub mod net;

pub use loadavg::{LoadAvg, loadavg};
