extern crate syn;

mod features;

#[macro_use]
mod macros;

use syn::{Meta, MetaList, MetaNameValue, NestedMeta};

#[test]
fn test_parse_meta_item_word() {
    let input = "hello";

    snapshot!(input as Meta, @r###"Word("hello")"###);
}

#[test]
fn test_parse_meta_name_value() {
    let input = "foo = 5";
    let (inner, meta) = (input, input);

    snapshot!(inner as MetaNameValue, @r###"
   ⋮MetaNameValue {
   ⋮    ident: "foo",
   ⋮    lit: 5,
   ⋮}
    "###);

    snapshot!(meta as Meta, @r###"
   ⋮Meta::NameValue {
   ⋮    ident: "foo",
   ⋮    lit: 5,
   ⋮}
    "###);

    assert_eq!(meta, inner.into());
}

#[test]
fn test_parse_meta_name_value_with_keyword() {
    let input = "static = 5";
    let (inner, meta) = (input, input);

    snapshot!(inner as MetaNameValue, @r###"
   ⋮MetaNameValue {
   ⋮    ident: "static",
   ⋮    lit: 5,
   ⋮}
    "###);

    snapshot!(meta as Meta, @r###"
   ⋮Meta::NameValue {
   ⋮    ident: "static",
   ⋮    lit: 5,
   ⋮}
    "###);

    assert_eq!(meta, inner.into());
}

#[test]
fn test_parse_meta_name_value_with_bool() {
    let input = "true = 5";
    let (inner, meta) = (input, input);

    snapshot!(inner as MetaNameValue, @r###"
   ⋮MetaNameValue {
   ⋮    ident: "true",
   ⋮    lit: 5,
   ⋮}
    "###);

    snapshot!(meta as Meta, @r###"
   ⋮Meta::NameValue {
   ⋮    ident: "true",
   ⋮    lit: 5,
   ⋮}
    "###);

    assert_eq!(meta, inner.into());
}

#[test]
fn test_parse_meta_item_list_lit() {
    let input = "foo(5)";
    let (inner, meta) = (input, input);

    snapshot!(inner as MetaList, @r###"
   ⋮MetaList {
   ⋮    ident: "foo",
   ⋮    nested: [
   ⋮        Literal(5),
   ⋮    ],
   ⋮}
    "###);

    snapshot!(meta as Meta, @r###"
   ⋮Meta::List {
   ⋮    ident: "foo",
   ⋮    nested: [
   ⋮        Literal(5),
   ⋮    ],
   ⋮}
    "###);

    assert_eq!(meta, inner.into());
}

#[test]
fn test_parse_meta_item_multiple() {
    let input = "foo(word, name = 5, list(name2 = 6), word2)";
    let (inner, meta) = (input, input);

    snapshot!(inner as MetaList, @r###"
   ⋮MetaList {
   ⋮    ident: "foo",
   ⋮    nested: [
   ⋮        Meta(Word("word")),
   ⋮        Meta(Meta::NameValue {
   ⋮            ident: "name",
   ⋮            lit: 5,
   ⋮        }),
   ⋮        Meta(Meta::List {
   ⋮            ident: "list",
   ⋮            nested: [
   ⋮                Meta(Meta::NameValue {
   ⋮                    ident: "name2",
   ⋮                    lit: 6,
   ⋮                }),
   ⋮            ],
   ⋮        }),
   ⋮        Meta(Word("word2")),
   ⋮    ],
   ⋮}
    "###);

    snapshot!(meta as Meta, @r###"
   ⋮Meta::List {
   ⋮    ident: "foo",
   ⋮    nested: [
   ⋮        Meta(Word("word")),
   ⋮        Meta(Meta::NameValue {
   ⋮            ident: "name",
   ⋮            lit: 5,
   ⋮        }),
   ⋮        Meta(Meta::List {
   ⋮            ident: "list",
   ⋮            nested: [
   ⋮                Meta(Meta::NameValue {
   ⋮                    ident: "name2",
   ⋮                    lit: 6,
   ⋮                }),
   ⋮            ],
   ⋮        }),
   ⋮        Meta(Word("word2")),
   ⋮    ],
   ⋮}
    "###);

    assert_eq!(meta, inner.into());
}

#[test]
fn test_parse_nested_meta() {
    let input = "5";
    snapshot!(input as NestedMeta, @"Literal(5)");

    let input = "list(name2 = 6)";
    snapshot!(input as NestedMeta, @r###"
   ⋮Meta(Meta::List {
   ⋮    ident: "list",
   ⋮    nested: [
   ⋮        Meta(Meta::NameValue {
   ⋮            ident: "name2",
   ⋮            lit: 6,
   ⋮        }),
   ⋮    ],
   ⋮})
    "###);
}
