use common::{self, numeric_identifier, letters_numbers_dash_dot};
use version::Identifier;
use std::str::{FromStr, from_utf8};
use recognize::*;

#[derive(Debug)]
pub struct VersionReq {
    pub predicates: Vec<Predicate>,
}

#[derive(PartialEq,Debug)]
pub enum WildcardVersion {
    Major,
    Minor,
    Patch,
}

#[derive(PartialEq,Debug)]
pub enum Op {
    Ex, // Exact
    Gt, // Greater than
    GtEq, // Greater than or equal to
    Lt, // Less than
    LtEq, // Less than or equal to
    Tilde, // e.g. ~1.0.0
    Compatible, // compatible by definition of semver, indicated by ^
    Wildcard(WildcardVersion), // x.y.*, x.*, *
}

impl FromStr for Op {
    type Err = String;

    fn from_str(s: &str) -> Result<Op, String> {
        match s {
            "=" => Ok(Op::Ex),
            ">" => Ok(Op::Gt),
            ">=" => Ok(Op::GtEq),
            "<" => Ok(Op::Lt),
            "<=" => Ok(Op::LtEq),
            "~" => Ok(Op::Tilde),
            "^" => Ok(Op::Compatible),
            _ => Err(String::from("Could not parse Op")),
        }
    }
}

#[derive(PartialEq,Debug)]
pub struct Predicate {
    pub op: Op,
    pub major: u64,
    pub minor: Option<u64>,
    pub patch: Option<u64>,
    pub pre: Vec<Identifier>,
}

fn numeric_or_wild(s: &[u8]) -> Option<(Option<u64>, usize)> {
    if let Some((val, len)) = numeric_identifier(s) {
        Some((Some(val), len))
    } else if let Some(len) = OneOf(b"*xX").p(s) {
        Some((None, len))
    } else {
        None
    }
}

fn dot_numeric_or_wild(s: &[u8]) -> Option<(Option<u64>, usize)> {
    b'.'.p(s).and_then(|len|
        numeric_or_wild(&s[len..]).map(|(val, len2)| (val, len + len2))
    )
}

fn operation(s: &[u8]) -> Option<(Op, usize)> {
    if let Some(len) = "=".p(s) {
        Some((Op::Ex, len))
    } else if let Some(len) = ">=".p(s) {
        Some((Op::GtEq, len))
    } else if let Some(len) = ">".p(s) {
        Some((Op::Gt, len))
    } else if let Some(len) = "<=".p(s) {
        Some((Op::LtEq, len))
    } else if let Some(len) = "<".p(s) {
        Some((Op::Lt, len))
    } else if let Some(len) = "~".p(s) {
        Some((Op::Tilde, len))
    } else if let Some(len) = "^".p(s) {
        Some((Op::Compatible, len))
    } else {
        None
    }
}

fn whitespace(s: &[u8]) -> Option<usize> {
    ZeroOrMore(OneOf(b"\t\r\n ")).p(s)
}

pub fn parse_predicate(range: &str) -> Result<Predicate, String> {
    let s = range.trim().as_bytes();
    let mut i = 0;
    let mut operation = if let Some((op, len)) = operation(&s[i..]) {
        i += len;
        op
    } else {
        // operations default to Compatible
        Op::Compatible
    };
    if let Some(len) = whitespace.p(&s[i..]) {
        i += len;
    }
    let major = if let Some((major, len)) = numeric_identifier(&s[i..]) {
        i += len;
        major
    } else {
        return Err("Error parsing major version number: ".to_string());
    };
    let minor = if let Some((minor, len)) = dot_numeric_or_wild(&s[i..]) {
        i += len;
        if minor.is_none() {
            operation = Op::Wildcard(WildcardVersion::Minor);
        }
        minor
    } else {
        None
    };
    let patch = if let Some((patch, len)) = dot_numeric_or_wild(&s[i..]) {
        i += len;
        if patch.is_none() {
            operation = Op::Wildcard(WildcardVersion::Patch);
        }
        patch
    } else {
        None
    };
    let (pre, pre_len) = common::parse_optional_meta(&s[i..], b'-')?;
    i += pre_len;
    if let Some(len) = (b'+', letters_numbers_dash_dot).p(&s[i..]) {
        i += len;
    }
    if i != s.len() {
        return Err("Extra junk after valid predicate: ".to_string() +
            from_utf8(&s[i..]).unwrap());
    }
    Ok(Predicate {
        op: operation,
        major: major,
        minor: minor,
        patch: patch,
        pre: pre,
    })
}

