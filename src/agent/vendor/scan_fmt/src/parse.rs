// Copyright 2015-2019 Will Lentz.
// Licensed under the MIT license.
use alloc::string::{String, ToString};

#[cfg(feature = "regex")]
use regex::Regex;

#[derive(Debug, PartialEq)]
enum FmtType {
    NonWhitespaceOrEnd,
    OnlyEnd,
    Pattern,
    Dec10,
    Hex16,
    Flt,
    #[cfg(feature = "regex")]
    Regex,
}

#[cfg(feature = "std")]
use std::error::Error;

use alloc::vec::Vec;
use core::fmt;

#[derive(Debug, PartialEq)]
pub struct ScanError(pub String);

#[cfg(feature = "std")]
impl Error for ScanError {}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Scan error: {}", self.0)
    }
}

// Handle the following format strings:
// {}X -> everything until whitespace or next character 'X'
// {s} -> everything until whitespace
// {d} -> only base-10 integers
// {x} -> only unsigned base-16 integers.  Allow 0xfff or fff
// {f} -> only floats
// {*} -> get token, but don't assign it to output
// {[]} -> only search for given characters
//         starting with '^' negates everything
//         ranges with '-' work.  To include '-' put it at end or start
//         to include ']' put it at the start (or right after ^)
//  e.g., {[^,]} -> match everything until next comma

// Make it slightly easier to scan through a Vec<>
struct VecScanner {
    data: Vec<char>,
    pos: usize,
    limit_pos: usize, // if non-0, then inc_limit() returns when 'pos' gets here
}

impl VecScanner {
    fn new(d: Vec<char>) -> VecScanner {
        VecScanner {
            data: d,
            pos: 0,
            limit_pos: 0,
        }
    }

    fn cur(&self) -> char {
        self.data[self.pos]
    }

    fn peek(&self, n: usize) -> Option<char> {
        if self.pos + n < self.data.len() {
            Some(self.data[self.pos + n])
        } else {
            None
        }
    }

    fn is_end(&self) -> bool {
        self.pos >= self.data.len()
    }

    // returns true if we have more data
    fn inc(&mut self) -> bool {
        self.pos += 1;
        !self.is_end()
    }

    // set the maximum position for inc_limit()
    fn start_inc_limit(&mut self, max_length: Option<usize>) {
        match max_length {
            Some(n) => {
                self.limit_pos = self.pos + n;
            }
            None => {
                self.limit_pos = 0;
            }
        }
    }

    fn hit_inc_limit(&mut self) -> bool {
        self.limit_pos > 0 && self.pos >= self.limit_pos
    }

    // same as inc(), but also honors start_inc_limit(max_length)
    fn inc_limit(&mut self) -> bool {
        self.pos += 1;
        !(self.is_end() || self.hit_inc_limit())
    }
}

fn is_whitespace(c: char) -> bool {
    match c {
        ' ' | '\t' | '\n' | '\r' => true,
        _ => false,
    }
}

// scan to past whitespace. Return false if end of input.
fn skip_whitespace(vs: &mut VecScanner) -> bool {
    while !vs.is_end() {
        if is_whitespace(vs.cur()) {
            vs.inc();
        } else {
            break;
        }
    }
    !vs.is_end()
}

struct FmtResult {
    data_type: FmtType,
    max_length: Option<usize>,
    store_result: bool,
    invert_char_list: bool,
    end_char: char,
    // Store pattern characters and ranges.  It might be worth
    // optimizing this if format strings are long.
    char_list: Vec<(char, char)>,
    #[cfg(feature = "regex")]
    regex: Option<Regex>,
}

// See top-level docs for allowed formats.
// Starts right after opening '{'.  Consumes characters to final }
// Note that '{' and '}' can exist unescaped inside [].
fn get_format(fstr: &mut VecScanner) -> Option<FmtResult> {
    let mut res = FmtResult {
        data_type: FmtType::NonWhitespaceOrEnd,
        max_length: None,
        end_char: ' ',
        store_result: true,
        invert_char_list: false,
        char_list: vec![],
        #[cfg(feature = "regex")]
        regex: None,
    };
    if fstr.cur() == '*' {
        res.store_result = false;
        if !fstr.inc() {
            return None;
        }
    }

    if fstr.cur() == '}' {
        if fstr.inc() {
            res.end_char = fstr.cur();
        }
        return Some(res);
    }

    // Read optional field width specifier (e.g., the "2" in {2d})
    let pos_start = fstr.pos;
    while fstr.cur().is_digit(10) {
        if !fstr.inc() {
            return None;
        }
    }
    if fstr.pos > pos_start {
        let max_length_string: String = fstr.data[pos_start..fstr.pos].iter().cloned().collect();
        res.max_length = max_length_string.parse::<usize>().ok();
    }

    match fstr.cur() {
        's' => { /* already FmtType::NonWhitespaceOrEnd */ }
        'e' => {
            res.data_type = FmtType::OnlyEnd;
        }
        'd' => {
            res.data_type = FmtType::Dec10;
        }
        'x' => {
            res.data_type = FmtType::Hex16;
        }
        'f' => {
            res.data_type = FmtType::Flt;
        }
        '[' => {
            res.data_type = FmtType::Pattern;
        }
        #[cfg(feature = "regex")]
        '/' => {
            res.data_type = FmtType::Regex;
        }
        _ => return None, // unexpected format
    }
    if !fstr.inc() {
        return None;
    }

    match res.data_type {
        FmtType::Pattern => handle_pattern(res, fstr),
        #[cfg(feature = "regex")]
        FmtType::Regex => handle_regex(res, fstr),
        _ => {
            if fstr.cur() != '}' {
                return None;
            }
            fstr.inc();
            Some(res)
        }
    }
}

