/// Convert n KB to bytes.
///
/// ## Examples
///
/// ```
/// extern crate byte_unit;
///
/// let result = byte_unit::n_kb_bytes!(4);
///
/// assert_eq!(4000, result);
/// ```
///
/// ```
/// extern crate byte_unit;
///
/// let result = byte_unit::n_kb_bytes!(2.5, f64);
///
/// assert_eq!(2500, result);
/// ```
#[macro_export]
macro_rules! n_kb_bytes {
    () => {
        $crate::KILOBYTE
    };
    ($x:expr) => {
        $crate::n_kb_bytes($x)
    };
    ($x:expr, $t:ty) => {
        $crate::_bytes_as!($x * ($crate::MEGABYTE as $t)) / $crate::KILOBYTE
    };
}

/// Convert n KiB to bytes.
///
/// ## Examples
///
/// ```
/// extern crate byte_unit;
///
/// let result = byte_unit::n_kib_bytes!(4);
///
/// assert_eq!(4096, result);
/// ```
///
/// ```
/// extern crate byte_unit;
///
/// let result = byte_unit::n_kib_bytes!(2.5, f64);
///
/// assert_eq!(2560, result);
/// ```
#[macro_export]
macro_rules! n_kib_bytes {
    () => {
        $crate::KIBIBYTE
    };
    ($x:expr) => {
        $crate::n_kib_bytes($x)
    };
    ($x:expr, $t:ty) => {
        $crate::_bytes_as!($x * ($crate::MEBIBYTE as $t)) / $crate::KIBIBYTE
    };
}

/// Convert n MB to bytes.
///
/// ## Examples
///
/// ```
/// extern crate byte_unit;
///
/// let result = byte_unit::n_mb_bytes!(4);
///
/// assert_eq!(4000000, result);
/// ```
///
/// ```
/// extern crate byte_unit;
///
/// let result = byte_unit::n_mb_bytes!(2.5, f64);
///
/// assert_eq!(2500000, result);
/// ```
#[macro_export]
macro_rules! n_mb_bytes {
    () => {
        $crate::MEGABYTE
    };
    ($x:expr) => {
        $crate::n_mb_bytes($x)
    };
    ($x:expr, $t:ty) => {
        $crate::_bytes_as!($x * ($crate::MEGABYTE as $t))
    };
}

/// Convert n MiB to bytes.
///
/// ## Examples
///
/// ```
/// extern crate byte_unit;
///
/// let result = byte_unit::n_mib_bytes!(4);
///
/// assert_eq!(4194304, result);
/// ```
///
/// ```
/// extern crate byte_unit;
///
/// let result = byte_unit::n_mib_bytes!(2.5, f64);
///
/// assert_eq!(2621440, result);
/// ```
#[macro_export]
macro_rules! n_mib_bytes {
    () => {
        $crate::MEBIBYTE
    };
    ($x:expr) => {
        $crate::n_mib_bytes($x)
    };
    ($x:expr, $t:ty) => {
        $crate::_bytes_as!($x * ($crate::MEBIBYTE as $t))
    };
}

/// Convert n GB to bytes.
///
/// ## Examples
///
/// ```
/// extern crate byte_unit;
///
/// let result = byte_unit::n_gb_bytes!(4);
///
/// assert_eq!(4000000000, result);
/// ```
///
/// ```
/// extern crate byte_unit;
///
/// let result = byte_unit::n_gb_bytes!(2.5, f64);
///
/// assert_eq!(2500000000, result);
/// ```
#[macro_export]
macro_rules! n_gb_bytes {
    () => {
        $crate::GIGABYTE
    };
    ($x:expr) => {
        $crate::n_gb_bytes($x)
    };
    ($x:expr, $t:ty) => {
        $crate::n_kb_bytes($crate::_bytes_as!($x * ($crate::MEGABYTE as $t)))
    };
}

/// Convert n GiB to bytes.
///
/// ## Examples
///
/// ```
/// extern crate byte_unit;
///
/// let result = byte_unit::n_gib_bytes!(4);
///
/// assert_eq!(4294967296, result);
/// ```
///
/// ```
/// extern crate byte_unit;
///
/// let result = byte_unit::n_gib_bytes!(2.5, f64);
///
/// assert_eq!(2684354560, result);
/// ```
#[macro_export]
macro_rules! n_gib_bytes {
    () => {
        $crate::GIBIBYTE
    };
    ($x:expr) => {
        $crate::n_gib_bytes($x)
    };
    ($x:expr, $t:ty) => {
        $crate::n_kib_bytes($crate::_bytes_as!($x * ($crate::MEBIBYTE as $t)))
    };
}

/// Convert n TB to bytes.
///
/// ## Examples
///
/// ```
/// extern crate byte_unit;
///
/// let result = byte_unit::n_tb_bytes!(4);
///
/// assert_eq!(4000000000000, result);
/// ```
///
/// ```
/// extern crate byte_unit;
///
/// let result = byte_unit::n_tb_bytes!(2.5, f64);
///
/// assert_eq!(2500000000000, result);
/// ```
#[macro_export]
macro_rules! n_tb_bytes {
    () => {
        $crate::TERABYTE
    };
    ($x:expr) => {
        $crate::n_tb_bytes($x)
    };
    ($x:expr, $t:ty) => {
        $crate::n_mb_bytes($crate::_bytes_as!($x * ($crate::MEGABYTE as $t)))
    };
}

