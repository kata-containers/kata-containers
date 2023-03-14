#[macro_use]
extern crate derive_builder;

#[derive(Debug, PartialEq, Default, Builder, Clone)]
pub struct Lorem {
    #[builder(default = "88", field(type = "usize", build = "self.ipsum + 42"))]
    ipsum: usize,
}

fn main() {}
