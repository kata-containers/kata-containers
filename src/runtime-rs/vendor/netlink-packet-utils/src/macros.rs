// SPDX-License-Identifier: MIT

#[macro_export(local_inner_macros)]
macro_rules! getter {
    ($buffer: ident, $name:ident, slice, $offset:expr) => {
        impl<'a, T: AsRef<[u8]> + ?Sized> $buffer<&'a T> {
            pub fn $name(&self) -> &'a [u8] {
                &self.buffer.as_ref()[$offset]
            }
        }
    };
    ($buffer: ident, $name:ident, $ty:tt, $offset:expr) => {
        impl<'a, T: AsRef<[u8]>> $buffer<T> {
            getter!($name, $ty, $offset);
        }
    };
    ($name:ident, u8, $offset:expr) => {
        pub fn $name(&self) -> u8 {
            self.buffer.as_ref()[$offset]
        }
    };
    ($name:ident, u16, $offset:expr) => {
        pub fn $name(&self) -> u16 {
            use $crate::byteorder::{ByteOrder, NativeEndian};
            NativeEndian::read_u16(&self.buffer.as_ref()[$offset])
        }
    };
    ($name:ident, u32, $offset:expr) => {
        pub fn $name(&self) -> u32 {
            use $crate::byteorder::{ByteOrder, NativeEndian};
            NativeEndian::read_u32(&self.buffer.as_ref()[$offset])
        }
    };
    ($name:ident, u64, $offset:expr) => {
        pub fn $name(&self) -> u64 {
            use $crate::byteorder::{ByteOrder, NativeEndian};
            NativeEndian::read_u64(&self.buffer.as_ref()[$offset])
        }
    };
    ($name:ident, i8, $offset:expr) => {
        pub fn $name(&self) -> i8 {
            self.buffer.as_ref()[$offset]
        }
    };
    ($name:ident, i16, $offset:expr) => {
        pub fn $name(&self) -> i16 {
            use $crate::byteorder::{ByteOrder, NativeEndian};
            NativeEndian::read_i16(&self.buffer.as_ref()[$offset])
        }
    };
    ($name:ident, i32, $offset:expr) => {
        pub fn $name(&self) -> i32 {
            use $crate::byteorder::{ByteOrder, NativeEndian};
            NativeEndian::read_i32(&self.buffer.as_ref()[$offset])
        }
    };
    ($name:ident, i64, $offset:expr) => {
        pub fn $name(&self) -> i64 {
            use $crate::byteorder::{ByteOrder, NativeEndian};
            NativeEndian::read_i64(&self.buffer.as_ref()[$offset])
        }
    };
}

#[macro_export(local_inner_macros)]
macro_rules! setter {
    ($buffer: ident, $name:ident, slice, $offset:expr) => {
        impl<'a, T: AsRef<[u8]> + AsMut<[u8]> + ?Sized> $buffer<&'a mut T> {
            $crate::paste::item! {
                pub fn [<$name _mut>](&mut self) -> &mut [u8] {
                    &mut self.buffer.as_mut()[$offset]
                }
            }
        }
    };
    ($buffer: ident, $name:ident, $ty:tt, $offset:expr) => {
        impl<'a, T: AsRef<[u8]> + AsMut<[u8]>> $buffer<T> {
            setter!($name, $ty, $offset);
        }
    };
    ($name:ident, u8, $offset:expr) => {
        $crate::paste::item! {
            pub fn [<set_ $name>](&mut self, value: u8) {
                self.buffer.as_mut()[$offset] = value;
            }
        }
    };
    ($name:ident, u16, $offset:expr) => {
        $crate::paste::item! {
            pub fn [<set_ $name>](&mut self, value: u16) {
                use $crate::byteorder::{ByteOrder, NativeEndian};
                NativeEndian::write_u16(&mut self.buffer.as_mut()[$offset], value)
            }
        }
    };
    ($name:ident, u32, $offset:expr) => {
        $crate::paste::item! {
            pub fn [<set_ $name>](&mut self, value: u32) {
                use $crate::byteorder::{ByteOrder, NativeEndian};
                NativeEndian::write_u32(&mut self.buffer.as_mut()[$offset], value)
            }
        }
    };
    ($name:ident, u64, $offset:expr) => {
        $crate::paste::item! {
            pub fn [<set_ $name>](&mut self, value: u64) {
                use $crate::byteorder::{ByteOrder, NativeEndian};
                NativeEndian::write_u64(&mut self.buffer.as_mut()[$offset], value)
            }
        }
    };
    ($name:ident, i8, $offset:expr) => {
        $crate::paste::item! {
            pub fn [<set_ $name>](&mut self, value: i8) {
                self.buffer.as_mut()[$offset] = value;
            }
        }
    };
    ($name:ident, i16, $offset:expr) => {
        $crate::paste::item! {
            pub fn [<set_ $name>](&mut self, value: i16) {
                use $crate::byteorder::{ByteOrder, NativeEndian};
                NativeEndian::write_i16(&mut self.buffer.as_mut()[$offset], value)
            }
        }
    };
    ($name:ident, i32, $offset:expr) => {
        $crate::paste::item! {
            pub fn [<set_ $name>](&mut self, value: i32) {
                use $crate::byteorder::{ByteOrder, NativeEndian};
                NativeEndian::write_i32(&mut self.buffer.as_mut()[$offset], value)
            }
        }
    };
    ($name:ident, i64, $offset:expr) => {
        $crate::paste::item! {
            pub fn [<set_ $name>](&mut self, value: i64) {
                use $crate::byteorder::{ByteOrder, NativeEndian};
                NativeEndian::write_i64(&mut self.buffer.as_mut()[$offset], value)
            }
        }
    };
}

