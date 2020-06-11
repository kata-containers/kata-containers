// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0

//! Parser for IPv4/IPv6/MAC addresses.

use std::net::{Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

use super::{Errno, Result, __u8, nix_errno};

#[inline]
pub(crate) fn parse_u8(s: &str, radix: u32) -> Result<u8> {
    if radix >= 2 && radix <= 36 {
        u8::from_str_radix(s, radix).map_err(|_| nix::Error::Sys(Errno::EINVAL))
    } else {
        u8::from_str(s).map_err(|_| nix::Error::Sys(Errno::EINVAL))
    }
}

pub fn parse_ipv4_addr(s: &str) -> Result<Vec<u8>> {
    match Ipv4Addr::from_str(s) {
        Ok(v) => Ok(Vec::from(v.octets().as_ref())),
        Err(_e) => nix_errno(Errno::EINVAL),
    }
}

pub fn parse_ip_addr(s: &str) -> Result<Vec<u8>> {
    if let Ok(v6) = Ipv6Addr::from_str(s) {
        Ok(Vec::from(v6.octets().as_ref()))
    } else {
        parse_ipv4_addr(s)
    }
}

pub fn parse_ip_addr_with_family(ip_address: &str) -> Result<(__u8, Vec<u8>)> {
    if let Ok(v6) = Ipv6Addr::from_str(ip_address) {
        Ok((libc::AF_INET6 as __u8, Vec::from(v6.octets().as_ref())))
    } else {
        parse_ipv4_addr(ip_address).map(|v| (libc::AF_INET as __u8, v))
    }
}

pub fn parse_ipv4_cidr(s: &str) -> Result<(Vec<u8>, u8)> {
    let fields: Vec<&str> = s.split('/').collect();

    if fields.len() != 2 {
        nix_errno(Errno::EINVAL)
    } else {
        Ok((parse_ipv4_addr(fields[0])?, parse_u8(fields[1], 10)?))
    }
}

pub fn parse_cidr(s: &str) -> Result<(Vec<u8>, u8)> {
    let fields: Vec<&str> = s.split('/').collect();

    if fields.len() != 2 {
        nix_errno(Errno::EINVAL)
    } else {
        Ok((parse_ip_addr(fields[0])?, parse_u8(fields[1], 10)?))
    }
}

pub fn parse_mac_addr(hwaddr: &str) -> Result<Vec<u8>> {
    let fields: Vec<&str> = hwaddr.split(':').collect();

    if fields.len() != 6 {
        nix_errno(Errno::EINVAL)
    } else {
        Ok(vec![
            parse_u8(fields[0], 16)?,
            parse_u8(fields[1], 16)?,
            parse_u8(fields[2], 16)?,
            parse_u8(fields[3], 16)?,
            parse_u8(fields[4], 16)?,
            parse_u8(fields[5], 16)?,
        ])
    }
}

/// Format an IPv4/IPv6/MAC address.
///
/// # Safety
/// Caller needs to ensure that addr and len are valid.
pub unsafe fn format_address(addr: *const u8, len: u32) -> Result<String> {
    let mut a: String;
    if len == 4 {
        // ipv4
        let mut i = 1;
        let mut p = addr as i64;

        a = format!("{}", *(p as *const u8));
        while i < len {
            p += 1;
            i += 1;
            a.push_str(format!(".{}", *(p as *const u8)).as_str());
        }

        return Ok(a);
    }

    if len == 6 {
        // hwaddr
        let mut i = 1;
        let mut p = addr as i64;

        a = format!("{:0>2X}", *(p as *const u8));
        while i < len {
            p += 1;
            i += 1;
            a.push_str(format!(":{:0>2X}", *(p as *const u8)).as_str());
        }

        return Ok(a);
    }

    if len == 16 {
        // ipv6
        let p = addr as *const u8 as *const libc::c_void;
        let mut ar: [u8; 16] = [0; 16];
        let mut v: Vec<u8> = vec![0; 16];
        let dp: *mut libc::c_void = v.as_mut_ptr() as *mut libc::c_void;
        libc::memcpy(dp, p, 16);

        ar.copy_from_slice(v.as_slice());

        return Ok(Ipv6Addr::from(ar).to_string());
    }

    nix_errno(Errno::EINVAL)
}

#[cfg(test)]
mod tests {
    use super::*;
    use libc;

    #[test]
    fn test_ip_addr() {
        let ip = parse_ipv4_addr("1.2.3.4").unwrap();
        assert_eq!(ip, vec![0x1u8, 0x2u8, 0x3u8, 0x4u8]);
        parse_ipv4_addr("1.2.3.4.5").unwrap_err();
        parse_ipv4_addr("1.2.3-4").unwrap_err();
        parse_ipv4_addr("1.2.3.a").unwrap_err();
        parse_ipv4_addr("1.2.3.x").unwrap_err();
        parse_ipv4_addr("-1.2.3.4").unwrap_err();
        parse_ipv4_addr("+1.2.3.4").unwrap_err();

        let (family, _) = parse_ip_addr_with_family("192.168.1.1").unwrap();
        assert_eq!(family, libc::AF_INET as __u8);

        let (family, ip) =
            parse_ip_addr_with_family("2001:0db8:85a3:0000:0000:8a2e:0370:7334").unwrap();
        assert_eq!(family, libc::AF_INET6 as __u8);
        assert_eq!(ip.len(), 16);
        parse_ip_addr_with_family("2001:0db8:85a3:0000:0000:8a2e:0370:73345").unwrap_err();

        let ip = parse_ip_addr("::1").unwrap();
        assert_eq!(ip[0], 0x0);
        assert_eq!(ip[15], 0x1);
    }

    #[test]
    fn test_parse_cidr() {
        let (_, mask) = parse_ipv4_cidr("1.2.3.4/31").unwrap();
        assert_eq!(mask, 31);

        parse_ipv4_cidr("1.2.3/4/31").unwrap_err();
        parse_ipv4_cidr("1.2.3.4/f").unwrap_err();
        parse_ipv4_cidr("1.2.3/8").unwrap_err();
        parse_ipv4_cidr("1.2.3.4.8").unwrap_err();

        let (ip, mask) = parse_cidr("2001:db8:a::123/64").unwrap();
        assert_eq!(mask, 64);
        assert_eq!(ip[0], 0x20);
        assert_eq!(ip[15], 0x23);
    }

    #[test]
    fn test_parse_mac_addr() {
        let mac = parse_mac_addr("FF:FF:FF:FF:FF:FE").unwrap();
        assert_eq!(mac.len(), 6);
        assert_eq!(mac[0], 0xff);
        assert_eq!(mac[5], 0xfe);

        parse_mac_addr("FF:FF:FF:FF:FF:FE:A0").unwrap_err();
        parse_mac_addr("FF:FF:FF:FF:FF:FX").unwrap_err();
        parse_mac_addr("FF:FF:FF:FF:FF").unwrap_err();
    }

    #[test]
    fn test_format_address() {
        let buf = [1u8, 2u8, 3u8, 4u8];
        let addr = unsafe { format_address(&buf as *const u8, 4).unwrap() };
        assert_eq!(addr, "1.2.3.4");

        let buf = [1u8, 2u8, 3u8, 4u8, 5u8, 6u8];
        let addr = unsafe { format_address(&buf as *const u8, 6).unwrap() };
        assert_eq!(addr, "01:02:03:04:05:06");
    }
}
