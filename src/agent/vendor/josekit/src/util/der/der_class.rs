use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DerClass {
    Universal,
    Application,
    ContextSpecific,
    Private,
}

impl DerClass {
    pub fn class_no(&self) -> u8 {
        match self {
            DerClass::Universal => 0,
            DerClass::Application => 1,
            DerClass::ContextSpecific => 2,
            DerClass::Private => 3,
        }
    }
}

impl fmt::Display for DerClass {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
