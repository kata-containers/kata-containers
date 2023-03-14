// SPDX-License-Identifier: MIT

use std::{
    mem::size_of,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
};

use anyhow::Context;
use byteorder::{BigEndian, ByteOrder, NativeEndian};

use crate::DecodeError;

pub fn parse_mac(payload: &[u8]) -> Result<[u8; 6], DecodeError> {
    if payload.len() != 6 {
        return Err(format!("invalid MAC address: {:?}", payload).into());
    }
    let mut address: [u8; 6] = [0; 6];
    for (i, byte) in payload.iter().enumerate() {
        address[i] = *byte;
    }
    Ok(address)
}

pub fn parse_ipv6(payload: &[u8]) -> Result<[u8; 16], DecodeError> {
    if payload.len() != 16 {
        return Err(format!("invalid IPv6 address: {:?}", payload).into());
    }
    let mut address: [u8; 16] = [0; 16];
    for (i, byte) in payload.iter().enumerate() {
        address[i] = *byte;
    }
    Ok(address)
}

pub fn parse_ip(payload: &[u8]) -> Result<IpAddr, DecodeError> {
    match payload.len() {
        4 => Ok(Ipv4Addr::new(payload[0], payload[1], payload[2], payload[3]).into()),
        16 => Ok(Ipv6Addr::from([
            payload[0],
            payload[1],
            payload[2],
            payload[3],
            payload[4],
            payload[5],
            payload[6],
            payload[7],
            payload[8],
            payload[9],
            payload[10],
            payload[11],
            payload[12],
            payload[13],
            payload[14],
            payload[15],
        ])
        .into()),
        _ => Err(format!("invalid IPv6 address: {:?}", payload).into()),
    }
}

pub fn parse_string(payload: &[u8]) -> Result<String, DecodeError> {
    if payload.is_empty() {
        return Ok(String::new());
    }
    // iproute2 is a bit inconsistent with null-terminated strings.
    let slice = if payload[payload.len() - 1] == 0 {
        &payload[..payload.len() - 1]
    } else {
        &payload[..payload.len()]
    };
    let s = String::from_utf8(slice.to_vec()).context("invalid string")?;
    Ok(s)
}

pub fn parse_u8(payload: &[u8]) -> Result<u8, DecodeError> {
    if payload.len() != 1 {
        return Err(format!("invalid u8: {:?}", payload).into());
    }
    Ok(payload[0])
}

pub fn parse_u32(payload: &[u8]) -> Result<u32, DecodeError> {
    if payload.len() != size_of::<u32>() {
        return Err(format!("invalid u32: {:?}", payload).into());
    }
    Ok(NativeEndian::read_u32(payload))
}

pub fn parse_u64(payload: &[u8]) -> Result<u64, DecodeError> {
    if payload.len() != size_of::<u64>() {
        return Err(format!("invalid u64: {:?}", payload).into());
    }
    Ok(NativeEndian::read_u64(payload))
}

pub fn parse_u128(payload: &[u8]) -> Result<u128, DecodeError> {
    if payload.len() != size_of::<u128>() {
        return Err(format!("invalid u128: {:?}", payload).into());
    }
    Ok(NativeEndian::read_u128(payload))
}

pub fn parse_u16(payload: &[u8]) -> Result<u16, DecodeError> {
    if payload.len() != size_of::<u16>() {
        return Err(format!("invalid u16: {:?}", payload).into());
    }
    Ok(NativeEndian::read_u16(payload))
}

pub fn parse_i32(payload: &[u8]) -> Result<i32, DecodeError> {
    if payload.len() != 4 {
        return Err(format!("invalid u32: {:?}", payload).into());
    }
    Ok(NativeEndian::read_i32(payload))
}

pub fn parse_u16_be(payload: &[u8]) -> Result<u16, DecodeError> {
    if payload.len() != size_of::<u16>() {
        return Err(format!("invalid u16: {:?}", payload).into());
    }
    Ok(BigEndian::read_u16(payload))
}

pub fn parse_u32_be(payload: &[u8]) -> Result<u32, DecodeError> {
    if payload.len() != size_of::<u32>() {
        return Err(format!("invalid u32: {:?}", payload).into());
    }
    Ok(BigEndian::read_u32(payload))
}
