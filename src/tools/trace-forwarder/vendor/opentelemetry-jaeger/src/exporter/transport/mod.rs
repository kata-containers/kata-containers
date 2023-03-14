//! Additional Thrift transport implementations
mod buffer;
mod noop;

pub(crate) use buffer::TBufferChannel;
pub(crate) use noop::TNoopChannel;
