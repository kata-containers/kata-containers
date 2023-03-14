use ascii_canvas::AsciiCanvas;
use crate::grammar::parse_tree::Span;
use crate::message::builder::MessageBuilder;
use crate::test_util::expect_debug;
use crate::tls::Tls;

use super::*;

fn install_tls() -> Tls {
    Tls::test_string(
        r#"foo
bar
baz
"#,
    )
}

#[test]
fn hello_world() {
    let _tls = install_tls();
    let msg = MessageBuilder::new(Span(0, 2))
        .heading()
        .text("Hello, world!")
        .end()
        .body()
        .begin_wrap()
        .text(
            "This is a very, very, very, very long sentence. \
             OK, not THAT long!",
        )
        .end()
        .indented_by(4)
        .end()
        .end();
    let min_width = msg.min_width();
    let mut canvas = AsciiCanvas::new(0, min_width);
    msg.emit(&mut canvas);
    expect_debug(
        &canvas.to_strings(),
        r#"
[
    "tmp.txt:1:1: 1:2: Hello, world!",
    "",
    "      This is a very, very,",
    "      very, very long sentence.",
    "      OK, not THAT long!"
]
"#
        .trim(),
    );
}

/// Test a case where the body in the message is longer than the
/// header (which used to mess up the `min_width` computation).
#[test]
fn long_body() {
    let _tls = install_tls();
    let msg = MessageBuilder::new(Span(0, 2))
        .heading()
        .text("Hello, world!")
        .end()
        .body()
        .text(
            "This is a very, very, very, very long sentence. \
             OK, not THAT long!",
        )
        .end()
        .end();
    let min_width = msg.min_width();
    let mut canvas = AsciiCanvas::new(0, min_width);
    msg.emit(&mut canvas);
    expect_debug(
        &canvas.to_strings(),
        r#"
[
    "tmp.txt:1:1: 1:2: Hello, world!",
    "",
    "  This is a very, very, very, very long sentence. OK, not THAT long!"
]
"#
        .trim(),
    );
}

#[test]
fn paragraphs() {
    let _tls = install_tls();
    let msg = MessageBuilder::new(Span(0, 2))
        .heading()
        .text("Hello, world!")
        .end() // heading
        .body()
        .begin_paragraphs()
        .begin_wrap()
        .text(
            "This is the first paragraph. It contains a lot of really interesting \
             information that the reader will no doubt peruse with care.",
        )
        .end()
        .begin_wrap()
        .text(
            "This is the second paragraph. It contains even more really interesting \
             information that the reader will no doubt skip over with wild abandon.",
        )
        .end()
        .begin_wrap()
        .text(
            "This is the final paragraph. The reader won't even spare this one \
             a second glance, despite it containing just waht they need to know \
             to solve their problem and to derive greater pleasure from life. \
             The secret: All you need is love! Dum da da dum.",
        )
        .end()
        .end()
        .end()
        .end();
    let min_width = msg.min_width();
    let mut canvas = AsciiCanvas::new(0, min_width);
    msg.emit(&mut canvas);
    expect_debug(
        &canvas.to_strings(),
        r#"
[
    "tmp.txt:1:1: 1:2: Hello, world!",
    "",
    "  This is the first paragraph.",
    "  It contains a lot of really",
    "  interesting information that",
    "  the reader will no doubt",
    "  peruse with care.",
    "",
    "  This is the second paragraph.",
    "  It contains even more really",
    "  interesting information that",
    "  the reader will no doubt skip",
    "  over with wild abandon.",
    "",
    "  This is the final paragraph.",
    "  The reader won't even spare",
    "  this one a second glance,",
    "  despite it containing just",
    "  waht they need to know to",
    "  solve their problem and to",
    "  derive greater pleasure from",
    "  life. The secret: All you",
    "  need is love! Dum da da dum."
]
"#
        .trim(),
    );
}
