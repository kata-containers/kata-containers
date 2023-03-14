/// A macro which defines an enum type.
macro_rules! enum_builder {
    (
    $(#[$comment:meta])*
    @U8
        EnumName: $enum_name: ident;
        EnumVal { $( $enum_var: ident => $enum_val: expr ),* }
    ) => {
        $(#[$comment])*
        #[derive(Debug, PartialEq, Eq, Clone, Copy)]
        pub enum $enum_name {
            $( $enum_var),*
            ,Unknown(u8)
        }
        impl $enum_name {
            pub fn get_u8(&self) -> u8 {
                let x = self.clone();
                match x {
                    $( $enum_name::$enum_var => $enum_val),*
                    ,$enum_name::Unknown(x) => x
                }
            }
        }
        impl Codec for $enum_name {
            fn encode(&self, bytes: &mut Vec<u8>) {
                self.get_u8().encode(bytes);
            }

            fn read(r: &mut Reader) -> Option<Self> {
                u8::read(r).map($enum_name::from)
            }
        }
        impl From<u8> for $enum_name {
            fn from(x: u8) -> Self {
                match x {
                    $($enum_val => $enum_name::$enum_var),*
                    , x => $enum_name::Unknown(x),
                }
            }
        }
    };
    (
    $(#[$comment:meta])*
    @U16
        EnumName: $enum_name: ident;
        EnumVal { $( $enum_var: ident => $enum_val: expr ),* }
    ) => {
        $(#[$comment])*
        #[derive(Debug, PartialEq, Eq, Clone, Copy)]
        pub enum $enum_name {
            $( $enum_var),*
            ,Unknown(u16)
        }
        impl $enum_name {
            pub fn get_u16(&self) -> u16 {
                let x = self.clone();
                match x {
                    $( $enum_name::$enum_var => $enum_val),*
                    ,$enum_name::Unknown(x) => x
                }
            }

            pub fn as_str(&self) -> Option<&'static str> {
                match self {
                    $( $enum_name::$enum_var => Some(stringify!($enum_var))),*
                    ,$enum_name::Unknown(_) => None,
                }
            }
        }
        impl Codec for $enum_name {
            fn encode(&self, bytes: &mut Vec<u8>) {
                self.get_u16().encode(bytes);
            }

            fn read(r: &mut Reader) -> Option<Self> {
                u16::read(r).map($enum_name::from)
            }
        }
        impl From<u16> for $enum_name {
            fn from(x: u16) -> Self {
                match x {
                    $($enum_val => $enum_name::$enum_var),*
                    , x => $enum_name::Unknown(x),
                }
            }
        }
    };
}
