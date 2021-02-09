// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
//! DSound procedure declarations, constant definitions and macros
STRUCT!{struct DSCAPS {
    dwSize: ::DWORD,
    dwFlags: ::DWORD,
    dwMinSecondarySampleRate: ::DWORD,
    dwMaxSecondarySampleRate: ::DWORD,
    dwPrimaryBuffers: ::DWORD,
    dwMaxHwMixingAllBuffers: ::DWORD,
    dwMaxHwMixingStaticBuffers: ::DWORD,
    dwMaxHwMixingStreamingBuffers: ::DWORD,
    dwFreeHwMixingAllBuffers: ::DWORD,
    dwFreeHwMixingStaticBuffers: ::DWORD,
    dwFreeHwMixingStreamingBuffers: ::DWORD,
    dwMaxHw3DAllBuffers: ::DWORD,
    dwMaxHw3DStaticBuffers: ::DWORD,
    dwMaxHw3DStreamingBuffers: ::DWORD,
    dwFreeHw3DAllBuffers: ::DWORD,
    dwFreeHw3DStaticBuffers: ::DWORD,
    dwFreeHw3DStreamingBuffers: ::DWORD,
    dwTotalHwMemBytes: ::DWORD,
    dwFreeHwMemBytes: ::DWORD,
    dwMaxContigFreeHwMemBytes: ::DWORD,
    dwUnlockTransferRateHwBuffers: ::DWORD,
    dwPlayCpuOverheadSwBuffers: ::DWORD,
    dwReserved1: ::DWORD,
    dwReserved2: ::DWORD,
}}
pub type LPDSCAPS = *mut DSCAPS;
STRUCT!{struct DSBCAPS {
    dwSize: ::DWORD,
    dwFlags: ::DWORD,
    dwBufferBytes: ::DWORD,
    dwUnlockTransferRate: ::DWORD,
    dwPlayCpuOverhead: ::DWORD,
}}
pub type LPDSBCAPS = *mut DSBCAPS;
STRUCT!{struct DSBUFFERDESC {
    dwSize: ::DWORD,
    dwFlags: ::DWORD,
    dwBufferBytes: ::DWORD,
    dwReserved: ::DWORD,
    lpwfxFormat: ::LPWAVEFORMATEX,
    guid3DAlgorithm: ::GUID,
}}
pub type LPCDSBUFFERDESC = *const DSBUFFERDESC;
RIDL!(
interface IDirectSoundBuffer(IDirectSoundBufferVtbl): IUnknown(IUnknownVtbl) {
    fn GetCaps(&mut self, pDSBufferCaps: ::LPDSBCAPS) -> ::HRESULT,
    fn GetCurrentPosition(
        &mut self, pdwCurrentPlayCursor: ::LPDWORD, pdwCurrentWriteCursor: ::LPDWORD
    ) -> ::HRESULT,
    fn GetFormat(
        &mut self, pwfxFormat: ::LPWAVEFORMATEX, dwSizeAllocated: ::DWORD,
        pdwSizeWritten: ::LPDWORD
    ) -> ::HRESULT,
    fn GetVolume(&mut self, plVolume: ::LPLONG) -> ::HRESULT,
    fn GetPan(&mut self, plPan: ::LPLONG) -> ::HRESULT,
    fn GetFrequency(&mut self, pdwFrequency: ::LPDWORD) -> ::HRESULT,
    fn GetStatus(&mut self, pdwStatus: ::LPDWORD) -> ::HRESULT,
    fn Initialize(
        &mut self, pDirectSound: ::LPDIRECTSOUND, pcDSBufferDesc: ::LPCDSBUFFERDESC
    ) -> ::HRESULT,
    fn Lock(
        &mut self, dwOffset: ::DWORD, dwBytes: ::DWORD, ppvAudioPtr1: *mut ::LPVOID,
        pdwAudioBytes1: ::LPDWORD, ppvAudioPtr2: *mut ::LPVOID, pdwAudioBytes2: ::LPDWORD,
        dwFlags: ::DWORD
    ) -> ::HRESULT,
    fn Play(&mut self, dwReserved1: ::DWORD, dwPriority: ::DWORD, dwFlags: ::DWORD) -> ::HRESULT,
    fn SetCurrentPosition(&mut self, dwNewPosition: ::DWORD) -> ::HRESULT,
    fn SetFormat(&mut self, pcfxFormat: ::LPCWAVEFORMATEX) -> ::HRESULT,
    fn SetVolume(&mut self, lVolume: ::LONG) -> ::HRESULT,
    fn SetPan(&mut self, lPan: ::LONG) -> ::HRESULT,
    fn SetFrequency(&mut self, dwFrequency: ::DWORD) -> ::HRESULT,
    fn Stop(&mut self) -> ::HRESULT,
    fn Unlock(
        &mut self, pvAudioPtr1: ::LPVOID, dwAudioBytes1: ::DWORD, pvAudioPtr2: ::LPVOID,
        dwAudioBytes2: ::DWORD
    ) -> ::HRESULT,
    fn Restore(&mut self) -> ::HRESULT
}
);
pub type LPDIRECTSOUNDBUFFER = *mut IDirectSoundBuffer;
RIDL!(
interface IDirectSound(IDirectSoundVtbl): IUnknown(IUnknownVtbl)
{
    fn CreateSoundBuffer(
        &mut self, pcDSBufferDesc: ::LPCDSBUFFERDESC, ppDSBuffer: *mut ::LPDIRECTSOUNDBUFFER,
        pUnkOuter: ::LPUNKNOWN
    ) -> ::HRESULT,
    fn GetCaps(&mut self, pDSCaps: ::LPDSCAPS) -> ::HRESULT,
    fn DuplicateSoundBuffer(
        &mut self, pDSBufferOriginal: LPDIRECTSOUNDBUFFER,
        ppDSBufferDuplicate: *mut ::LPDIRECTSOUNDBUFFER
    ) -> ::HRESULT,
    fn SetCooperativeLevel(&mut self, hWnd: ::HWND, dwLevel: ::DWORD) -> ::HRESULT,
    fn Compact(&mut self) -> ::HRESULT,
    fn GetSpeakerConfig(&mut self, pdwSpeakerConfig: ::LPDWORD) -> ::HRESULT,
    fn SetSpeakerConfig(&mut self, dwSpeakerConfig: ::DWORD) -> ::HRESULT,
    fn Initialize(&mut self, pcGuidDevice: ::LPCGUID) -> ::HRESULT
}
);
pub type LPDIRECTSOUND = *mut IDirectSound;
pub const DS_OK: ::HRESULT = ::S_OK;
pub const DSERR_GENERIC: ::HRESULT = ::E_FAIL;
pub const DSSCL_NORMAL: ::DWORD = 0x00000001;
pub const DSSCL_PRIORITY: ::DWORD = 0x00000002;
pub const DSSCL_EXCLUSIVE: ::DWORD = 0x00000003;
pub const DSSCL_WRITEPRIMARY: ::DWORD = 0x00000004;
pub const DSBCAPS_PRIMARYBUFFER: ::DWORD = 0x00000001;
pub const DSBCAPS_STATIC: ::DWORD = 0x00000002;
pub const DSBCAPS_LOCHARDWARE: ::DWORD = 0x00000004;
pub const DSBCAPS_LOCSOFTWARE: ::DWORD = 0x00000008;
pub const DSBCAPS_CTRL3D: ::DWORD = 0x00000010;
pub const DSBCAPS_CTRLFREQUENCY: ::DWORD = 0x00000020;
pub const DSBCAPS_CTRLPAN: ::DWORD = 0x00000040;
pub const DSBCAPS_CTRLVOLUME: ::DWORD = 0x00000080;
pub const DSBCAPS_CTRLPOSITIONNOTIFY: ::DWORD = 0x00000100;
pub const DSBCAPS_CTRLFX: ::DWORD = 0x00000200;
pub const DSBCAPS_STICKYFOCUS: ::DWORD = 0x00004000;
pub const DSBCAPS_GLOBALFOCUS: ::DWORD = 0x00008000;
pub const DSBCAPS_GETCURRENTPOSITION2: ::DWORD = 0x00010000;
pub const DSBCAPS_MUTE3DATMAXDISTANCE: ::DWORD = 0x00020000;
pub const DSBCAPS_LOCDEFER: ::DWORD = 0x00040000;
pub const DSBCAPS_TRUEPLAYPOSITION: ::DWORD = 0x00080000;
pub const DSBPLAY_LOOPING: ::DWORD = 0x00000001;
pub const DSBPLAY_LOCHARDWARE: ::DWORD = 0x00000002;
pub const DSBPLAY_LOCSOFTWARE: ::DWORD = 0x00000004;
pub const DSBPLAY_TERMINATEBY_TIME: ::DWORD = 0x00000008;
pub const DSBPLAY_TERMINATEBY_DISTANCE: ::DWORD = 0x000000010;
pub const DSBPLAY_TERMINATEBY_PRIORITY: ::DWORD = 0x000000020;