#[macro_export(local_inner_macros)]
macro_rules! buffer {
    ($name:ident($buffer_len:expr) { $($field:ident : ($ty:tt, $offset:expr)),* $(,)? }) => {
        buffer!($name { $($field: ($ty, $offset),)* });
        buffer_check_length!($name($buffer_len));
    };

    ($name:ident { $($field:ident : ($ty:tt, $offset:expr)),* $(,)? }) => {
        buffer_common!($name);
        fields!($name {
            $($field: ($ty, $offset),)*
        });
    };

    ($name:ident, $buffer_len:expr) => {
        buffer_common!($name);
        buffer_check_length!($name($buffer_len));
    };

    ($name:ident) => {
        buffer_common!($name);
    };
}

#[macro_export(local_inner_macros)]
macro_rules! fields {
    ($buffer:ident { $($name:ident : ($ty:tt, $offset:expr)),* $(,)? }) => {
        $(
            getter!($buffer, $name, $ty, $offset);
        )*

            $(
                setter!($buffer, $name, $ty, $offset);
            )*
    }
}

#[macro_export]
macro_rules! buffer_check_length {
    ($name:ident($buffer_len:expr)) => {
        impl<T: AsRef<[u8]>> $name<T> {
            pub fn new_checked(buffer: T) -> Result<Self, DecodeError> {
                let packet = Self::new(buffer);
                packet.check_buffer_length()?;
                Ok(packet)
            }

            fn check_buffer_length(&self) -> Result<(), DecodeError> {
                let len = self.buffer.as_ref().len();
                if len < $buffer_len {
                    Err(format!(
                        concat!("invalid ", stringify!($name), ": length {} < {}"),
                        len, $buffer_len
                    )
                    .into())
                } else {
                    Ok(())
                }
            }
        }
    };
}

#[macro_export]
macro_rules! buffer_common {
    ($name:ident) => {
        #[derive(Debug, PartialEq, Eq, Clone, Copy)]
        pub struct $name<T> {
            buffer: T,
        }

        impl<T: AsRef<[u8]>> $name<T> {
            pub fn new(buffer: T) -> Self {
                Self { buffer }
            }

            pub fn into_inner(self) -> T {
                self.buffer
            }
        }

        impl<'a, T: AsRef<[u8]> + ?Sized> $name<&'a T> {
            pub fn inner(&self) -> &'a [u8] {
                &self.buffer.as_ref()[..]
            }
        }

        impl<'a, T: AsRef<[u8]> + AsMut<[u8]> + ?Sized> $name<&'a mut T> {
            pub fn inner_mut(&mut self) -> &mut [u8] {
                &mut self.buffer.as_mut()[..]
            }
        }
    };
}
