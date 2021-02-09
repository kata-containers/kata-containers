// Test support for inspecting Values

use std::fmt;
use std::str;

use super::internal;
use super::{Error, Value};

#[derive(Debug, PartialEq)]
pub(in kv) enum Token {
    U64(u64),
    I64(i64),
    F64(f64),
    Char(char),
    Bool(bool),
    Str(String),
    None,

    #[cfg(feature = "kv_unstable_sval")]
    Sval,
}

#[cfg(test)]
impl<'v> Value<'v> {
    pub(in kv) fn to_token(&self) -> Token {
        struct TestVisitor(Option<Token>);

        impl<'v> internal::Visitor<'v> for TestVisitor {
            fn debug(&mut self, v: &dyn fmt::Debug) -> Result<(), Error> {
                self.0 = Some(Token::Str(format!("{:?}", v)));
                Ok(())
            }

            fn u64(&mut self, v: u64) -> Result<(), Error> {
                self.0 = Some(Token::U64(v));
                Ok(())
            }

            fn i64(&mut self, v: i64) -> Result<(), Error> {
                self.0 = Some(Token::I64(v));
                Ok(())
            }

            fn f64(&mut self, v: f64) -> Result<(), Error> {
                self.0 = Some(Token::F64(v));
                Ok(())
            }

            fn bool(&mut self, v: bool) -> Result<(), Error> {
                self.0 = Some(Token::Bool(v));
                Ok(())
            }

            fn char(&mut self, v: char) -> Result<(), Error> {
                self.0 = Some(Token::Char(v));
                Ok(())
            }

            fn str(&mut self, v: &str) -> Result<(), Error> {
                self.0 = Some(Token::Str(v.into()));
                Ok(())
            }

            fn none(&mut self) -> Result<(), Error> {
                self.0 = Some(Token::None);
                Ok(())
            }

            #[cfg(feature = "kv_unstable_sval")]
            fn sval(&mut self, _: &dyn internal::sval::Value) -> Result<(), Error> {
                self.0 = Some(Token::Sval);
                Ok(())
            }
        }

        let mut visitor = TestVisitor(None);
        self.visit(&mut visitor).unwrap();

        visitor.0.unwrap()
    }
}
