/// KB
pub const KILOBYTE: u64 = 1_000;
/// KiB
pub const KIBIBYTE: u64 = 1 << 10;
/// MB
pub const MEGABYTE: u64 = 1_000_000;
/// MiB
pub const MEBIBYTE: u64 = 1 << 20;
/// GB
pub const GIGABYTE: u64 = 1_000_000_000;
/// GiB
pub const GIBIBYTE: u64 = 1 << 30;
/// TB
pub const TERABYTE: u64 = 1_000_000_000_000;
/// TiB
pub const TEBIBYTE: u64 = 1 << 40;
/// PB
pub const PETABYTE: u64 = 1_000_000_000_000_000;
/// PiB
pub const PEBIBYTE: u64 = 1 << 50;

/// Convert n KB to bytes.
#[inline]
pub const fn n_kb_bytes(bytes: u64) -> u64 {
    bytes * KILOBYTE
}

/// Convert n KiB to bytes.
#[inline]
pub const fn n_kib_bytes(bytes: u64) -> u64 {
    bytes * KIBIBYTE
}

/// Convert n MB to bytes.
#[inline]
pub const fn n_mb_bytes(bytes: u64) -> u64 {
    bytes * MEGABYTE
}

/// Convert n MiB to bytes.
#[inline]
pub const fn n_mib_bytes(bytes: u64) -> u64 {
    bytes * MEBIBYTE
}

/// Convert n GB to bytes.
#[inline]
pub const fn n_gb_bytes(bytes: u64) -> u64 {
    bytes * GIGABYTE
}

/// Convert n GiB to bytes.
#[inline]
pub const fn n_gib_bytes(bytes: u64) -> u64 {
    bytes * GIBIBYTE
}

/// Convert n TB to bytes.
#[inline]
pub const fn n_tb_bytes(bytes: u64) -> u64 {
    bytes * TERABYTE
}

/// Convert n TiB to bytes.
#[inline]
pub const fn n_tib_bytes(bytes: u64) -> u64 {
    bytes * TEBIBYTE
}

/// Convert n PB to bytes.
#[inline]
pub const fn n_pb_bytes(bytes: u64) -> u64 {
    bytes * PETABYTE
}

/// Convert n PiB to bytes.
#[inline]
pub const fn n_pib_bytes(bytes: u64) -> u64 {
    bytes * PEBIBYTE
}
