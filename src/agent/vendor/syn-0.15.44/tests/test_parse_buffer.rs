#[macro_use]
extern crate syn;

use syn::parse::{discouraged::Speculative, Parse, ParseStream, Parser, Result};

#[test]
#[should_panic(expected = "Fork was not derived from the advancing parse stream")]
fn smuggled_speculative_cursor_between_sources() {
    struct BreakRules;
    impl Parse for BreakRules {
        fn parse(input1: ParseStream) -> Result<Self> {
            let nested = |input2: ParseStream| {
                input1.advance_to(&input2);
                Ok(Self)
            };
            nested.parse_str("")
        }
    }

    syn::parse_str::<BreakRules>("").unwrap();
}

#[test]
#[should_panic(expected = "Fork was not derived from the advancing parse stream")]
fn smuggled_speculative_cursor_between_brackets() {
    struct BreakRules;
    impl Parse for BreakRules {
        fn parse(input: ParseStream) -> Result<Self> {
            let a;
            let b;
            parenthesized!(a in input);
            parenthesized!(b in input);
            a.advance_to(&b);
            Ok(Self)
        }
    }

    syn::parse_str::<BreakRules>("()()").unwrap();
}

#[test]
#[should_panic(expected = "Fork was not derived from the advancing parse stream")]
fn smuggled_speculative_cursor_into_brackets() {
    struct BreakRules;
    impl Parse for BreakRules {
        fn parse(input: ParseStream) -> Result<Self> {
            let a;
            parenthesized!(a in input);
            input.advance_to(&a);
            Ok(Self)
        }
    }

    syn::parse_str::<BreakRules>("()").unwrap();
}
