#![deny(rust_2018_idioms)]
#![doc(
    html_logo_url = "https://storage.googleapis.com/fdo-gitlab-uploads/project/avatar/3213/zbus-logomark.png"
)]
#![doc = include_str!("../README.md")]

#[cfg(doctest)]
mod doctests {
    doc_comment::doctest!("../README.md");
}

mod bus_name;
pub use bus_name::*;

mod unique_name;
pub use unique_name::*;

mod well_known_name;
pub use well_known_name::*;

mod interface_name;
pub use interface_name::*;

mod member_name;
pub use member_name::*;

mod error;
pub use error::*;

mod error_name;
pub use error_name::*;
