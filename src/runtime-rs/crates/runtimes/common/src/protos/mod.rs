pub(crate) mod cri {
    include!(concat!(env!("OUT_DIR"), "/cri/mod.rs"));
}

pub use cri::*;
