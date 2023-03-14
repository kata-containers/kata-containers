use std::fmt;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum LexicalError {
}

impl fmt::Display for LexicalError {
    // This trait requires `fmt` with this exact signature.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("{}")
    }
}

pub type Spanned<Token, Loc, LexicalError>
    = Result<(Loc, Token, Loc), LexicalError>;

// The type of the parser's input.
//
// The parser iterators over tuples consisting of the token's starting
// position, the token itself, and the token's ending position.
pub(crate) type LexerItem<Token, Loc, LexicalError>
    = Spanned<Token, Loc, LexicalError>;

/// The components of an OpenPGP Message.
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::upper_case_acronyms)]
pub enum Token {
    PIPE,

    STAR,
    PLUS,
    QUESTION,

    LPAREN,
    RPAREN,

    DOT,
    CARET,
    DOLLAR,
    BACKSLASH,

    LBRACKET,
    RBRACKET,
    DASH,

    OTHER(char),
}
assert_send_and_sync!(Token);

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&format!("{:?}", self)[..])
    }
}

impl From<Token> for String {
    fn from(t: Token) -> String {
        use self::Token::*;
        match t {
            PIPE => '|'.to_string(),
            STAR => '*'.to_string(),
            PLUS => '+'.to_string(),
            QUESTION => '?'.to_string(),
            LPAREN => '('.to_string(),
            RPAREN => ')'.to_string(),
            DOT => '.'.to_string(),
            CARET => '^'.to_string(),
            DOLLAR => '$'.to_string(),
            BACKSLASH => '\\'.to_string(),
            LBRACKET => '['.to_string(),
            RBRACKET => ']'.to_string(),
            DASH => '-'.to_string(),
            OTHER(c) => c.to_string(),
        }
    }
}

impl Token {
    pub fn to_char(&self) -> char {
        use self::Token::*;
        match self {
            PIPE => '|',
            STAR => '*',
            PLUS => '+',
            QUESTION => '?',
            LPAREN => '(',
            RPAREN => ')',
            DOT => '.',
            CARET => '^',
            DOLLAR => '$',
            BACKSLASH => '\\',
            LBRACKET => '[',
            RBRACKET => ']',
            DASH => '-',
            OTHER(c) => *c,
        }
    }
}

pub(crate) struct Lexer<'input> {
    offset: usize,
    input: &'input str,
}

impl<'input> Lexer<'input> {
    pub fn new(input: &'input str) -> Self {
        Lexer { offset: 0, input }
    }
}

impl<'input> Iterator for Lexer<'input> {
    type Item = LexerItem<Token, usize, LexicalError>;

    fn next(&mut self) -> Option<Self::Item> {
        use self::Token::*;

        tracer!(super::TRACE, "regex::Lexer::next");

        // Returns the length of the first character in s in bytes.
        // If s is empty, returns 0.
        fn char_bytes(s: &str) -> usize {
            if let Some(c) = s.chars().next() {
                c.len_utf8()
            } else {
                0
            }
        }

        let one = |input: &'input str| -> Option<Token> {
            let c = input.chars().next()?;
            Some(match c {
                '|' => PIPE,
                '*' => STAR,
                '+' => PLUS,
                '?' => QUESTION,
                '(' => LPAREN,
                ')' => RPAREN,
                '.' => DOT,
                '^' => CARET,
                '$' => DOLLAR,
                '\\' => BACKSLASH,
                '[' => LBRACKET,
                ']' => RBRACKET,
                '-' => DASH,
                _ => OTHER(c),
            })
        };

        let l = char_bytes(self.input);
        let t = match one(self.input) {
            Some(t) => t,
            None => return None,
        };

        self.input = &self.input[l..];

        let start = self.offset;
        let end = start + l;
        self.offset += l;

        t!("Returning token at offset {}: '{:?}'",
           start, t);

        Some(Ok((start, t, end)))
    }
}

impl<'input> From<&'input str> for Lexer<'input> {
    fn from(i: &'input str) -> Lexer<'input> {
        Lexer::new(i)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexer() {
        fn lex(s: &str, expected: &[Token]) {
            let tokens: Vec<Token> = Lexer::new(s)
                .map(|t| t.unwrap().1)
                .collect();

            assert_eq!(&tokens[..], expected,
                       "{}", s);
        }

        use Token::*;
        lex("|", &[ PIPE ]);
        lex("*", &[ STAR ]);
        lex("+", &[ PLUS ]);
        lex("?", &[ QUESTION ]);
        lex("(", &[ LPAREN ]);
        lex(")", &[ RPAREN ]);
        lex(".", &[ DOT ]);
        lex("^", &[ CARET ]);
        lex("$", &[ DOLLAR ]);
        lex("\\", &[ BACKSLASH ]);
        lex("[", &[ LBRACKET ]);
        lex("]", &[ RBRACKET ]);
        lex("-", &[ DASH ]);
        lex("a", &[ OTHER('a') ]);
        lex("aa", &[ OTHER('a'), OTHER('a') ]);
        lex("foo", &[ OTHER('f'), OTHER('o'), OTHER('o') ]);

        lex("foo\\bar", &[ OTHER('f'), OTHER('o'), OTHER('o'),
                           BACKSLASH,
                           OTHER('b'), OTHER('a'), OTHER('r') ]);
        lex("*?!", &[ STAR, QUESTION, OTHER('!') ]);

        // Multi-byte UTF-8.
        lex("ßℝ💣", &[ OTHER('ß'), OTHER('ℝ'), OTHER('💣'), ]);
        lex("(ß|ℝ|💣",
            &[ LPAREN, OTHER('ß'), PIPE, OTHER('ℝ'), PIPE, OTHER('💣') ]);
        lex("東京", &[ OTHER('東'), OTHER('京') ]);
    }
}
