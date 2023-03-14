// SPDX-License-Identifier: MIT

use std::{fmt, io, mem::size_of};

use byteorder::{ByteOrder, NativeEndian};

use crate::{DecodeError, Emitable, Field, Parseable, Rest};

const CODE: Field = 0..4;
const PAYLOAD: Rest = 4..;
const ERROR_HEADER_LEN: usize = PAYLOAD.start;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ErrorBuffer<T> {
    buffer: T,
}

impl<T: AsRef<[u8]>> ErrorBuffer<T> {
    pub fn new(buffer: T) -> ErrorBuffer<T> {
        ErrorBuffer { buffer }
    }

    /// Consume the packet, returning the underlying buffer.
    pub fn into_inner(self) -> T {
        self.buffer
    }

    pub fn new_checked(buffer: T) -> Result<Self, DecodeError> {
        let packet = Self::new(buffer);
        packet.check_buffer_length()?;
        Ok(packet)
    }

    fn check_buffer_length(&self) -> Result<(), DecodeError> {
        let len = self.buffer.as_ref().len();
        if len < ERROR_HEADER_LEN {
            Err(format!(
                "invalid ErrorBuffer: length is {} but ErrorBuffer are at least {} bytes",
                len, ERROR_HEADER_LEN
            )
            .into())
        } else {
            Ok(())
        }
    }

    /// Return the error code
    pub fn code(&self) -> i32 {
        let data = self.buffer.as_ref();
        NativeEndian::read_i32(&data[CODE])
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> ErrorBuffer<&'a T> {
    /// Return a pointer to the payload.
    pub fn payload(&self) -> &'a [u8] {
        let data = self.buffer.as_ref();
        &data[PAYLOAD]
    }
}

impl<'a, T: AsRef<[u8]> + AsMut<[u8]> + ?Sized> ErrorBuffer<&'a mut T> {
    /// Return a mutable pointer to the payload.
    pub fn payload_mut(&mut self) -> &mut [u8] {
        let data = self.buffer.as_mut();
        &mut data[PAYLOAD]
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> ErrorBuffer<T> {
    /// set the error code field
    pub fn set_code(&mut self, value: i32) {
        let data = self.buffer.as_mut();
        NativeEndian::write_i32(&mut data[CODE], value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorMessage {
    pub code: i32,
    pub header: Vec<u8>,
}

pub type AckMessage = ErrorMessage;

impl Emitable for ErrorMessage {
    fn buffer_len(&self) -> usize {
        size_of::<i32>() + self.header.len()
    }
    fn emit(&self, buffer: &mut [u8]) {
        let mut buffer = ErrorBuffer::new(buffer);
        buffer.set_code(self.code);
        buffer.payload_mut().copy_from_slice(&self.header)
    }
}

impl<'buffer, T: AsRef<[u8]> + 'buffer> Parseable<ErrorBuffer<&'buffer T>> for ErrorMessage {
    fn parse(buf: &ErrorBuffer<&'buffer T>) -> Result<ErrorMessage, DecodeError> {
        // FIXME: The payload of an error is basically a truncated packet, which requires custom
        // logic to parse correctly. For now we just return it as a Vec<u8>
        // let header: NetlinkHeader = {
        //     NetlinkBuffer::new_checked(self.payload())
        //         .context("failed to parse netlink header")?
        //         .parse()
        //         .context("failed to parse nelink header")?
        // };
        Ok(ErrorMessage {
            code: buf.code(),
            header: buf.payload().to_vec(),
        })
    }
}

impl ErrorMessage {
    /// According to [`netlink(7)`](https://linux.die.net/man/7/netlink)
    /// the `NLMSG_ERROR` return Negative errno or 0 for acknowledgements.
    ///
    /// convert into [`std::io::Error`](https://doc.rust-lang.org/std/io/struct.Error.html)
    /// using the absolute value from errno code
    pub fn to_io(&self) -> io::Error {
        io::Error::from_raw_os_error(self.code.abs())
    }
}

impl fmt::Display for ErrorMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.to_io(), f)
    }
}

impl From<ErrorMessage> for io::Error {
    fn from(e: ErrorMessage) -> io::Error {
        e.to_io()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn into_io_error() {
        let io_err = io::Error::from_raw_os_error(95);
        let err_msg = ErrorMessage {
            code: -95,
            header: vec![],
        };

        let to_io: io::Error = err_msg.to_io();

        assert_eq!(err_msg.to_string(), io_err.to_string());
        assert_eq!(to_io.raw_os_error(), io_err.raw_os_error());
    }
}
