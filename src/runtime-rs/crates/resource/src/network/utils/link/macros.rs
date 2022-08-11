// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

macro_rules! set_name {
    ($name_field:expr, $name_str:expr) => {{
        let name_c = &::std::ffi::CString::new($name_str.to_owned()).map_err(|_| {
            ::std::io::Error::new(
                ::std::io::ErrorKind::InvalidInput,
                "malformed interface name",
            )
        })?;
        let name_slice = name_c.as_bytes_with_nul();
        if name_slice.len() > libc::IFNAMSIZ {
            return Err(io::Error::new(::std::io::ErrorKind::InvalidInput, "").into());
        }
        $name_field[..name_slice.len()].clone_from_slice(name_slice);

        Ok(())
    }};
}

macro_rules! get_name {
    ($name_field:expr) => {{
        let nul_pos = match $name_field.iter().position(|x| *x == 0) {
            Some(p) => p,
            None => {
                return Err(::std::io::Error::new(
                    ::std::io::ErrorKind::InvalidData,
                    "malformed interface name",
                )
                .into())
            }
        };

        std::ffi::CString::new(&$name_field[..nul_pos])
            .unwrap()
            .into_string()
            .map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "malformed interface name")
            })
    }};
}

pub(crate) use get_name;
pub(crate) use set_name;
