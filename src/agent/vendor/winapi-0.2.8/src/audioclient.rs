// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
//! this ALWAYS GENERATED file contains the definitions for the interfaces
//1627
pub const AUDCLNT_E_NOT_INITIALIZED: ::HRESULT = AUDCLNT_ERR!(0x001);
pub const AUDCLNT_E_ALREADY_INITIALIZED: ::HRESULT = AUDCLNT_ERR!(0x002);
pub const AUDCLNT_E_WRONG_ENDPOINT_TYPE: ::HRESULT = AUDCLNT_ERR!(0x003);
pub const AUDCLNT_E_DEVICE_INVALIDATED: ::HRESULT = AUDCLNT_ERR!(0x004);
pub const AUDCLNT_E_NOT_STOPPED: ::HRESULT = AUDCLNT_ERR!(0x005);
pub const AUDCLNT_E_BUFFER_TOO_LARGE: ::HRESULT = AUDCLNT_ERR!(0x006);
pub const AUDCLNT_E_OUT_OF_ORDER: ::HRESULT = AUDCLNT_ERR!(0x007);
pub const AUDCLNT_E_UNSUPPORTED_FORMAT: ::HRESULT = AUDCLNT_ERR!(0x008);
pub const AUDCLNT_E_INVALID_SIZE: ::HRESULT = AUDCLNT_ERR!(0x009);
pub const AUDCLNT_E_DEVICE_IN_USE: ::HRESULT = AUDCLNT_ERR!(0x00a);
pub const AUDCLNT_E_BUFFER_OPERATION_PENDING: ::HRESULT = AUDCLNT_ERR!(0x00b);
pub const AUDCLNT_E_THREAD_NOT_REGISTERED: ::HRESULT = AUDCLNT_ERR!(0x00c);
pub const AUDCLNT_E_EXCLUSIVE_MODE_NOT_ALLOWED: ::HRESULT = AUDCLNT_ERR!(0x00e);
pub const AUDCLNT_E_ENDPOINT_CREATE_FAILED: ::HRESULT = AUDCLNT_ERR!(0x00f);
pub const AUDCLNT_E_SERVICE_NOT_RUNNING: ::HRESULT = AUDCLNT_ERR!(0x010);
pub const AUDCLNT_E_EVENTHANDLE_NOT_EXPECTED: ::HRESULT = AUDCLNT_ERR!(0x011);
pub const AUDCLNT_E_EXCLUSIVE_MODE_ONLY: ::HRESULT = AUDCLNT_ERR!(0x012);
pub const AUDCLNT_E_BUFDURATION_PERIOD_NOT_EQUAL: ::HRESULT = AUDCLNT_ERR!(0x013);
pub const AUDCLNT_E_EVENTHANDLE_NOT_SET: ::HRESULT = AUDCLNT_ERR!(0x014);
pub const AUDCLNT_E_INCORRECT_BUFFER_SIZE: ::HRESULT = AUDCLNT_ERR!(0x015);
pub const AUDCLNT_E_BUFFER_SIZE_ERROR: ::HRESULT = AUDCLNT_ERR!(0x016);
pub const AUDCLNT_E_CPUUSAGE_EXCEEDED: ::HRESULT = AUDCLNT_ERR!(0x017);
pub const AUDCLNT_E_BUFFER_ERROR: ::HRESULT = AUDCLNT_ERR!(0x018);
pub const AUDCLNT_E_BUFFER_SIZE_NOT_ALIGNED: ::HRESULT = AUDCLNT_ERR!(0x019);
pub const AUDCLNT_E_INVALID_DEVICE_PERIOD: ::HRESULT = AUDCLNT_ERR!(0x020);
pub const AUDCLNT_E_INVALID_STREAM_FLAG: ::HRESULT = AUDCLNT_ERR!(0x021);
pub const AUDCLNT_E_ENDPOINT_OFFLOAD_NOT_CAPABLE: ::HRESULT = AUDCLNT_ERR!(0x022);
pub const AUDCLNT_E_OUT_OF_OFFLOAD_RESOURCES: ::HRESULT = AUDCLNT_ERR!(0x023);
pub const AUDCLNT_E_OFFLOAD_MODE_ONLY: ::HRESULT = AUDCLNT_ERR!(0x024);
pub const AUDCLNT_E_NONOFFLOAD_MODE_ONLY: ::HRESULT = AUDCLNT_ERR!(0x025);
pub const AUDCLNT_E_RESOURCES_INVALIDATED: ::HRESULT = AUDCLNT_ERR!(0x026);
pub const AUDCLNT_E_RAW_MODE_UNSUPPORTED: ::HRESULT = AUDCLNT_ERR!(0x027);
pub const AUDCLNT_S_BUFFER_EMPTY: ::SCODE = AUDCLNT_SUCCESS!(0x001);
pub const AUDCLNT_S_THREAD_ALREADY_REGISTERED: ::SCODE = AUDCLNT_SUCCESS!(0x002);
pub const AUDCLNT_S_POSITION_STALLED: ::SCODE = AUDCLNT_SUCCESS!(0x003);
DEFINE_GUID!(IID_IAudioClient, 0x1CB9AD4C, 0xDBFA, 0x4c32,
    0xB1, 0x78, 0xC2, 0xF5, 0x68, 0xA7, 0x03, 0xB2);
