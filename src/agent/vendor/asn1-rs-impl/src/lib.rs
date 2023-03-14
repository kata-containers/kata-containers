extern crate proc_macro;
use proc_macro::{Span, TokenStream};
use syn::{parse_macro_input, Error, LitInt};

#[proc_macro]
pub fn encode_oid(input: TokenStream) -> TokenStream {
    let token_stream = input.to_string();
    let (s, relative) = if token_stream.starts_with("rel ") {
        (&token_stream[4..], true)
    } else {
        (token_stream.as_ref(), false)
    };
    let items: Result<Vec<_>, _> = s.split('.').map(|x| x.trim().parse::<u128>()).collect();
    let mut items: &[_] = match items.as_ref() {
        Ok(v) => v.as_ref(),
        Err(_) => return create_error("Could not parse OID"),
    };
    let mut v = Vec::new();
    if !relative {
        if items.len() < 2 {
            if items.len() == 1 && items[0] == 0 {
                return "[0]".parse().unwrap();
            }
            return create_error("Need at least two components for non-relative oid");
        }
        if items[0] > 2 || items[1] > 39 {
            return create_error("First components are too big");
        }
        let first_byte = (items[0] * 40 + items[1]) as u8;
        v.push(first_byte);
        items = &items[2..];
    }
    for &int in items {
        let enc = encode_base128(int);
        v.extend_from_slice(&enc);
    }
    // "fn answer() -> u32 { 42 }".parse().unwrap()
    let mut s = String::with_capacity(2 + 6 * v.len());
    s.push('[');
    for byte in v.iter() {
        s.insert_str(s.len(), &format!("0x{:02x}, ", byte));
    }
    s.push(']');
    s.parse().unwrap()
}

#[inline]
fn create_error(msg: &str) -> TokenStream {
    let s = format!("Invalid OID({})", msg);
    Error::new(Span::call_site().into(), &s)
        .to_compile_error()
        .into()
}

// encode int as base128
fn encode_base128(int: u128) -> Vec<u8> {
    let mut val = int;
    let mut base128 = Vec::new();
    let lo = val & 0x7f;
    base128.push(lo as u8);
    val >>= 7;
    loop {
        if val == 0 {
            base128.reverse();
            return base128;
        }
        let lo = val & 0x7f;
        base128.push(lo as u8 | 0x80);
        val >>= 7;
    }
}

#[proc_macro]
pub fn encode_int(input: TokenStream) -> TokenStream {
    let lit = parse_macro_input!(input as LitInt);

    match impl_encode_int(lit) {
        Ok(ts) => ts,
        Err(e) => e.to_compile_error().into(),
    }

    // let token_stream = input.to_string();
    // let items: Result<Vec<_>, _> = token_stream
    //     .split('.')
    //     .map(|x| x.trim().parse::<u64>())
    //     .collect();
    // let err = Error::new(Span::call_site().into(), "invalid OID");
    // if let Ok(items) = items {
    //     let mut v = Vec::new();
    //     if items.len() < 2 || items[0] > 2 || items[1] > 39 {
    //         return err.to_compile_error().into();
    //     }
    //     let first_byte = (items[0] * 40 + items[1]) as u8;
    //     v.push(first_byte);
    //     for &int in &items[2..] {
    //         let enc = encode_base128(int);
    //         v.extend_from_slice(&enc);
    //     }
    //     // "fn answer() -> u32 { 42 }".parse().unwrap()
    //     let mut s = String::with_capacity(2 + 6 * v.len());
    //     s.push('[');
    //     for byte in v.iter() {
    //         s.insert_str(s.len(), &format!("0x{:02x}, ", byte));
    //     }
    //     s.push(']');
    //     s.parse().unwrap()
    // } else {
    //     eprintln!("could not parse OID '{}'", token_stream);
    //     err.to_compile_error().into()
    // }
}

fn impl_encode_int(lit: LitInt) -> Result<TokenStream, Error> {
    let value = lit.base10_parse::<u64>()?;

    let bytes = value.to_be_bytes();
    let v: Vec<_> = bytes.iter().skip_while(|&c| *c == 0).collect();

    let mut s = String::with_capacity(2 + 6 * v.len());
    s.push('[');
    for byte in v.iter() {
        s.insert_str(s.len(), &format!("0x{:02x}, ", byte));
    }
    s.push(']');
    Ok(s.parse().unwrap())
}
