use crate::imp;
use imp::fs::Dev;

/// `makedev(maj, min)`
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man3/makedev.3.html
#[inline]
pub fn makedev(maj: u32, min: u32) -> Dev {
    imp::fs::makedev(maj, min)
}

/// `minor(dev)`
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man3/minor.3.html
#[inline]
pub fn minor(dev: Dev) -> u32 {
    imp::fs::minor(dev)
}

/// `major(dev)`
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man3/major.3.html
#[inline]
pub fn major(dev: Dev) -> u32 {
    imp::fs::major(dev)
}
