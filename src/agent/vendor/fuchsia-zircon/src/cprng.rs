use {Status, ok, sys};

/// Draw random bytes from the kernel's CPRNG to fill the given buffer. Returns the actual number of
/// bytes drawn, which may sometimes be less than the size of the buffer provided.
///
/// The buffer must have length less than `ZX_CPRNG_DRAW_MAX_LEN`.
///
/// Wraps the
/// [zx_cprng_draw](https://fuchsia.googlesource.com/zircon/+/HEAD/docs/syscalls/cprng_draw.md)
/// syscall.
pub fn cprng_draw(buffer: &mut [u8]) -> Result<usize, Status> {
    let mut actual = 0;
    let status = unsafe { sys::zx_cprng_draw(buffer.as_mut_ptr(), buffer.len(), &mut actual) };
    ok(status).map(|()| actual)
}

/// Mix the given entropy into the kernel CPRNG.
///
/// The buffer must have length less than `ZX_CPRNG_ADD_ENTROPY_MAX_LEN`.
///
/// Wraps the
/// [zx_cprng_add_entropy](https://fuchsia.googlesource.com/zircon/+/HEAD/docs/syscalls/cprng_add_entropy.md)
/// syscall.
pub fn cprng_add_entropy(buffer: &[u8]) -> Result<(), Status> {
    let status = unsafe { sys::zx_cprng_add_entropy(buffer.as_ptr(), buffer.len()) };
    ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cprng() {
        let mut buffer = [0; 20];
        assert_eq!(cprng_draw(&mut buffer), Ok(20));
        let mut first_zero = 0;
        let mut last_zero = 0;
        for _ in 0..30 {
            let mut buffer = [0; 20];
            assert_eq!(cprng_draw(&mut buffer), Ok(20));
            if buffer[0] == 0 {
                first_zero += 1;
            }
            if buffer[19] == 0 {
                last_zero += 1;
            }
        }
        assert_ne!(first_zero, 30);
        assert_ne!(last_zero, 30);
    }

    #[test]
    fn cprng_too_large() {
        let mut buffer = [0; sys::ZX_CPRNG_DRAW_MAX_LEN + 1];
        assert_eq!(cprng_draw(&mut buffer), Err(Status::INVALID_ARGS));

        for mut s in buffer.chunks_mut(sys::ZX_CPRNG_DRAW_MAX_LEN) {
            assert_eq!(cprng_draw(&mut s), Ok(s.len()));
        }
    }

    #[test]
    fn cprng_add() {
        let buffer = [0, 1, 2];
        assert_eq!(cprng_add_entropy(&buffer), Ok(()));
    }
}