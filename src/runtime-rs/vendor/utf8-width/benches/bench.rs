use std::fs;

use bencher::{benchmark_group, benchmark_main, Bencher};

#[cfg(unix)]
const TEXT_PATH: &str = "benches/data/wikipedia-rust.txt";

#[cfg(windows)]
const TEXT_PATH: &str = r"benches\data\wikipedia-rust.txt";

static UTF8_CHAR_WIDTH: [usize; 256] = [
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, // 0x1F
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, // 0x3F
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, // 0x5F
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, // 0x7F
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, // 0x9F
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, // 0xBF
    0, 0, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    2, // 0xDF
    3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, // 0xEF
    4, 4, 4, 4, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 0xFF
];

fn retrieve_get_width(bencher: &mut Bencher) {
    let bytes = fs::read(TEXT_PATH).unwrap();
    let length = bytes.len();

    bencher.iter(|| {
        let mut widths = Vec::new();

        let mut p = 0;

        loop {
            let e = bytes[p];

            let width = utf8_width::get_width(e);

            widths.push(width);

            p += width;

            if p == length {
                break;
            }
        }

        widths
    });

    bencher.bytes = length as u64;
}

fn retrieve_get_width_assume_valid(bencher: &mut Bencher) {
    let bytes = fs::read(TEXT_PATH).unwrap();
    let length = bytes.len();

    bencher.iter(|| {
        let mut widths = Vec::new();

        let mut p = 0;
        let length = bytes.len();

        loop {
            let e = bytes[p];

            let width = unsafe { utf8_width::get_width_assume_valid(e) };

            widths.push(width);

            p += width;

            if p == length {
                break;
            }
        }

        widths
    });

    bencher.bytes = length as u64;
}

fn retrieve_get_width_by_looking_table(bencher: &mut Bencher) {
    let bytes = fs::read(TEXT_PATH).unwrap();
    let length = bytes.len();

    bencher.iter(|| {
        let mut widths = Vec::new();

        let mut p = 0;
        let length = bytes.len();

        loop {
            let e = bytes[p];

            let width = UTF8_CHAR_WIDTH[e as usize];

            widths.push(width);

            p += width;

            if p == length {
                break;
            }
        }

        widths
    });

    bencher.bytes = length as u64;
}

fn retrieve_get_width_by_chars(bencher: &mut Bencher) {
    let text = fs::read_to_string(TEXT_PATH).unwrap();
    let length = text.len();

    bencher.iter(|| {
        let mut widths = Vec::new();

        for c in text.chars() {
            widths.push(c.len_utf8())
        }

        widths
    });

    bencher.bytes = length as u64;
}

benchmark_group!(
    get_width,
    retrieve_get_width,
    retrieve_get_width_assume_valid,
    retrieve_get_width_by_looking_table,
    retrieve_get_width_by_chars
);
benchmark_main!(get_width);
