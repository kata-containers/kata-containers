//! Padding and unpadding of messages divided into blocks.
//!
//! This crate provides `Padding` trait which provides padding and unpadding
//! operations. Additionally several common padding schemes are available out
//! of the box.
#![no_std]
#![doc(html_logo_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo_small.png")]

/// Error for indicating failed padding operation
#[derive(Clone, Copy, Debug)]
pub struct PadError;

/// Error for indicating failed unpadding operation
#[derive(Clone, Copy, Debug)]
pub struct UnpadError;

/// Trait for padding messages divided into blocks
pub trait Padding {
    /// Pads `block` filled with data up to `pos`.
    ///
    /// `pos` should be inside of the block and block must not be full, i.e.
    /// `pos < block.len()` must be true. Otherwise method will return
    /// `PadError`. Some potentially irreversible padding schemes can allow
    /// padding of the full block, in this case aforementioned condition is
    /// relaxed to `pos <= block.len()`.
    fn pad_block(block: &mut [u8], pos: usize) -> Result<(), PadError>;

    /// Pads message with length `pos` in the provided buffer.
    ///
    /// `&buf[..pos]` is perceived as the message, the buffer must contain
    /// enough leftover space for padding: `block_size - (pos % block_size)`
    /// extra bytes must be available. Otherwise method will return
    /// `PadError`.
    fn pad(buf: &mut [u8], pos: usize, block_size: usize) -> Result<&mut [u8], PadError> {
        let bs = block_size * (pos / block_size);
        if buf.len() < bs || buf.len() - bs < block_size {
            Err(PadError)?
        }
        Self::pad_block(&mut buf[bs..bs + block_size], pos - bs)?;
        Ok(&mut buf[..bs + block_size])
    }

    /// Unpad given `data` by truncating it according to the used padding.
    /// In case of the malformed padding will return `UnpadError`
    fn unpad(data: &[u8]) -> Result<&[u8], UnpadError>;
}

/// Pad block with zeros.
///
/// ```
/// use block_padding::{ZeroPadding, Padding};
///
/// let msg = b"test";
/// let n = msg.len();
/// let mut buffer = [0xff; 16];
/// buffer[..n].copy_from_slice(msg);
/// let padded_msg = ZeroPadding::pad(&mut buffer, n, 8).unwrap();
/// assert_eq!(padded_msg, b"test\x00\x00\x00\x00");
/// assert_eq!(ZeroPadding::unpad(&padded_msg).unwrap(), msg);
/// ```
/// ```
/// # use block_padding::{ZeroPadding, Padding};
/// # let msg = b"test";
/// # let n = msg.len();
/// # let mut buffer = [0xff; 16];
/// # buffer[..n].copy_from_slice(msg);
/// let padded_msg = ZeroPadding::pad(&mut buffer, n, 2).unwrap();
/// assert_eq!(padded_msg, b"test");
/// assert_eq!(ZeroPadding::unpad(&padded_msg).unwrap(), msg);
/// ```
///
/// Note that zero padding may not be reversible if the original message ends
/// with one or more zero bytes.
#[derive(Clone, Copy, Debug)]
pub enum ZeroPadding {}

impl Padding for ZeroPadding {
    fn pad_block(block: &mut [u8], pos: usize) -> Result<(), PadError> {
        if pos > block.len() {
            Err(PadError)?
        }
        set(&mut block[pos..], 0);
        Ok(())
    }

    fn pad(buf: &mut [u8], pos: usize, block_size: usize) -> Result<&mut [u8], PadError> {
        if pos % block_size == 0 {
            Ok(&mut buf[..pos])
        } else {
            let bs = block_size * (pos / block_size);
            let be = bs + block_size;
            if buf.len() < be {
                Err(PadError)?
            }
            Self::pad_block(&mut buf[bs..be], pos - bs)?;
            Ok(&mut buf[..be])
        }
    }

    fn unpad(data: &[u8]) -> Result<&[u8], UnpadError> {
        let mut n = data.len() - 1;
        while n != 0 {
            if data[n] != 0 {
                break;
            }
            n -= 1;
        }
        Ok(&data[..n + 1])
    }
}

