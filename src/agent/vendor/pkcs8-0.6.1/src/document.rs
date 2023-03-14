//! Serialized DER-encoded documents stored in heap-backed buffers.
// TODO(tarcieri): heapless support?

#[cfg(feature = "pkcs5")]
pub(crate) mod encrypted_private_key;
pub(crate) mod private_key;
pub(crate) mod public_key;
