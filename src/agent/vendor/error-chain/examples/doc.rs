#![deny(missing_docs)]

//! This module is used to check that all generated items are documented.

#[macro_use]
extern crate error_chain;

/// Inner module.
pub mod inner {
    error_chain! {}
}

error_chain! {
    links {
        Inner(inner::Error, inner::ErrorKind) #[doc = "Doc"];
    }
    foreign_links {
        Io(::std::io::Error) #[doc = "Io"];
    }
    errors {
        /// Doc
        Test2 {

        }
    }
}

fn main() {}
