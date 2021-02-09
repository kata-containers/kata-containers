extern crate syn;

mod features;

#[macro_use]
mod macros;

use syn::parse::Parser;
use syn::{Attribute, Meta};

#[test]
fn test_meta_item_word() {
    let (interpret, parse) = test("#[foo]");

    snapshot!(interpret, @r###"Word("foo")"###);

    snapshot!(parse, @r###"Word("foo")"###);
}

#[test]
fn test_meta_item_name_value() {
    let (interpret, parse) = test("#[foo = 5]");

    snapshot!(interpret, @r###"
   ⋮Meta::NameValue {
   ⋮    ident: "foo",
   ⋮    lit: 5,
   ⋮}
    "###);

    snapshot!(parse, @r###"
   ⋮Meta::NameValue {
   ⋮    ident: "foo",
   ⋮    lit: 5,
   ⋮}
    "###);
}

#[test]
fn test_meta_item_bool_value() {
    let (interpret, parse) = test("#[foo = true]");;

    snapshot!(interpret, @r###"
   ⋮Meta::NameValue {
   ⋮    ident: "foo",
   ⋮    lit: Lit::Bool {
   ⋮        value: true,
   ⋮    },
   ⋮}
    "###);

    snapshot!(parse, @r###"
   ⋮Meta::NameValue {
   ⋮    ident: "foo",
   ⋮    lit: Lit::Bool {
   ⋮        value: true,
   ⋮    },
   ⋮}
    "###);

    let (interpret, parse) = test("#[foo = false]");

    snapshot!(interpret, @r###"
   ⋮Meta::NameValue {
   ⋮    ident: "foo",
   ⋮    lit: Lit::Bool {
   ⋮        value: false,
   ⋮    },
   ⋮}
    "###);

    snapshot!(parse, @r###"
   ⋮Meta::NameValue {
   ⋮    ident: "foo",
   ⋮    lit: Lit::Bool {
   ⋮        value: false,
   ⋮    },
   ⋮}
    "###);
}

#[test]
fn test_meta_item_list_lit() {
    let (interpret, parse) = test("#[foo(5)]");

    snapshot!(interpret, @r###"
   ⋮Meta::List {
   ⋮    ident: "foo",
   ⋮    nested: [
   ⋮        Literal(5),
   ⋮    ],
   ⋮}
    "###);

    snapshot!(parse, @r###"
   ⋮Meta::List {
   ⋮    ident: "foo",
   ⋮    nested: [
   ⋮        Literal(5),
   ⋮    ],
   ⋮}
    "###);
}

#[test]
fn test_meta_item_list_word() {
    let (interpret, parse) = test("#[foo(bar)]");

    snapshot!(interpret, @r###"
   ⋮Meta::List {
   ⋮    ident: "foo",
   ⋮    nested: [
   ⋮        Meta(Word("bar")),
   ⋮    ],
   ⋮}
    "###);

    snapshot!(parse, @r###"
   ⋮Meta::List {
   ⋮    ident: "foo",
   ⋮    nested: [
   ⋮        Meta(Word("bar")),
   ⋮    ],
   ⋮}
    "###);
}

#[test]
fn test_meta_item_list_name_value() {
    let (interpret, parse) = test("#[foo(bar = 5)]");

    snapshot!(interpret, @r###"
   ⋮Meta::List {
   ⋮    ident: "foo",
   ⋮    nested: [
   ⋮        Meta(Meta::NameValue {
   ⋮            ident: "bar",
   ⋮            lit: 5,
   ⋮        }),
   ⋮    ],
   ⋮}
    "###);

    snapshot!(parse, @r###"
   ⋮Meta::List {
   ⋮    ident: "foo",
   ⋮    nested: [
   ⋮        Meta(Meta::NameValue {
   ⋮            ident: "bar",
   ⋮            lit: 5,
   ⋮        }),
   ⋮    ],
   ⋮}
    "###);
}

#[test]
fn test_meta_item_list_bool_value() {
    let (interpret, parse) = test("#[foo(bar = true)]");

    snapshot!(interpret, @r###"
   ⋮Meta::List {
   ⋮    ident: "foo",
   ⋮    nested: [
   ⋮        Meta(Meta::NameValue {
   ⋮            ident: "bar",
   ⋮            lit: Lit::Bool {
   ⋮                value: true,
   ⋮            },
   ⋮        }),
   ⋮    ],
   ⋮}
    "###);

    snapshot!(parse, @r###"
   ⋮Meta::List {
   ⋮    ident: "foo",
   ⋮    nested: [
   ⋮        Meta(Meta::NameValue {
   ⋮            ident: "bar",
   ⋮            lit: Lit::Bool {
   ⋮                value: true,
   ⋮            },
   ⋮        }),
   ⋮    ],
   ⋮}
    "###);
}

#[test]
fn test_meta_item_multiple() {
    let (interpret, parse) = test("#[foo(word, name = 5, list(name2 = 6), word2)]");

    snapshot!(interpret, @r###"
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

    snapshot!(parse, @r###"
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
}

#[test]
fn test_bool_lit() {
    let (interpret, parse) = test("#[foo(true)]");

    snapshot!(interpret, @r###"
   ⋮Meta::List {
   ⋮    ident: "foo",
   ⋮    nested: [
   ⋮        Literal(Lit::Bool {
   ⋮            value: true,
   ⋮        }),
   ⋮    ],
   ⋮}
    "###);

    snapshot!(parse, @r###"
   ⋮Meta::List {
   ⋮    ident: "foo",
   ⋮    nested: [
   ⋮        Literal(Lit::Bool {
   ⋮            value: true,
   ⋮        }),
   ⋮    ],
   ⋮}
    "###);
}

fn test(input: &str) -> (Meta, Meta) {
    let attrs = Attribute::parse_outer.parse_str(input).unwrap();

    assert_eq!(attrs.len(), 1);
    let attr = attrs.into_iter().next().unwrap();

    let interpret = attr.interpret_meta().unwrap();
    let parse = attr.parse_meta().unwrap();
    assert_eq!(interpret, parse);

    (interpret, parse)
}
