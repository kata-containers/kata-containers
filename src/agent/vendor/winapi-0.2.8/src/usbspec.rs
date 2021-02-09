// Copyright © 2016, bitbegin
// Licensed under the MIT License <LICENSE.md>
//! USB Spec Definitions.
ENUM!{enum USB_DEVICE_SPEED {
    UsbLowSpeed = 0,
    UsbFullSpeed,
    UsbHighSpeed,
    UsbSuperSpeed,
}}
ENUM!{enum USB_DEVICE_TYPE {
    Usb11Device = 0,
    Usb20Device,
}}
STRUCT!{struct BM_REQUEST_TYPE {
    _BM: ::UCHAR,
    B: ::UCHAR,
}}
BITFIELD!{BM_REQUEST_TYPE _BM: ::UINT8 [
    Recipient set_Recipient[0..2],
    Reserved set_Reserved[2..5],
    Type set_Type[5..7],
    Dir set_Dir[7..8],
]}
pub type PBM_REQUEST_TYPE = *mut BM_REQUEST_TYPE;

STRUCT!{#[repr(packed)] struct USB_CONFIGURATION_DESCRIPTOR {
    bLength: ::UCHAR,
    bDescriptorType: ::UCHAR,
    wTotalLength: ::USHORT,
    bNumInterfaces: ::UCHAR,
    bConfigurationValue: ::UCHAR,
    iConfiguration: ::UCHAR,
    bmAttributes: ::UCHAR,
    MaxPower: ::UCHAR,
}}
pub type PUSB_CONFIGURATION_DESCRIPTOR = *mut USB_CONFIGURATION_DESCRIPTOR;
#[test]
fn test_USB_CONFIGURATION_DESCRIPTOR_size() {
    use std::mem::size_of;
    assert_eq!(size_of::<USB_CONFIGURATION_DESCRIPTOR>(), 9)
}
