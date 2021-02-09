//! Retreive the file-max value from /proc/sys/fs/file-max

use std::fs::File;
use std::io::Result;

use parsers::{map_result, parse_u64, read_to_end};
use nom::eol;

/// Path to the file-max value
static FILE_MAX_PATH: &'static str = "/proc/sys/fs/file-max";

// Linux kernel uses get_max_files() which returns an unsigned long
// see include/linux/fs.h

named!(parse_file_max<u64>,
    do_parse!(max: parse_u64 >> eol >> (max))
);

/// Get file-max value for the current system
pub fn file_max() -> Result<u64> {
    let mut buf = [0;32];
    let mut file = try!(File::open(FILE_MAX_PATH));
    map_result(parse_file_max(try!(read_to_end(&mut file, &mut buf))))
}

#[cfg(test)]
pub mod tests {
    use super::file_max;

    #[test]
    fn test_file_max() {
        let max = file_max();
        assert_eq!(max.is_ok(), true);
    }
}