/// Convert n TiB to bytes.
///
/// ## Examples
///
/// ```
/// extern crate byte_unit;
///
/// let result = byte_unit::n_tib_bytes!(4);
///
/// assert_eq!(4398046511104, result);
/// ```
///
/// ```
/// extern crate byte_unit;
///
/// let result = byte_unit::n_tib_bytes!(2.5, f64);
///
/// assert_eq!(2748779069440, result);
/// ```
#[macro_export]
macro_rules! n_tib_bytes {
    () => {
        $crate::TEBIBYTE
    };
    ($x:expr) => {
        $crate::n_tib_bytes($x)
    };
    ($x:expr, $t:ty) => {
        $crate::n_mib_bytes($crate::_bytes_as!($x * ($crate::MEBIBYTE as $t)))
    };
}

/// Convert n PB to bytes.
///
/// ## Examples
///
/// ```
/// extern crate byte_unit;
///
/// let result = byte_unit::n_pb_bytes!(4);
///
/// assert_eq!(4000000000000000, result);
/// ```
///
/// ```
/// extern crate byte_unit;
///
/// let result = byte_unit::n_pb_bytes!(2.5, f64);
///
/// assert_eq!(2500000000000000, result);
/// ```
#[macro_export]
macro_rules! n_pb_bytes {
    () => {
        $crate::PETABYTE
    };
    ($x:expr) => {
        $crate::n_pb_bytes($x)
    };
    ($x:expr, $t:ty) => {
        $crate::n_gb_bytes($crate::_bytes_as!($x * ($crate::MEGABYTE as $t)))
    };
}

/// Convert n PiB to bytes.
///
/// ## Examples
///
/// ```
/// extern crate byte_unit;
///
/// let result = byte_unit::n_pib_bytes!(4);
///
/// assert_eq!(4503599627370496, result);
/// ```
///
/// ```
/// extern crate byte_unit;
///
/// let result = byte_unit::n_pib_bytes!(2.5, f64);
///
/// assert_eq!(2814749767106560, result);
/// ```
#[macro_export]
macro_rules! n_pib_bytes {
    () => {
        $crate::PEBIBYTE
    };
    ($x:expr) => {
        $crate::n_pib_bytes($x)
    };
    ($x:expr, $t:ty) => {
        $crate::n_gib_bytes($crate::_bytes_as!($x * ($crate::MEBIBYTE as $t)))
    };
}

/// Convert n EB to bytes.
///
/// ## Examples
///
/// ```
/// extern crate byte_unit;
///
/// let result = byte_unit::n_eb_bytes!(4);
///
/// assert_eq!(4000000000000000000, result);
/// ```
///
/// ```
/// extern crate byte_unit;
///
/// let result = byte_unit::n_eb_bytes!(2.5, f64);
///
/// assert_eq!(2500000000000000000, result);
/// ```
#[cfg(feature = "u128")]
#[macro_export]
macro_rules! n_eb_bytes {
    () => {
        $crate::EXABYTE
    };
    ($x:expr) => {
        $crate::n_eb_bytes($x)
    };
    ($x:expr, $t:ty) => {
        $crate::n_tb_bytes($crate::_bytes_as!($x * ($crate::MEGABYTE as $t)))
    };
}

/// Convert n EiB to bytes.
///
/// ## Examples
///
/// ```
/// extern crate byte_unit;
///
/// let result = byte_unit::n_eib_bytes!(4);
///
/// assert_eq!(4611686018427387904, result);
/// ```
///
/// ```
/// extern crate byte_unit;
///
/// let result = byte_unit::n_eib_bytes!(2.5, f64);
///
/// assert_eq!(2882303761517117440, result);
/// ```
#[cfg(feature = "u128")]
#[macro_export]
macro_rules! n_eib_bytes {
    () => {
        $crate::EXBIBYTE
    };
    ($x:expr) => {
        $crate::n_eib_bytes($x)
    };
    ($x:expr, $t:ty) => {
        $crate::n_tib_bytes($crate::_bytes_as!($x * ($crate::MEBIBYTE as $t)))
    };
}

/// Convert n ZB to bytes.
///
/// ## Examples
///
/// ```
/// extern crate byte_unit;
///
/// let result = byte_unit::n_zb_bytes!(4);
///
/// assert_eq!(4000000000000000000000, result);
/// ```
///
/// ```
/// extern crate byte_unit;
///
/// let result = byte_unit::n_zb_bytes!(2.5, f64);
///
/// assert_eq!(2500000000000000000000, result);
/// ```
#[cfg(feature = "u128")]
#[macro_export]
macro_rules! n_zb_bytes {
    () => {
        $crate::ZETTABYTE
    };
    ($x:expr) => {
        $crate::n_zb_bytes($x)
    };
    ($x:expr, $t:ty) => {
        $crate::n_pb_bytes($crate::_bytes_as!($x * ($crate::MEGABYTE as $t)))
    };
}

/// Convert n ZiB to bytes.
///
/// ## Examples
///
/// ```
/// extern crate byte_unit;
///
/// let result = byte_unit::n_zib_bytes!(4);
///
/// assert_eq!(4722366482869645213696, result);
/// ```
///
/// ```
/// extern crate byte_unit;
///
/// let result = byte_unit::n_zib_bytes!(2.5, f64);
///
/// assert_eq!(2951479051793528258560, result);
/// ```
#[cfg(feature = "u128")]
#[macro_export]
macro_rules! n_zib_bytes {
    () => {
        $crate::ZEBIBYTE
    };
    ($x:expr) => {
        $crate::n_zib_bytes($x)
    };
    ($x:expr, $t:ty) => {
        $crate::n_pib_bytes($crate::_bytes_as!($x * ($crate::MEBIBYTE as $t)))
    };
}
