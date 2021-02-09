// Copyright © 2016, bitbegin
// Licensed under the MIT License <LICENSE.md>
//! FFI bindings to winusb.
pub type WINUSB_INTERFACE_HANDLE = ::PVOID;
pub type PWINUSB_INTERFACE_HANDLE = *mut ::PVOID;
pub type WINUSB_ISOCH_BUFFER_HANDLE = ::PVOID;
pub type PWINUSB_ISOCH_BUFFER_HANDLE = *mut ::PVOID;
STRUCT!{#[repr(packed)] struct WINUSB_SETUP_PACKET {
	RequestType: ::UCHAR,
	Request: ::UCHAR,
	Value: ::USHORT,
	Index: ::USHORT,
	Length: ::USHORT,
}}
pub type PWINUSB_SETUP_PACKET = *mut WINUSB_SETUP_PACKET;

STRUCT!{struct USB_INTERFACE_DESCRIPTOR {
	bLength: ::UCHAR,
	bDescriptorType: ::UCHAR,
	bInterfaceNumber: ::UCHAR,
	bAlternateSetting: ::UCHAR,
	bNumEndpoints: ::UCHAR,
	bInterfaceClass: ::UCHAR,
	bInterfaceSubClass: ::UCHAR,
	bInterfaceProtocol: ::UCHAR,
	iInterface: ::UCHAR,
}}
pub type PUSB_INTERFACE_DESCRIPTOR = *mut USB_INTERFACE_DESCRIPTOR;
#[test]
fn test_USB_INTERFACE_DESCRIPTOR_size() {
    use std::mem::size_of;
    assert_eq!(size_of::<USB_INTERFACE_DESCRIPTOR>(), 9)
}
