// SPDX-License-Identifier: MIT

use super::{buffer::RuleMessageBuffer, header::RuleHeader, nlas::Nla};
use crate::{
    utils::{Emitable, Parseable},
    DecodeError,
};
use anyhow::Context;

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct RuleMessage {
    pub header: RuleHeader,
    pub nlas: Vec<Nla>,
}

impl Emitable for RuleMessage {
    fn buffer_len(&self) -> usize {
        self.header.buffer_len() + self.nlas.as_slice().buffer_len()
    }

    fn emit(&self, buffer: &mut [u8]) {
        self.header.emit(buffer);
        self.nlas
            .as_slice()
            .emit(&mut buffer[self.header.buffer_len()..]);
    }
}

impl<'a, T: AsRef<[u8]> + 'a> Parseable<RuleMessageBuffer<&'a T>> for RuleMessage {
    fn parse(buf: &RuleMessageBuffer<&'a T>) -> Result<Self, DecodeError> {
        let header = RuleHeader::parse(buf).context("failed to parse link message header")?;
        let nlas = Vec::<Nla>::parse(buf).context("failed to parse link message NLAs")?;
        Ok(RuleMessage { header, nlas })
    }
}

impl<'a, T: AsRef<[u8]> + 'a> Parseable<RuleMessageBuffer<&'a T>> for Vec<Nla> {
    fn parse(buf: &RuleMessageBuffer<&'a T>) -> Result<Self, DecodeError> {
        let mut nlas = vec![];
        for nla_buf in buf.nlas() {
            nlas.push(Nla::parse(&nla_buf?)?);
        }
        Ok(nlas)
    }
}
