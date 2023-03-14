pub struct Captures<'a> {
    pub begin: &'a [u8],
    pub data: &'a [u8],
    pub end: &'a [u8],
}

pub fn parse_captures<'a>(input: &'a [u8]) -> Option<Captures<'a>> {
    parser_inner(input).map(|(_, cap)| cap)
}
pub fn parse_captures_iter<'a>(input: &'a [u8]) -> CaptureMatches<'a> {
    CaptureMatches { input }
}

pub struct CaptureMatches<'a> {
    input: &'a [u8],
}
impl<'a> Iterator for CaptureMatches<'a> {
    type Item = Captures<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.input.is_empty() {
            return None;
        }
        match parser_inner(self.input) {
            Some((remaining, captures)) => {
                self.input = remaining;
                Some(captures)
            }
            None => {
                self.input = &[];
                None
            }
        }
    }
}

fn parser_inner<'a>(input: &'a [u8]) -> Option<(&'a [u8], Captures<'a>)> {
    // Should be equivalent to the regex
    // "(?s)-----BEGIN (?P<begin>.*?)-----[ \t\n\r]*(?P<data>.*?)-----END (?P<end>.*?)-----[ \t\n\r]*"

    // (?s)                                      # Enable dotall (. matches all characters incl \n)
    // -----BEGIN (?P<begin>.*?)-----[ \t\n\r]*  # Parse begin
    // (?P<data>.*?)                             # Parse data
    // -----END (?P<end>.*?)-----[ \t\n\r]*      # Parse end

    let (input, _) = read_until(input, b"-----BEGIN ")?;
    let (input, begin) = read_until(input, b"-----")?;
    let input = skip_whitespace(input);
    let (input, data) = read_until(input, b"-----END ")?;
    let (remaining, end) = read_until(input, b"-----")?;
    let remaining = skip_whitespace(remaining);

    let captures = Captures { begin, data, end };
    Some((remaining, captures))
}

// Equivalent to the regex [ \t\n\r]*
fn skip_whitespace(mut input: &[u8]) -> &[u8] {
    while let Some(b) = input.first() {
        match b {
            b' ' | b'\t' | b'\n' | b'\r' => {
                input = &input[1..];
            }
            _ => break,
        }
    }
    input
}
// Equivalent to (.*?) followed by a string
// Returns the remaining input (after the secondary matched string) and the matched data
fn read_until<'a, 'b>(input: &'a [u8], marker: &'b [u8]) -> Option<(&'a [u8], &'a [u8])> {
    // If there is no end condition, short circuit
    if marker.is_empty() {
        return Some((&[], input));
    }
    let mut index = 0;
    let mut found = 0;
    while input.len() - index >= marker.len() - found {
        if input[index] == marker[found] {
            found += 1;
        } else {
            found = 0;
        }
        index += 1;
        if found == marker.len() {
            let remaining = &input[index..];
            let matched = &input[..index - found];
            return Some((remaining, matched));
        }
    }
    None
}
