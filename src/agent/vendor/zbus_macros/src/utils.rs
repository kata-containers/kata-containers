use std::collections::HashMap;

use proc_macro2::TokenStream;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::{format_ident, quote};
use syn::{
    Attribute, FnArg, Ident, Lit, Meta, MetaList, NestedMeta, Pat, PatIdent, PatType, Result,
};

pub fn zbus_path() -> TokenStream {
    if let Ok(FoundCrate::Name(name)) = crate_name("zbus") {
        let ident = format_ident!("{}", name);
        quote! { ::#ident }
    } else {
        quote! { ::zbus }
    }
}

pub fn arg_ident(arg: &FnArg) -> Option<&Ident> {
    match arg {
        FnArg::Typed(PatType { pat, .. }) => {
            if let Pat::Ident(PatIdent { ident, .. }) = &**pat {
                return Some(ident);
            }
            None
        }
        _ => None,
    }
}

pub fn get_doc_attrs(attrs: &[Attribute]) -> Vec<&Attribute> {
    attrs.iter().filter(|x| x.path.is_ident("doc")).collect()
}

// Convert to pascal case, assuming snake case.
// If `s` is already in pascal case, should yield the same result.
pub fn pascal_case(s: &str) -> String {
    let mut pascal = String::new();
    let mut capitalize = true;
    for ch in s.chars() {
        if ch == '_' {
            capitalize = true;
        } else if capitalize {
            pascal.push(ch.to_ascii_uppercase());
            capitalize = false;
        } else {
            pascal.push(ch);
        }
    }
    pascal
}

// Convert to snake case, assuming pascal case.
// If `s` is already in snake case, should yield the same result.
pub fn snake_case(s: &str) -> String {
    let mut snake = String::new();
    for ch in s.chars() {
        if ch.is_ascii_uppercase() && !snake.is_empty() {
            snake.push('_');
        }
        snake.push(ch.to_ascii_lowercase());
    }
    snake
}

#[derive(Debug, PartialEq, Eq)]
pub enum ItemAttribute {
    Property(HashMap<String, String>),
    Signal,
    NoReply,
    OutArgs(Vec<String>),
    Name(String),
    ZbusError,
    Object(String),
    AsyncObject(String),
    BlockingObject(String),
}

impl ItemAttribute {
    pub fn is_property(&self) -> bool {
        matches!(self, Self::Property(_))
    }

    pub fn is_signal(&self) -> bool {
        self == &Self::Signal
    }

    pub fn is_out_args(&self) -> bool {
        matches!(self, Self::OutArgs(_))
    }
}

// find the #[@attr_name] attribute in @attrs
pub fn find_attribute_meta(attrs: &[Attribute], attr_name: &str) -> Result<Option<MetaList>> {
    let meta = match attrs.iter().find(|a| a.path.is_ident(attr_name)) {
        Some(a) => a.parse_meta(),
        _ => return Ok(None),
    };
    match meta? {
        Meta::List(n) => Ok(Some(n)),
        _ => panic!("wrong meta type, expected list"),
    }
}

fn parse_ident(meta: &NestedMeta) -> String {
    let meta = match meta {
        NestedMeta::Meta(m) => m,
        _ => panic!("wrong meta type, expected meta"),
    };

    let ident = match meta {
        Meta::Path(p) => p.get_ident().unwrap(),
        Meta::NameValue(n) => match n.path.get_ident() {
            None => panic!("missing ident"),
            Some(ident) => ident,
        },
        Meta::List(l) => match l.path.get_ident() {
            None => panic!("missing ident"),
            Some(ident) => ident,
        },
    };
    ident.to_string()
}

// parse a single meta like: ident = "value". meta can have multiple values too.
fn parse_single_attribute(meta: &NestedMeta) -> (String, Vec<String>) {
    let meta = match &meta {
        NestedMeta::Meta(m) => m,
        _ => panic!("wrong meta type, expected meta"),
    };

    let (ident, values) = match meta {
        Meta::Path(p) => (p.get_ident().unwrap(), vec!["".to_string()]),
        Meta::NameValue(n) => {
            let value = match &n.lit {
                Lit::Str(s) => s.value(),
                _ => panic!("wrong meta type, expected string"),
            };

            let ident = match n.path.get_ident() {
                None => panic!("missing ident"),
                Some(ident) => ident,
            };

            (ident, vec![value])
        }
        Meta::List(l) => {
            let mut values = vec![];
            for nested in l.nested.iter() {
                match nested {
                    NestedMeta::Lit(lit) => match lit {
                        Lit::Str(s) => values.push(s.value()),
                        _ => panic!("wrong meta type, expected string"),
                    },
                    x => panic!("wrong meta type, expected literal but got {:?}", x),
                }
            }

            let ident = match l.path.get_ident() {
                None => panic!("missing ident"),
                Some(ident) => ident,
            };

            (ident, values)
        }
    };

    (ident.to_string(), values)
}

fn proxy_parse_item_attribute(meta: &NestedMeta) -> Result<ItemAttribute> {
    let ident = parse_ident(meta);
    match ident.as_str() {
        "property" => {
            let mut attrs = HashMap::new();
            property_parse_item_attribute(meta, &mut attrs);
            Ok(ItemAttribute::Property(attrs))
        }
        _ => parse_simple_attribute(meta),
    }
}

fn parse_simple_attribute(meta: &NestedMeta) -> Result<ItemAttribute> {
    let (ident, mut values) = parse_single_attribute(meta);
    match ident.as_ref() {
        "name" => Ok(ItemAttribute::Name(values.remove(0))),
        "signal" => Ok(ItemAttribute::Signal),
        "no_reply" => Ok(ItemAttribute::NoReply),
        "out_args" => Ok(ItemAttribute::OutArgs(values)),
        "object" => Ok(ItemAttribute::Object(values.remove(0))),
        "async_object" => Ok(ItemAttribute::AsyncObject(values.remove(0))),
        "blocking_object" => Ok(ItemAttribute::BlockingObject(values.remove(0))),
        "property" => unreachable!(),
        s => panic!("Unknown item meta {}", s),
    }
}

fn property_parse_item_attribute(meta: &NestedMeta, attrs: &mut HashMap<String, String>) {
    let meta = match &meta {
        NestedMeta::Meta(m) => m,
        _ => panic!("wrong meta type, expected meta"),
    };

    match meta {
        Meta::Path(_) => {}
        Meta::NameValue(n) => {
            let key = n.path.get_ident().unwrap().to_string();
            let value = match &n.lit {
                Lit::Str(s) => s.value(),
                _ => panic!("wrong meta type, expected string"),
            };
            attrs.insert(key, value);
        }
        Meta::List(l) => {
            for nested in l.nested.iter() {
                property_parse_item_attribute(nested, attrs);
            }
        }
    }
}

// Parse optional item attributes such as:
// #[dbus_proxy(name = "MyName", property)]
pub fn parse_item_attributes(attrs: &[Attribute], attr_name: &str) -> Result<Vec<ItemAttribute>> {
    let meta = find_attribute_meta(attrs, attr_name)?;

    let v = match meta {
        Some(meta) => meta
            .nested
            .iter()
            .map(|m| proxy_parse_item_attribute(m).unwrap())
            .collect(),
        None => Vec::new(),
    };

    Ok(v)
}

fn error_parse_item_attribute(meta: &NestedMeta) -> Result<ItemAttribute> {
    let (ident, mut values) = parse_single_attribute(meta);

    match ident.as_ref() {
        "name" => Ok(ItemAttribute::Name(values.remove(0))),
        "zbus_error" => Ok(ItemAttribute::ZbusError),
        s => panic!("Unknown item meta {}", s),
    }
}

// Parse optional item attributes such as:
// #[dbus_error(name = "MyName")]
pub fn error_parse_item_attributes(attrs: &[Attribute]) -> Result<Vec<ItemAttribute>> {
    let meta = find_attribute_meta(attrs, "dbus_error")?;

    let v = match meta {
        Some(meta) => meta
            .nested
            .iter()
            .map(|m| error_parse_item_attribute(m).unwrap())
            .collect(),
        None => Vec::new(),
    };

    Ok(v)
}

pub fn is_blank(s: &str) -> bool {
    s.trim().is_empty()
}

#[cfg(test)]
mod tests {
    use super::{pascal_case, snake_case};

    #[test]
    fn test_snake_to_pascal_case() {
        assert_eq!("MeaningOfLife", &pascal_case("meaning_of_life"));
    }

    #[test]
    fn test_pascal_case_on_pascal_cased_str() {
        assert_eq!("MeaningOfLife", &pascal_case("MeaningOfLife"));
    }

    #[test]
    fn test_pascal_case_to_snake_case() {
        assert_eq!("meaning_of_life", &snake_case("MeaningOfLife"));
    }

    #[test]
    fn test_snake_case_on_snake_cased_str() {
        assert_eq!("meaning_of_life", &snake_case("meaning_of_life"));
    }
}