fn handle_pattern(mut res: FmtResult, fstr: &mut VecScanner) -> Option<FmtResult> {
    // handle [] pattern
    res.data_type = FmtType::Pattern;

    if fstr.cur() == '^' {
        res.invert_char_list = true;
        if !fstr.inc() {
            return None;
        }
    }

    match fstr.cur() {
        ']' | '-' => {
            res.char_list.push((fstr.cur(), fstr.cur()));
            if !fstr.inc() {
                return None;
            }
        }
        _ => (),
    }

    // look for end of [] pattern
    while fstr.cur() != ']' {
        if fstr.peek(1) == Some('-') && fstr.peek(2) != Some(']') {
            let prev_char = fstr.cur();
            if !fstr.inc() {
                break;
            } // go to '-'
            if !fstr.inc() {
                break;
            } // go past '-'
              // add character range
            res.char_list.push((prev_char, fstr.cur()));
        } else {
            res.char_list.push((fstr.cur(), fstr.cur()));
        }
        if !fstr.inc() {
            return None;
        }
    }
    if !fstr.inc() {
        return None;
    } // go past ']'
    if fstr.cur() != '}' {
        return None;
    }
    fstr.inc(); // go past closing '}'

    Some(res)
}

#[cfg(feature = "regex")]
fn handle_regex(mut res: FmtResult, fstr: &mut VecScanner) -> Option<FmtResult> {
    let start = fstr.pos;
    let mut last_was_escape = false;
    while fstr.inc() {
        if fstr.cur() == '/' && !last_was_escape {
            break;
        }

        if fstr.cur() == '\\' {
            last_was_escape = true;
        } else {
            last_was_escape = false;
        }
    }
    if fstr.cur() != '/' {
        // invalid
        return None;
    }

    let substr = Some('^')
        .into_iter()
        .chain(fstr.data[start..fstr.pos].iter().cloned())
        .collect::<String>();

    if let Ok(re) = Regex::new(&substr) {
        res.regex = Some(re);
    } else {
        return None;
    }

    // consume close
    fstr.inc();
    if fstr.cur() != '}' {
        return None;
    }
    fstr.inc();

    Some(res)
}

fn scan_dec10(vs: &mut VecScanner, max_length: Option<usize>) {
    // look for [+-]{0,1}[0-9]+, up to max_length characters
    vs.start_inc_limit(max_length);
    scan_dec10_nest(vs);
}

// advance past base-10 decimal - assumes someone has called start_inc_limit()
fn scan_dec10_nest(vs: &mut VecScanner) {
    // look for [+-]{0,1}[0-9]+
    match vs.cur() {
        '+' | '-' => {
            if !vs.inc_limit() {
                return;
            }
        }
        _ => (),
    }

    while vs.cur().is_digit(10) {
        if !vs.inc_limit() {
            return;
        }
    }
}

// advance past base-16 hex
// look for (0x){0,1}[0-9a-fA-F]+
fn scan_hex16(vs: &mut VecScanner, max_length: Option<usize>) {
    vs.start_inc_limit(max_length);
    if vs.cur() == '0' {
        if !vs.inc_limit() {
            return;
        }
    }
    if vs.cur() == 'x' {
        if !vs.inc_limit() {
            return;
        }
    }
    while vs.cur().is_digit(16) {
        if !vs.inc_limit() {
            return;
        };
    }
}

// advance past float
// look for [+-]{0,1}[0-9]+
// then optional .[0-9]+
// then optional e[+-]{1}[0-9]+
fn scan_float(vs: &mut VecScanner, max_length: Option<usize>) {
    vs.start_inc_limit(max_length);
    scan_dec10_nest(vs);
    if vs.cur() == '.' {
        if !vs.inc_limit() {
            return;
        }
        while vs.cur().is_digit(10) {
            if !vs.inc_limit() {
                return;
            }
        }
    }
    if vs.cur() == 'e' {
        if !vs.inc_limit() {
            return;
        }
        scan_dec10_nest(vs);
    }
}