/// Pad block with bytes with value equal to the number of bytes added.
///
/// PKCS#7 described in the [RFC 5652](https://tools.ietf.org/html/rfc5652#section-6.3).
///
/// ```
/// use block_padding::{Pkcs7, Padding};
///
/// let msg = b"test";
/// let n = msg.len();
/// let mut buffer = [0xff; 8];
/// buffer[..n].copy_from_slice(msg);
/// let padded_msg = Pkcs7::pad(&mut buffer, n, 8).unwrap();
/// assert_eq!(padded_msg, b"test\x04\x04\x04\x04");
/// assert_eq!(Pkcs7::unpad(&padded_msg).unwrap(), msg);
/// ```
/// ```
/// # use block_padding::{Pkcs7, Padding};
/// # let msg = b"test";
/// # let n = msg.len();
/// # let mut buffer = [0xff; 8];
/// # buffer[..n].copy_from_slice(msg);
/// let padded_msg = Pkcs7::pad(&mut buffer, n, 2).unwrap();
/// assert_eq!(padded_msg, b"test\x02\x02");
/// assert_eq!(Pkcs7::unpad(&padded_msg).unwrap(), msg);
/// ```
/// ```
/// # use block_padding::{Pkcs7, Padding};
/// let mut buffer = [0xff; 5];
/// assert!(Pkcs7::pad(&mut buffer, 4, 2).is_err());
/// ```
/// ```
/// # use block_padding::{Pkcs7, Padding};
/// # let buffer = [0xff; 16];
/// assert!(Pkcs7::unpad(&buffer).is_err());
/// ```
///
/// In addition to conditions stated in the `Padding` trait documentation,
/// `pad_block` will return `PadError` if `block.len() > 255`, and in case of
/// `pad` if `block_size > 255`.
#[derive(Clone, Copy, Debug)]
pub enum Pkcs7 {}

impl Padding for Pkcs7 {
    fn pad_block(block: &mut [u8], pos: usize) -> Result<(), PadError> {
        if block.len() > 255 {
            Err(PadError)?
        }
        if pos >= block.len() {
            Err(PadError)?
        }
        let n = block.len() - pos;
        set(&mut block[pos..], n as u8);
        Ok(())
    }

    fn unpad(data: &[u8]) -> Result<&[u8], UnpadError> {
        if data.is_empty() {
            Err(UnpadError)?
        }
        let l = data.len();
        let n = data[l - 1];
        if n == 0 || n as usize > l {
            Err(UnpadError)?
        }
        for v in &data[l - n as usize..l - 1] {
            if *v != n {
                Err(UnpadError)?
            }
        }
        Ok(&data[..l - n as usize])
    }
}

/// Pad block with zeros except the last byte which will be set to the number
/// bytes.
///
/// ```
/// use block_padding::{AnsiX923, Padding};
///
/// let msg = b"test";
/// let n = msg.len();
/// let mut buffer = [0xff; 16];
/// buffer[..n].copy_from_slice(msg);
/// let padded_msg = AnsiX923::pad(&mut buffer, n, 8).unwrap();
/// assert_eq!(padded_msg, b"test\x00\x00\x00\x04");
/// assert_eq!(AnsiX923::unpad(&padded_msg).unwrap(), msg);
/// ```
/// ```
/// # use block_padding::{AnsiX923, Padding};
/// # let msg = b"test";
/// # let n = msg.len();
/// # let mut buffer = [0xff; 16];
/// # buffer[..n].copy_from_slice(msg);
/// let padded_msg = AnsiX923::pad(&mut buffer, n, 2).unwrap();
/// assert_eq!(padded_msg, b"test\x00\x02");
/// assert_eq!(AnsiX923::unpad(&padded_msg).unwrap(), msg);
/// ```
/// ```
/// # use block_padding::{AnsiX923, Padding};
/// # let buffer = [0xff; 16];
/// assert!(AnsiX923::unpad(&buffer).is_err());
/// ```
///
/// In addition to conditions stated in the `Padding` trait documentation,
/// `pad_block` will return `PadError` if `block.len() > 255`, and in case of
/// `pad` if `block_size > 255`.
#[derive(Clone, Copy, Debug)]
pub enum AnsiX923 {}

impl Padding for AnsiX923 {
    fn pad_block(block: &mut [u8], pos: usize) -> Result<(), PadError> {
        if block.len() > 255 {
            Err(PadError)?
        }
        if pos >= block.len() {
            Err(PadError)?
        }
        let bs = block.len();
        set(&mut block[pos..bs - 1], 0);
        block[bs - 1] = (bs - pos) as u8;
        Ok(())
    }

