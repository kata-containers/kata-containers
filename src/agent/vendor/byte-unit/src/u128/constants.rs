/// KB
pub const KILOBYTE: u128 = 1000;
/// KiB
pub const KIBIBYTE: u128 = 1024;
/// MB
pub const MEGABYTE: u128 = 1000000;
/// MiB
pub const MEBIBYTE: u128 = 1048576;
/// GB
pub const GIGABYTE: u128 = 1000000000;
/// GiB
pub const GIBIBYTE: u128 = 1073741824;
/// TB
pub const TERABYTE: u128 = 1000000000000;
/// TiB
pub const TEBIBYTE: u128 = 1099511627776;
/// PB
pub const PETABYTE: u128 = 1000000000000000;
/// PiB
pub const PEBIBYTE: u128 = 1125899906842624;
/// EB
pub const EXABYTE: u128 = 1000000000000000000;
/// EiB
pub const EXBIBYTE: u128 = 1152921504606846976;
/// ZB
pub const ZETTABYTE: u128 = 1000000000000000000000;
/// ZiB
pub const ZEBIBYTE: u128 = 1180591620717411303424;

/// Convert n KB to bytes.
#[inline]
pub const fn n_kb_bytes(bytes: u128) -> u128 {
    bytes * KILOBYTE
}

/// Convert n KiB to bytes.
#[inline]
pub const fn n_kib_bytes(bytes: u128) -> u128 {
    bytes * KIBIBYTE
}

/// Convert n MB to bytes.
#[inline]
pub const fn n_mb_bytes(bytes: u128) -> u128 {
    bytes * MEGABYTE
}

/// Convert n MiB to bytes.
#[inline]
pub const fn n_mib_bytes(bytes: u128) -> u128 {
    bytes * MEBIBYTE
}

/// Convert n GB to bytes.
#[inline]
pub const fn n_gb_bytes(bytes: u128) -> u128 {
    bytes * GIGABYTE
}

/// Convert n GiB to bytes.
#[inline]
pub const fn n_gib_bytes(bytes: u128) -> u128 {
    bytes * GIBIBYTE
}

/// Convert n TB to bytes.
#[inline]
pub const fn n_tb_bytes(bytes: u128) -> u128 {
    bytes * TERABYTE
}

/// Convert n TiB to bytes.
#[inline]
pub const fn n_tib_bytes(bytes: u128) -> u128 {
    bytes * TEBIBYTE
}

/// Convert n PB to bytes.
#[inline]
pub const fn n_pb_bytes(bytes: u128) -> u128 {
    bytes * PETABYTE
}

/// Convert n PiB to bytes.
#[inline]
pub const fn n_pib_bytes(bytes: u128) -> u128 {
    bytes * PEBIBYTE
}

/// Convert n EB to bytes.
#[inline]
pub const fn n_eb_bytes(bytes: u128) -> u128 {
    bytes * EXABYTE
}

/// Convert n EiB to bytes.
#[inline]
pub const fn n_eib_bytes(bytes: u128) -> u128 {
    bytes * EXBIBYTE
}

/// Convert n ZB to bytes.
#[inline]
pub const fn n_zb_bytes(bytes: u128) -> u128 {
    bytes * ZETTABYTE
}

/// Convert n ZiB to bytes.
#[inline]
pub const fn n_zib_bytes(bytes: u128) -> u128 {
    bytes * ZEBIBYTE
}
