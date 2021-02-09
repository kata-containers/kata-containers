use ext::TokenStreamExt;
pub use proc_macro2::*;

fn is_ident_start(c: u8) -> bool {
    (b'a' <= c && c <= b'z') || (b'A' <= c && c <= b'Z') || c == b'_'
}

fn is_ident_continue(c: u8) -> bool {
    (b'a' <= c && c <= b'z') || (b'A' <= c && c <= b'Z') || c == b'_' || (b'0' <= c && c <= b'9')
}

fn is_ident(token: &str) -> bool {
    let mut iter = token.bytes();
    let first_ok = iter.next().map(is_ident_start).unwrap_or(false);
    
    first_ok && iter.all(is_ident_continue)
}

pub fn parse(tokens: &mut TokenStream, span: Span, s: &str) {
    if is_ident(s) {
        // Fast path, since idents are the most common token.
        tokens.append(Ident::new(s, span));
    } else {
        let s: TokenStream = s.parse().expect("invalid token stream");
        tokens.extend(s.into_iter().map(|mut t| {
            t.set_span(span);
            t
        }));
    }
}

macro_rules! push_punct {
    ($name:ident $char1:tt) => {
        pub fn $name(tokens: &mut TokenStream, span: Span) {
            let mut punct = Punct::new($char1, Spacing::Alone);
            punct.set_span(span);
            tokens.append(punct);
        }
    };
    ($name:ident $char1:tt $char2:tt) => {
        pub fn $name(tokens: &mut TokenStream, span: Span) {
            let mut punct = Punct::new($char1, Spacing::Joint);
            punct.set_span(span);
            tokens.append(punct);
            let mut punct = Punct::new($char2, Spacing::Alone);
            punct.set_span(span);
            tokens.append(punct);
        }
    };
    ($name:ident $char1:tt $char2:tt $char3:tt) => {
        pub fn $name(tokens: &mut TokenStream, span: Span) {
            let mut punct = Punct::new($char1, Spacing::Joint);
            punct.set_span(span);
            tokens.append(punct);
            let mut punct = Punct::new($char2, Spacing::Joint);
            punct.set_span(span);
            tokens.append(punct);
            let mut punct = Punct::new($char3, Spacing::Alone);
            punct.set_span(span);
            tokens.append(punct);
        }
    };
}

push_punct!(push_add '+');
push_punct!(push_add_eq '+' '=');
push_punct!(push_and '&');
push_punct!(push_and_and '&' '&');
push_punct!(push_and_eq '&' '=');
push_punct!(push_at '@');
push_punct!(push_bang '!');
push_punct!(push_caret '^');
push_punct!(push_caret_eq '^' '=');
push_punct!(push_colon ':');
push_punct!(push_colon2 ':' ':');
push_punct!(push_comma ',');
push_punct!(push_div '/');
push_punct!(push_div_eq '/' '=');
push_punct!(push_dot '.');
push_punct!(push_dot2 '.' '.');
push_punct!(push_dot3 '.' '.' '.');
push_punct!(push_dot_dot_eq '.' '.' '=');
push_punct!(push_eq '=');
push_punct!(push_eq_eq '=' '=');
push_punct!(push_ge '>' '=');
push_punct!(push_gt '>');
push_punct!(push_le '<' '=');
push_punct!(push_lt '<');
push_punct!(push_mul_eq '*' '=');
push_punct!(push_ne '!' '=');
push_punct!(push_or '|');
push_punct!(push_or_eq '|' '=');
push_punct!(push_or_or '|' '|');
push_punct!(push_pound '#');
push_punct!(push_question '?');
push_punct!(push_rarrow '-' '>');
push_punct!(push_larrow '<' '-');
push_punct!(push_rem '%');
push_punct!(push_rem_eq '%' '=');
push_punct!(push_fat_arrow '=' '>');
push_punct!(push_semi ';');
push_punct!(push_shl '<' '<');
push_punct!(push_shl_eq '<' '<' '=');
push_punct!(push_shr '>' '>');
push_punct!(push_shr_eq '>' '>' '=');
push_punct!(push_star '*');
push_punct!(push_sub '-');
push_punct!(push_sub_eq '-' '=');
