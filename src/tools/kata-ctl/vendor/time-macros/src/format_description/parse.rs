use proc_macro::Span;

use crate::format_description::component::{Component, NakedComponent};
use crate::format_description::error::InvalidFormatDescription;
use crate::format_description::{helper, modifier, FormatItem};
use crate::Error;

struct ParsedItem<'a> {
    item: FormatItem<'a>,
    remaining: &'a [u8],
}

fn parse_component(mut s: &[u8], index: &mut usize) -> Result<Component, InvalidFormatDescription> {
    s = helper::consume_whitespace(s, index);

    let component_index = *index;
    let whitespace_loc = s
        .iter()
        .position(u8::is_ascii_whitespace)
        .unwrap_or(s.len());
    *index += whitespace_loc;
    let component_name = &s[..whitespace_loc];
    s = &s[whitespace_loc..];
    s = helper::consume_whitespace(s, index);

    Ok(NakedComponent::parse(component_name, component_index)?
        .attach_modifiers(modifier::Modifiers::parse(component_name, s, index)?))
}

fn parse_literal<'a>(s: &'a [u8], index: &mut usize) -> ParsedItem<'a> {
    let loc = s.iter().position(|&c| c == b'[').unwrap_or(s.len());
    *index += loc;
    ParsedItem {
        item: FormatItem::Literal(&s[..loc]),
        remaining: &s[loc..],
    }
}

fn parse_item<'a>(
    s: &'a [u8],
    index: &mut usize,
) -> Result<ParsedItem<'a>, InvalidFormatDescription> {
    if let [b'[', b'[', remaining @ ..] = s {
        *index += 2;
        return Ok(ParsedItem {
            item: FormatItem::Literal(b"["),
            remaining,
        });
    };

    if s.starts_with(b"[") {
        if let Some(bracket_index) = s.iter().position(|&c| c == b']') {
            *index += 1; // opening bracket
            let ret_val = ParsedItem {
                item: FormatItem::Component(parse_component(&s[1..bracket_index], index)?),
                remaining: &s[bracket_index + 1..],
            };
            *index += 1; // closing bracket
            Ok(ret_val)
        } else {
            Err(InvalidFormatDescription::UnclosedOpeningBracket { index: *index })
        }
    } else {
        Ok(parse_literal(s, index))
    }
}

pub(crate) fn parse(mut s: &[u8], span: Span) -> Result<Vec<FormatItem<'_>>, Error> {
    let mut compound = Vec::new();
    let mut loc = 0;

    while !s.is_empty() {
        let ParsedItem { item, remaining } =
            parse_item(s, &mut loc).map_err(|error| Error::InvalidFormatDescription {
                error,
                span_start: Some(span),
                span_end: Some(span),
            })?;
        s = remaining;
        compound.push(item);
    }

    Ok(compound)
}