// advance until 'end' or whitespace
fn scan_nonws_or_end(vs: &mut VecScanner, end: char) {
    while !is_whitespace(vs.cur()) && vs.cur() != end {
        if !vs.inc() {
            return;
        }
    }
}

// advance past pattern
fn scan_pattern(vs: &mut VecScanner, fmt: &mut FmtResult) {
    // if invert, scan until character not in char_list
    // else scan while character is in char_list
    loop {
        let c = vs.cur();
        let mut found = false;
        for &(start, end) in fmt.char_list.iter() {
            if c >= start && c <= end {
                found = true;
                break;
            }
        }
        if found == fmt.invert_char_list {
            return;
        }
        if !vs.inc() {
            return;
        }
    }
}

#[cfg(feature = "regex")]
enum ReMatch {
    Captured { len: usize },
    NoCapture,
}

#[cfg(feature = "regex")]
fn scan_regex(vs: &mut VecScanner, fmt: &mut FmtResult) -> ReMatch {
    let re = fmt.regex.take().unwrap();
    let remainder = vs.data[vs.pos..].iter().cloned().collect::<String>();
    if let Some(mat) = re.captures(&remainder) {
        vs.pos += remainder[..mat.get(0).unwrap().end()].chars().count();
        if let Some(cap) = mat.get(1) {
            return ReMatch::Captured { len: cap.end() };
        }
    }
    return ReMatch::NoCapture;
}

// return data matching the format from user input (else "")
fn get_token(vs: &mut VecScanner, fmt: &mut FmtResult) -> String {
    let mut pos_start = vs.pos;
    match fmt.data_type {
        FmtType::OnlyEnd => {} // handled in scan()
        FmtType::NonWhitespaceOrEnd => scan_nonws_or_end(vs, fmt.end_char),
        FmtType::Dec10 => scan_dec10(vs, fmt.max_length),
        FmtType::Hex16 => scan_hex16(vs, fmt.max_length),
        FmtType::Flt => scan_float(vs, fmt.max_length),
        FmtType::Pattern => scan_pattern(vs, fmt),
        #[cfg(feature = "regex")]
        FmtType::Regex => {
            // if the regex has an internal group then we want to use the group
            // to select the substring, but either way the scan_regex function
            // will set pos to the end of the entire match consumed by the
            // regex
            match scan_regex(vs, fmt) {
                ReMatch::Captured { len } => {
                    return vs.data[pos_start..pos_start + len]
                        .iter()
                        .cloned()
                        .collect();
                }
                ReMatch::NoCapture => {}
            }
        }
    }
    if fmt.data_type == FmtType::Dec10 || fmt.data_type == FmtType::Flt {
        // parse<i32/f32> won't accept "+" in front of numbers
        if vs.data[pos_start] == '+' {
            pos_start += 1;
        }
    }
    vs.data[pos_start..vs.pos].iter().cloned().collect()
}

// Extract String tokens from the input string based on
// the format string.  See lib.rs for more info.
// Returns an iterator of the String results.
pub fn scan(input_string: &str, format: &str) -> alloc::vec::IntoIter<String> {
    let mut res: Vec<String> = vec![];
    let mut fmtstr = VecScanner::new(format.chars().collect());
    let mut instr = VecScanner::new(input_string.chars().collect());
    loop {
        let mut do_compare = true;
        if !skip_whitespace(&mut fmtstr) {
            break;
        }
        if !skip_whitespace(&mut instr) {
            break;
        }

        if fmtstr.cur() == '{' {
            if !fmtstr.inc() {
                break;
            }
            if fmtstr.cur() == '{' {
                // got an escaped {{
            } else {
                let fmt = get_format(&mut fmtstr);
                let mut fmt = if let Some(fmt) = fmt {
                    fmt
                } else {
                    break;
                };

                if fmt.data_type == FmtType::OnlyEnd && !instr.is_end() {
                    // we didn't get an end of input where expected, so invalidate any matches
                    return vec![String::from("")].into_iter();
                }
                let data = get_token(&mut instr, &mut fmt);
                if fmt.store_result {
                    if fmt.data_type == FmtType::Hex16 {
                        let no_prefix = data.trim_start_matches("0x");
                        res.push(no_prefix.to_string());
                    } else {
                        res.push(data);
                    }
                }
                do_compare = false;
            }
        } else {
            if fmtstr.cur() == '}' {
                // handle escaped }} by skipping first '}'
                if !fmtstr.inc() {
                    break;
                }
            }
        }
        if do_compare {
            if fmtstr.cur() != instr.cur() {
                return vec![String::from("")].into_iter();
                // we had a non match! --> if we only break here we will return all matches found so far.
                // This will create a misbehaviour when there is something like `{d}in` as the in is not cared for.
            }
            if !fmtstr.inc() {
                break;
            }
            if !instr.inc() {
                break;
            }
        }
    }
    res.into_iter()
}