DEFINE_GUID!(IID_IAudioRenderClient, 0xF294ACFC, 0x3146, 0x4483,
    0xA7, 0xBF, 0xAD, 0xDC, 0xA7, 0xC2, 0x60, 0xE2);
RIDL!{interface IAudioClient(IAudioClientVtbl): IUnknown(IUnknownVtbl) {
    fn Initialize(
        &mut self, ShareMode: ::AUDCLNT_SHAREMODE, StreamFlags: ::DWORD,
        hnsBufferDuration: ::REFERENCE_TIME, hnsPeriodicity: ::REFERENCE_TIME,
        pFormat: *const ::WAVEFORMATEX, AudioSessionGuid: ::LPCGUID
    ) -> ::HRESULT,
    fn GetBufferSize(&mut self, pNumBufferFrames: *mut ::UINT32) -> ::HRESULT,
    fn GetStreamLatency(&mut self, phnsLatency: *mut ::REFERENCE_TIME) -> ::HRESULT,
    fn GetCurrentPadding(&mut self, pNumPaddingFrames: *mut ::UINT32) -> ::HRESULT,
    fn IsFormatSupported(
        &mut self, ShareMode: ::AUDCLNT_SHAREMODE, pFormat: *const ::WAVEFORMATEX,
        ppClosestMatch: *mut *mut ::WAVEFORMATEX
    ) -> ::HRESULT,
    fn GetMixFormat(&mut self, ppDeviceFormat: *mut *mut ::WAVEFORMATEX) -> ::HRESULT,
    fn GetDevicePeriod(
        &mut self, phnsDefaultDevicePeriod: *mut ::REFERENCE_TIME,
        phnsMinimumDevicePeriod: *mut ::REFERENCE_TIME
    ) -> ::HRESULT,
    fn Start(&mut self) -> ::HRESULT,
    fn Stop(&mut self) -> ::HRESULT,
    fn Reset(&mut self) -> ::HRESULT,
    fn SetEventHandle(&mut self, eventHandle: ::HANDLE) -> ::HRESULT,
    fn GetService(&mut self, riid: ::REFIID, ppv: *mut ::LPVOID) -> ::HRESULT
}}
RIDL!{interface IAudioRenderClient(IAudioRenderClientVtbl): IUnknown(IUnknownVtbl) {
    fn GetBuffer(&mut self, NumFramesRequested: ::UINT32, ppData: *mut *mut ::BYTE) -> ::HRESULT,
    fn ReleaseBuffer(&mut self, NumFramesWritten: ::UINT32, dwFlags: ::DWORD) -> ::HRESULT
}}