    fn unpad(data: &[u8]) -> Result<&[u8], UnpadError> {
        if data.is_empty() {
            Err(UnpadError)?
        }
        let l = data.len();
        let n = data[l - 1] as usize;
        if n == 0 || n > l {
            return Err(UnpadError);
        }
        for v in &data[l - n..l - 1] {
            if *v != 0 {
                Err(UnpadError)?
            }
        }
        Ok(&data[..l - n])
    }
}

/// Pad block with byte sequence `\x80 00...00 00`.
///
/// ```
/// use block_padding::{Iso7816, Padding};
///
/// let msg = b"test";
/// let n = msg.len();
/// let mut buffer = [0xff; 16];
/// buffer[..n].copy_from_slice(msg);
/// let padded_msg = Iso7816::pad(&mut buffer, n, 8).unwrap();
/// assert_eq!(padded_msg, b"test\x80\x00\x00\x00");
/// assert_eq!(Iso7816::unpad(&padded_msg).unwrap(), msg);
/// ```
/// ```
/// # use block_padding::{Iso7816, Padding};
/// # let msg = b"test";
/// # let n = msg.len();
/// # let mut buffer = [0xff; 16];
/// # buffer[..n].copy_from_slice(msg);
/// let padded_msg = Iso7816::pad(&mut buffer, n, 2).unwrap();
/// assert_eq!(padded_msg, b"test\x80\x00");
/// assert_eq!(Iso7816::unpad(&padded_msg).unwrap(), msg);
/// ```
#[derive(Clone, Copy, Debug)]
pub enum Iso7816 {}

impl Padding for Iso7816 {
    fn pad_block(block: &mut [u8], pos: usize) -> Result<(), PadError> {
        if pos >= block.len() {
            Err(PadError)?
        }
        block[pos] = 0x80;
        set(&mut block[pos + 1..], 0);
        Ok(())
    }

    fn unpad(data: &[u8]) -> Result<&[u8], UnpadError> {
        if data.is_empty() {
            Err(UnpadError)?
        }
        let mut n = data.len() - 1;
        while n != 0 {
            if data[n] != 0 {
                break;
            }
            n -= 1;
        }
        if data[n] != 0x80 {
            Err(UnpadError)?
        }
        Ok(&data[..n])
    }
}

/// Don't pad the data. Useful for key wrapping. Padding will fail if the data cannot be
/// fitted into blocks without padding.
///
/// ```
/// use block_padding::{NoPadding, Padding};
///
/// let msg = b"test";
/// let n = msg.len();
/// let mut buffer = [0xff; 16];
/// buffer[..n].copy_from_slice(msg);
/// let padded_msg = NoPadding::pad(&mut buffer, n, 4).unwrap();
/// assert_eq!(padded_msg, b"test");
/// assert_eq!(NoPadding::unpad(&padded_msg).unwrap(), msg);
/// ```
/// ```
/// # use block_padding::{NoPadding, Padding};
/// # let msg = b"test";
/// # let n = msg.len();
/// # let mut buffer = [0xff; 16];
/// # buffer[..n].copy_from_slice(msg);
/// let padded_msg = NoPadding::pad(&mut buffer, n, 2).unwrap();
/// assert_eq!(padded_msg, b"test");
/// assert_eq!(NoPadding::unpad(&padded_msg).unwrap(), msg);
/// ```
#[derive(Clone, Copy, Debug)]
pub enum NoPadding {}

impl Padding for NoPadding {
    fn pad_block(block: &mut [u8], pos: usize) -> Result<(), PadError> {
        if pos % block.len() != 0 {
            Err(PadError)?
        }
        Ok(())
    }

    fn pad(buf: &mut [u8], pos: usize, block_size: usize) -> Result<&mut [u8], PadError> {
        if pos % block_size != 0 {
            Err(PadError)?
        }
        Ok(&mut buf[..pos])
    }

    fn unpad(data: &[u8]) -> Result<&[u8], UnpadError> {
        Ok(data)
    }
}

/// Sets all bytes in `dst` equal to `value`
#[inline(always)]
fn set(dst: &mut [u8], value: u8) {
    // SAFETY: we overwrite valid memory behind `dst`
    // note: loop is not used here because it produces
    // unnecessary branch which tests for zero-length slices
    unsafe {
        core::ptr::write_bytes(dst.as_mut_ptr(), value, dst.len());
    }
}
