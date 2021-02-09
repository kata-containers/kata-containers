// Copyright © 2016, bitbegin
// Licensed under the MIT License <LICENSE.md>
//! USBIO Definitions.
STRUCT!{struct WINUSB_PIPE_INFORMATION {
	PipeType: ::USBD_PIPE_TYPE,
	PipeId: ::UCHAR,
	MaximumPacketSize: ::USHORT,
	Interval: ::UCHAR,
}}
pub type PWINUSB_PIPE_INFORMATION = *mut WINUSB_PIPE_INFORMATION;
STRUCT!{struct WINUSB_PIPE_INFORMATION_EX {
	PipeType: ::USBD_PIPE_TYPE,
	PipeId: ::UCHAR,
	MaximumPacketSize: ::USHORT,
	Interval: ::UCHAR,
	MaximumBytesPerInterval: ::ULONG,
}}
pub type PWINUSB_PIPE_INFORMATION_EX = *mut WINUSB_PIPE_INFORMATION_EX;
