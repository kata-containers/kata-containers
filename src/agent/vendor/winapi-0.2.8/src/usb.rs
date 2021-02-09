// Copyright © 2016, bitbegin
// Licensed under the MIT License <LICENSE.md>
//! USB Definitions.
ENUM!{enum USBD_PIPE_TYPE {
    UsbdPipeTypeControl,
    UsbdPipeTypeIsochronous,
    UsbdPipeTypeBulk,
    UsbdPipeTypeInterrupt,
}}

pub type USBD_STATUS = ::LONG;

STRUCT!{struct USBD_ISO_PACKET_DESCRIPTOR {
	Offset: ::ULONG,
	Length: ::ULONG,
	Status: ::USBD_STATUS,
}}
pub type PUSBD_ISO_PACKET_DESCRIPTOR = *mut USBD_ISO_PACKET_DESCRIPTOR;