#[test]
fn test_simple() {
    let mut res = scan(" data 42-12=30", "data {d}-{d}={d}");
    assert_eq!(res.next().unwrap(), "42");
    assert_eq!(res.next().unwrap(), "12");
    assert_eq!(res.next().unwrap(), "30");
    assert_eq!(res.next(), None);
}

#[test]
fn test_plus_sign() {
    let mut res = scan("+42", "{d}");
    assert_eq!(res.next().unwrap(), "42");
    let mut res = scan("+42.7", "{f}");
    assert_eq!(res.next().unwrap(), "42.7");
}

#[test]
fn test_complex() {
    let mut res = scan(
        "test{123  bye -456} hi  -22.7e-1 +1.23fg",
        "test{{{d} bye {}}} hi {f} {f}",
    );
    assert_eq!(res.next().unwrap(), "123");
    assert_eq!(res.next().unwrap(), "-456");
    assert_eq!(res.next().unwrap(), "-22.7e-1");
    assert_eq!(res.next().unwrap(), "1.23");
    assert_eq!(res.next(), None);
}

#[test]
fn test_endline() {
    let mut res = scan("hi 15.7\r\n", "{} {}");
    assert_eq!(res.next().unwrap(), "hi");
    assert_eq!(res.next().unwrap(), "15.7");
}

#[test]
fn test_hex() {
    let mut res = scan("hi 0x15 ff fg", "hi {x} {x} {x}");
    assert_eq!(res.next().unwrap(), "15");
    assert_eq!(res.next().unwrap(), "ff");
    assert_eq!(res.next().unwrap(), "f");
}

#[test]
fn test_string() {
    let mut res = scan("The quick brown fox", "{s}{s} {}n {s}x");
    assert_eq!(res.next().unwrap(), "The");
    assert_eq!(res.next().unwrap(), "quick");
    assert_eq!(res.next().unwrap(), "brow");
    assert_eq!(res.next().unwrap(), "fox");
}

#[test]
fn test_pattern() {
    let mut res = scan(
        "hi abcdefghijklmnop 0123456789",
        "hi {[a-l]}{[^a-l ]} {[01234-8]}{[9]}",
    );
    assert_eq!(res.next().unwrap(), "abcdefghijkl");
    assert_eq!(res.next().unwrap(), "mnop");
    assert_eq!(res.next().unwrap(), "012345678");
    assert_eq!(res.next().unwrap(), "9");

    let mut res = scan("xyz  01234567λ89", "xyz {[40-3]}{*[65]}{[7-78-9λ]}");
    assert_eq!(res.next().unwrap(), "01234");
    assert_eq!(res.next().unwrap(), "7λ89");
}

#[test]
fn test_width() {
    let mut res = scan("01123fe071 432", "{2d}{3d}{4x}{2d} {3d}");
    assert_eq!(res.next().unwrap(), "01");
    assert_eq!(res.next().unwrap(), "123");
    assert_eq!(res.next().unwrap(), "fe07");
    assert_eq!(res.next().unwrap(), "1");
    assert_eq!(res.next().unwrap(), "432");
}

#[test]
fn match_end() {
    let mut res = scan("12 hi", "{d} hi{e}");
    assert_eq!(res.next().unwrap(), "12");
    assert_eq!(res.next(), None);
    let mut res = scan("12 hi2", "{d} hi{e}");
    assert_eq!(res.next().unwrap(), "");
}

#[cfg(all(test, feature = "regex"))]
mod test_regex {
    use super::scan;

    #[test]
    fn simple() {
        let mut res = scan("one (hello) two", "one ({/[^)]+/}) two");
        assert_eq!(res.next().unwrap(), "hello");
    }

    #[test]
    fn mixed_regex_and_pattern() {
        let mut res = scan("one ((hello)) two", r#"one ({/[^)]+\)?/}) two"#);
        assert_eq!(res.next().unwrap(), "(hello)");
    }

    #[test]
    fn bad_pattern() {
        // note the extra close paren
        let mut scanner = scan("one (hello)) two", "one ({/[^)]+/}) two");
        assert_eq!(scanner.next().unwrap(), "");
    }

    #[test]
    fn uses_group_if_present() {
        let mut res = scan("one (((hello))) two", r#"one {/(\(.*\)) /}two"#);
        assert_eq!(res.next().unwrap(), "(((hello)))");
    }

    #[test]
    fn unicode() {
        let mut res = scan("й", "{/.*/}");
        assert_eq!(res.next().unwrap(), "й");
    }
}
