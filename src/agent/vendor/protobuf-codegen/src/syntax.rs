#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Syntax {
    PROTO2,
    PROTO3,
}

impl Syntax {
    pub fn parse(s: &str) -> Self {
        match s {
            "" | "proto2" => Syntax::PROTO2,
            "proto3" => Syntax::PROTO3,
            _ => panic!("unsupported syntax value: {:?}", s),
        }
    }
}