pub fn parse(ranges: &str) -> Result<VersionReq, String> {
    // null is an error
    if ranges == "\0" {
        return Err(String::from("Null is not a valid VersionReq"));
    }

    // an empty range is a major version wildcard
    // so is a lone * or x of either capitalization
    if (ranges == "")
    || (ranges == "*")
    || (ranges == "x")
    || (ranges == "X") {
        return Ok(VersionReq {
            predicates: vec![Predicate {
                op: Op::Wildcard(WildcardVersion::Major),
                major: 0,
                minor: None,
                patch: None,
                pre: Vec::new(),
            }],
        });
    }


    let ranges = ranges.trim();

    let predicates: Result<Vec<_>, String> = ranges
        .split(",")
        .map(|range| {
            parse_predicate(range)
        })
        .collect();

    let predicates = try!(predicates);

    if predicates.len() == 0 {
        return Err(String::from("VersionReq did not parse properly"));
    }

    Ok(VersionReq {
        predicates: predicates,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use range;
    use version::Identifier;

    #[test]
    fn test_parsing_default() {
        let r = range::parse("1.0.0").unwrap();

        assert_eq!(Predicate {
                op: Op::Compatible,
                major: 1,
                minor: Some(0),
                patch: Some(0),
                pre: Vec::new(),
            },
            r.predicates[0]
        );
    }

    #[test]
    fn test_parsing_exact_01() {
        let r = range::parse("=1.0.0").unwrap();

        assert_eq!(Predicate {
                op: Op::Ex,
                major: 1,
                minor: Some(0),
                patch: Some(0),
                pre: Vec::new(),
            },
            r.predicates[0]
        );
    }

    #[test]
    fn test_parsing_exact_02() {
        let r = range::parse("=0.9.0").unwrap();

        assert_eq!(Predicate {
                op: Op::Ex,
                major: 0,
                minor: Some(9),
                patch: Some(0),
                pre: Vec::new(),
            },
            r.predicates[0]
        );
    }

    #[test]
    fn test_parsing_exact_03() {
        let r = range::parse("=0.1.0-beta2.a").unwrap();

        assert_eq!(Predicate {
                op: Op::Ex,
                major: 0,
                minor: Some(1),
                patch: Some(0),
                pre: vec![Identifier::AlphaNumeric(String::from("beta2")),
                          Identifier::AlphaNumeric(String::from("a"))],
            },
            r.predicates[0]
        );
    }

    #[test]
    pub fn test_parsing_greater_than() {
        let r = range::parse("> 1.0.0").unwrap();

        assert_eq!(Predicate {
                op: Op::Gt,
                major: 1,
                minor: Some(0),
                patch: Some(0),
                pre: Vec::new(),
            },
            r.predicates[0]
        );
    }

    #[test]
    pub fn test_parsing_greater_than_01() {
        let r = range::parse(">= 1.0.0").unwrap();

        assert_eq!(Predicate {
                op: Op::GtEq,
                major: 1,
                minor: Some(0),
                patch: Some(0),
                pre: Vec::new(),
            },
            r.predicates[0]
        );
    }

    #[test]
    pub fn test_parsing_greater_than_02() {
        let r = range::parse(">= 2.1.0-alpha2").unwrap();

        assert_eq!(Predicate {
                op: Op::GtEq,
                major: 2,
                minor: Some(1),
                patch: Some(0),
                pre: vec![Identifier::AlphaNumeric(String::from("alpha2"))],
            },
            r.predicates[0]
        );
    }

    #[test]
    pub fn test_parsing_less_than() {
        let r = range::parse("< 1.0.0").unwrap();

        assert_eq!(Predicate {
                op: Op::Lt,
                major: 1,
                minor: Some(0),
                patch: Some(0),
                pre: Vec::new(),
            },
            r.predicates[0]
        );
    }

    #[test]
    pub fn test_parsing_less_than_eq() {
        let r = range::parse("<= 2.1.0-alpha2").unwrap();

        assert_eq!(Predicate {
                op: Op::LtEq,
                major: 2,
                minor: Some(1),
                patch: Some(0),
                pre: vec![Identifier::AlphaNumeric(String::from("alpha2"))],
            },
            r.predicates[0]
        );
    }

    #[test]
    pub fn test_parsing_tilde() {
        let r = range::parse("~1").unwrap();

        assert_eq!(Predicate {
                op: Op::Tilde,
                major: 1,
                minor: None,
                patch: None,
                pre: Vec::new(),
            },
            r.predicates[0]
        );
    }

    #[test]
    pub fn test_parsing_compatible() {
        let r = range::parse("^0").unwrap();

        assert_eq!(Predicate {
                op: Op::Compatible,
                major: 0,
                minor: None,
                patch: None,
                pre: Vec::new(),
            },
            r.predicates[0]
        );
    }

    #[test]
    fn test_parsing_blank() {
        let r = range::parse("").unwrap();

        assert_eq!(Predicate {
                op: Op::Wildcard(WildcardVersion::Major),
                major: 0,
                minor: None,
                patch: None,
                pre: Vec::new(),
            },
            r.predicates[0]
        );
    }

    #[test]
    fn test_parsing_wildcard() {
        let r = range::parse("*").unwrap();

        assert_eq!(Predicate {
                op: Op::Wildcard(WildcardVersion::Major),
                major: 0,
                minor: None,
                patch: None,
                pre: Vec::new(),
            },
            r.predicates[0]
        );
    }

    #[test]
    fn test_parsing_x() {
        let r = range::parse("x").unwrap();

        assert_eq!(Predicate {
                op: Op::Wildcard(WildcardVersion::Major),
                major: 0,
                minor: None,
                patch: None,
                pre: Vec::new(),
            },
            r.predicates[0]
        );
    }

    #[test]
    fn test_parsing_capital_x() {
        let r = range::parse("X").unwrap();

        assert_eq!(Predicate {
                op: Op::Wildcard(WildcardVersion::Major),
                major: 0,
                minor: None,
                patch: None,
                pre: Vec::new(),
            },
            r.predicates[0]
        );
    }

    #[test]
    fn test_parsing_minor_wildcard_star() {
        let r = range::parse("1.*").unwrap();

        assert_eq!(Predicate {
                op: Op::Wildcard(WildcardVersion::Minor),
                major: 1,
                minor: None,
                patch: None,
                pre: Vec::new(),
            },
            r.predicates[0]
        );
    }

    #[test]
    fn test_parsing_minor_wildcard_x() {
        let r = range::parse("1.x").unwrap();

        assert_eq!(Predicate {
                op: Op::Wildcard(WildcardVersion::Minor),
                major: 1,
                minor: None,
                patch: None,
                pre: Vec::new(),
            },
            r.predicates[0]
        );
    }

    #[test]
    fn test_parsing_minor_wildcard_capital_x() {
        let r = range::parse("1.X").unwrap();

        assert_eq!(Predicate {
                op: Op::Wildcard(WildcardVersion::Minor),
                major: 1,
                minor: None,
                patch: None,
                pre: Vec::new(),
            },
            r.predicates[0]
        );
    }

    #[test]
    fn test_parsing_patch_wildcard_star() {
        let r = range::parse("1.2.*").unwrap();

        assert_eq!(Predicate {
                op: Op::Wildcard(WildcardVersion::Patch),
                major: 1,
                minor: Some(2),
                patch: None,
                pre: Vec::new(),
            },
            r.predicates[0]
        );
    }

    #[test]
    fn test_parsing_patch_wildcard_x() {
        let r = range::parse("1.2.x").unwrap();

        assert_eq!(Predicate {
                op: Op::Wildcard(WildcardVersion::Patch),
                major: 1,
                minor: Some(2),
                patch: None,
                pre: Vec::new(),
            },
            r.predicates[0]
        );
    }

    #[test]
    fn test_parsing_patch_wildcard_capital_x() {
        let r = range::parse("1.2.X").unwrap();

        assert_eq!(Predicate {
                op: Op::Wildcard(WildcardVersion::Patch),
                major: 1,
                minor: Some(2),
                patch: None,
                pre: Vec::new(),
            },
            r.predicates[0]
        );
    }

    #[test]
    pub fn test_multiple_01() {
        let r = range::parse("> 0.0.9, <= 2.5.3").unwrap();

        assert_eq!(Predicate {
                op: Op::Gt,
                major: 0,
                minor: Some(0),
                patch: Some(9),
                pre: Vec::new(),
            },
            r.predicates[0]
        );

        assert_eq!(Predicate {
                op: Op::LtEq,
                major: 2,
                minor: Some(5),
                patch: Some(3),
                pre: Vec::new(),
            },
            r.predicates[1]
        );
    }

    #[test]
    pub fn test_multiple_02() {
        let r = range::parse("0.3.0, 0.4.0").unwrap();

        assert_eq!(Predicate {
                op: Op::Compatible,
                major: 0,
                minor: Some(3),
                patch: Some(0),
                pre: Vec::new(),
            },
            r.predicates[0]
        );

        assert_eq!(Predicate {
                op: Op::Compatible,
                major: 0,
                minor: Some(4),
                patch: Some(0),
                pre: Vec::new(),
            },
            r.predicates[1]
        );
    }

    #[test]
    pub fn test_multiple_03() {
        let r = range::parse("<= 0.2.0, >= 0.5.0").unwrap();

        assert_eq!(Predicate {
                op: Op::LtEq,
                major: 0,
                minor: Some(2),
                patch: Some(0),
                pre: Vec::new(),
            },
            r.predicates[0]
        );

        assert_eq!(Predicate {
                op: Op::GtEq,
                major: 0,
                minor: Some(5),
                patch: Some(0),
                pre: Vec::new(),
            },
            r.predicates[1]
        );
    }

    #[test]
    pub fn test_multiple_04() {
        let r = range::parse("0.1.0, 0.1.4, 0.1.6").unwrap();

        assert_eq!(Predicate {
                op: Op::Compatible,
                major: 0,
                minor: Some(1),
                patch: Some(0),
                pre: Vec::new(),
            },
            r.predicates[0]
        );

        assert_eq!(Predicate {
                op: Op::Compatible,
                major: 0,
                minor: Some(1),
                patch: Some(4),
                pre: Vec::new(),
            },
            r.predicates[1]
        );

        assert_eq!(Predicate {
                op: Op::Compatible,
                major: 0,
                minor: Some(1),
                patch: Some(6),
                pre: Vec::new(),
            },
            r.predicates[2]
        );
    }

    #[test]
    pub fn test_multiple_05() {
        let r = range::parse(">=0.5.1-alpha3, <0.6").unwrap();

        assert_eq!(Predicate {
                op: Op::GtEq,
                major: 0,
                minor: Some(5),
                patch: Some(1),
                pre: vec![Identifier::AlphaNumeric(String::from("alpha3"))],
            },
            r.predicates[0]
        );

        assert_eq!(Predicate {
                op: Op::Lt,
                major: 0,
                minor: Some(6),
                patch: None,
                pre: Vec::new(),
            },
            r.predicates[1]
        );
    }

    #[test]
    fn test_parse_build_metadata_with_predicate() {
        assert_eq!(range::parse("^1.2.3+meta").unwrap().predicates[0].op,
                   Op::Compatible);
        assert_eq!(range::parse("~1.2.3+meta").unwrap().predicates[0].op,
                   Op::Tilde);
        assert_eq!(range::parse("=1.2.3+meta").unwrap().predicates[0].op,
                   Op::Ex);
        assert_eq!(range::parse("<=1.2.3+meta").unwrap().predicates[0].op,
                   Op::LtEq);
        assert_eq!(range::parse(">=1.2.3+meta").unwrap().predicates[0].op,
                   Op::GtEq);
        assert_eq!(range::parse("<1.2.3+meta").unwrap().predicates[0].op,
                   Op::Lt);
        assert_eq!(range::parse(">1.2.3+meta").unwrap().predicates[0].op,
                   Op::Gt);
    }

    #[test]
    pub fn test_parse_errors() {
        assert!(range::parse("\0").is_err());
        assert!(range::parse(">= >= 0.0.2").is_err());
        assert!(range::parse(">== 0.0.2").is_err());
        assert!(range::parse("a.0.0").is_err());
        assert!(range::parse("1.0.0-").is_err());
        assert!(range::parse(">=").is_err());
        assert!(range::parse("> 0.1.0,").is_err());
        assert!(range::parse("> 0.3.0, ,").is_err());
    }

    #[test]
    pub fn test_large_major_version() {
        assert!(range::parse("18446744073709551617.0.0").is_err());
    }

    #[test]
    pub fn test_large_minor_version() {
        assert!(range::parse("0.18446744073709551617.0").is_err());
    }

    #[test]
    pub fn test_large_patch_version() {
        assert!(range::parse("0.0.18446744073709551617").is_err());
    }
}
