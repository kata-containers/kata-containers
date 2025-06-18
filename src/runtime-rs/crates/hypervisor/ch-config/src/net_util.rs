// Copyright (c) 2022-2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, Result};
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

pub const MAC_ADDR_LEN: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct MacAddr {
    pub bytes: [u8; MAC_ADDR_LEN],
}

impl MacAddr {
    pub fn new(addr: [u8; MAC_ADDR_LEN]) -> MacAddr {
        MacAddr { bytes: addr }
    }
}

// Note: Implements ToString automatically.
impl fmt::Display for MacAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let b = &self.bytes;
        write!(
            f,
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            b[0], b[1], b[2], b[3], b[4], b[5]
        )
    }
}

// Requried to remove the `bytes` member from the serialized JSON!
impl Serialize for MacAddr {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.to_string().serialize(serializer)
    }
}

// Helper function: parse MAC address string to byte array
fn parse_mac_address_str(s: &str) -> Result<[u8; MAC_ADDR_LEN]> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != MAC_ADDR_LEN {
        return Err(anyhow!(
            "Invalid MAC address format: expected {} parts separated by ':', got {}",
            MAC_ADDR_LEN,
            parts.len()
        ));
    }

    let mut bytes = [0u8; MAC_ADDR_LEN];
    for (i, part) in parts.iter().enumerate() {
        if part.len() != 2 {
            return Err(anyhow!(
                "Invalid MAC address part '{}': expected 2 hex digits",
                part
            ));
        }
        bytes[i] = u8::from_str_radix(part, 16)
            .map_err(|e| anyhow!("Invalid hex digit in '{}': {}", part, e))?;
    }
    Ok(bytes)
}

// Customize Deserialize implementation, because the system's own one does not work.
impl<'de> Deserialize<'de> for MacAddr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // We expect the deserializer to provide a string, so we use deserialize_string
        deserializer.deserialize_string(MacAddrVisitor)
    }
}

// MacAddrVisitor will handle the actual conversion from string to MacAddr
struct MacAddrVisitor;

impl Visitor<'_> for MacAddrVisitor {
    type Value = MacAddr;

    // When deserialization fails, Serde will call this method to get a description of the expected format
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a MAC address string in format \"XX:XX:XX:XX:XX:XX\"")
    }

    // Called when the deserializer provides a string slice
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        // Use our auxiliary function to parse the string and convert it to MacAddr
        parse_mac_address_str(v)
            .map(MacAddr::new) // If the parsing is successful, create a MacAddr with a byte array
            .map_err(de::Error::custom) // If parsing fails, convert the error to Serde's error type
    }

    // Called when the deserializer provides a String (usually delegated to visit_str)
    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_str(&v)
    }
}

#[cfg(test)]
mod tests {
    use super::*; // Import parent module items, including MAC_ADDR_LEN and parse_mac_address_str

    #[test]
    fn test_parse_mac_address_str_valid() {
        // Test a standard MAC address
        let mac_str = "00:11:22:33:44:55";
        let expected_bytes = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55];
        assert_eq!(parse_mac_address_str(mac_str).unwrap(), expected_bytes);

        // Test a MAC address with uppercase letters
        let mac_str_upper = "AA:BB:CC:DD:EE:FF";
        let expected_bytes_upper = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        assert_eq!(
            parse_mac_address_str(mac_str_upper).unwrap(),
            expected_bytes_upper
        );

        // Test a mixed-case MAC address
        let mac_str_mixed = "aA:Bb:Cc:Dd:Ee:Ff";
        let expected_bytes_mixed = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        assert_eq!(
            parse_mac_address_str(mac_str_mixed).unwrap(),
            expected_bytes_mixed
        );

        // Test an all-zero MAC address
        let mac_str_zero = "00:00:00:00:00:00";
        let expected_bytes_zero = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        assert_eq!(
            parse_mac_address_str(mac_str_zero).unwrap(),
            expected_bytes_zero
        );
    }

    #[test]
    fn test_parse_mac_address_str_invalid_length() {
        // MAC address with too few segments
        let mac_str_short = "00:11:22:33:44";
        let err = parse_mac_address_str(mac_str_short).unwrap_err();
        assert!(err
            .to_string()
            .contains("Invalid MAC address format: expected 6 parts separated by ':', got 5"));

        // MAC address with too many segments
        let mac_str_long = "00:11:22:33:44:55:66";
        let err = parse_mac_address_str(mac_str_long).unwrap_err();
        assert!(err
            .to_string()
            .contains("Invalid MAC address format: expected 6 parts separated by ':', got 7"));

        // Empty string
        let mac_str_empty = "";
        let err = parse_mac_address_str(mac_str_empty).unwrap_err();
        // Note: split(':') on an empty string returns a Vec containing [""] if delimiter is not found,
        // so its length will be 1.
        assert!(err
            .to_string()
            .contains("Invalid MAC address format: expected 6 parts separated by ':', got 1"));
    }

    #[test]
    fn test_parse_mac_address_str_invalid_part_length() {
        // Part with insufficient length (1 digit)
        let mac_str_part_short = "0:11:22:33:44:55";
        let err = parse_mac_address_str(mac_str_part_short).unwrap_err();
        assert!(err
            .to_string()
            .contains("Invalid MAC address part '0': expected 2 hex digits"));

        // Part with excessive length (3 digits)
        let mac_str_part_long = "000:11:22:33:44:55";
        let err = parse_mac_address_str(mac_str_part_long).unwrap_err();
        assert!(err
            .to_string()
            .contains("Invalid MAC address part '000': expected 2 hex digits"));
    }

    #[test]
    fn test_parse_mac_address_str_invalid_chars() {
        // Contains non-hexadecimal character (letter G)
        let mac_str_invalid_char_g = "00:11:22:33:44:GG";
        let err = parse_mac_address_str(mac_str_invalid_char_g).unwrap_err();
        assert!(err.to_string().contains("Invalid hex digit in 'GG'"));

        // Contains non-hexadecimal character (symbol @)
        let mac_str_invalid_char_at = "00:11:22:33:44:@5";
        let err = parse_mac_address_str(mac_str_invalid_char_at).unwrap_err();
        assert!(err.to_string().contains("Invalid hex digit in '@5'"));

        // Contains whitespace character
        let mac_str_with_space = "00:11:22:33:44: 5";
        let err = parse_mac_address_str(mac_str_with_space).unwrap_err();
        assert!(err.to_string().contains("Invalid hex digit in ' 5'"));
    }

    #[test]
    fn test_parse_mac_address_str_malformed_string() {
        // String with only colons
        let mac_str_colon_only = ":::::";
        let err = parse_mac_address_str(mac_str_colon_only).unwrap_err();
        // Each empty part will trigger the "expected 2 hex digits" error
        assert!(err
            .to_string()
            .contains("Invalid MAC address part '': expected 2 hex digits"));

        // String with trailing colon
        let mac_str_trailing_colon = "00:11:22:33:44:55:";
        let err = parse_mac_address_str(mac_str_trailing_colon).unwrap_err();
        assert!(err
            .to_string()
            .contains("Invalid MAC address format: expected 6 parts separated by ':', got 7"));
    }
}
