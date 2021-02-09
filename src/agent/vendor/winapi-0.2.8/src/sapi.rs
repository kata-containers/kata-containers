// Copyright © 2015, Connor Hilarides
// Licensed under the MIT License <LICENSE.md>
//! Mappings for the contents of sapi.h
ENUM!{enum SPDATAKEYLOCATION {
    SPDKL_DefaultLocation = 0,
    SPDKL_CurrentUser = 1,
    SPDKL_LocalMachine = 2,
    SPDKL_CurrentConfig = 5,
}}
pub const SPDUI_EngineProperties: &'static str = "EngineProperties";
pub const SPDUI_AddRemoveWord: &'static str = "AddRemoveWord";
pub const SPDUI_UserTraining: &'static str = "UserTraining";
pub const SPDUI_MicTraining: &'static str = "MicTraining";
pub const SPDUI_RecoProfileProperties: &'static str = "RecoProfileProperties";
pub const SPDUI_AudioProperties: &'static str = "AudioProperties";
pub const SPDUI_AudioVolume: &'static str = "AudioVolume";
pub const SPDUI_UserEnrollment: &'static str = "UserEnrollment";
pub const SPDUI_ShareData: &'static str = "ShareData";
pub const SPDUI_Tutorial: &'static str = "Tutorial";
ENUM!{enum SPSTREAMFORMAT {
     SPSF_Default = -1i32 as u32,
     SPSF_NoAssignedFormat = 0,
     SPSF_Text = 1,
     SPSF_NonStandardFormat = 2,
     SPSF_ExtendedAudioFormat = 3,
     SPSF_8kHz8BitMono = 4,
     SPSF_8kHz8BitStereo = 5,
     SPSF_8kHz16BitMono = 6,
     SPSF_8kHz16BitStereo = 7,
     SPSF_11kHz8BitMono = 8,
     SPSF_11kHz8BitStereo = 9,
     SPSF_11kHz16BitMono = 10,
     SPSF_11kHz16BitStereo = 11,
     SPSF_12kHz8BitMono = 12,
     SPSF_12kHz8BitStereo = 13,
     SPSF_12kHz16BitMono = 14,
     SPSF_12kHz16BitStereo = 15,
     SPSF_16kHz8BitMono = 16,
     SPSF_16kHz8BitStereo = 17,
     SPSF_16kHz16BitMono = 18,
     SPSF_16kHz16BitStereo = 19,
     SPSF_22kHz8BitMono = 20,
     SPSF_22kHz8BitStereo = 21,
     SPSF_22kHz16BitMono = 22,
     SPSF_22kHz16BitStereo = 23,
     SPSF_24kHz8BitMono = 24,
     SPSF_24kHz8BitStereo = 25,
     SPSF_24kHz16BitMono = 26,
     SPSF_24kHz16BitStereo = 27,
     SPSF_32kHz8BitMono = 28,
     SPSF_32kHz8BitStereo = 29,
     SPSF_32kHz16BitMono = 30,
     SPSF_32kHz16BitStereo = 31,
     SPSF_44kHz8BitMono = 32,
     SPSF_44kHz8BitStereo = 33,
     SPSF_44kHz16BitMono = 34,
     SPSF_44kHz16BitStereo = 35,
     SPSF_48kHz8BitMono = 36,
     SPSF_48kHz8BitStereo = 37,
     SPSF_48kHz16BitMono = 38,
     SPSF_48kHz16BitStereo = 39,
     SPSF_TrueSpeech_8kHz1BitMono = 40,
     SPSF_CCITT_ALaw_8kHzMono = 41,
     SPSF_CCITT_ALaw_8kHzStereo = 42,
     SPSF_CCITT_ALaw_11kHzMono = 43,
     SPSF_CCITT_ALaw_11kHzStereo = 44,
     SPSF_CCITT_ALaw_22kHzMono = 45,
     SPSF_CCITT_ALaw_22kHzStereo = 46,
     SPSF_CCITT_ALaw_44kHzMono = 47,
     SPSF_CCITT_ALaw_44kHzStereo = 48,
     SPSF_CCITT_uLaw_8kHzMono = 49,
     SPSF_CCITT_uLaw_8kHzStereo = 50,
     SPSF_CCITT_uLaw_11kHzMono = 51,
     SPSF_CCITT_uLaw_11kHzStereo = 52,
     SPSF_CCITT_uLaw_22kHzMono = 53,
     SPSF_CCITT_uLaw_22kHzStereo = 54,
     SPSF_CCITT_uLaw_44kHzMono = 55,
     SPSF_CCITT_uLaw_44kHzStereo = 56,
     SPSF_ADPCM_8kHzMono = 57,
     SPSF_ADPCM_8kHzStereo = 58,
     SPSF_ADPCM_11kHzMono = 59,
     SPSF_ADPCM_11kHzStereo = 60,
     SPSF_ADPCM_22kHzMono = 61,
     SPSF_ADPCM_22kHzStereo = 62,
     SPSF_ADPCM_44kHzMono = 63,
     SPSF_ADPCM_44kHzStereo = 64,
     SPSF_GSM610_8kHzMono = 65,
     SPSF_GSM610_11kHzMono = 66,
     SPSF_GSM610_22kHzMono = 67,
     SPSF_GSM610_44kHzMono = 68,
     SPSF_NUM_FORMATS = 69,
}}
pub const SPREG_USER_ROOT: &'static str = "HKEY_CURRENT_USER\\SOFTWARE\\Microsoft\\Speech";
pub const SPREG_LOCAL_MACHINE_ROOT: &'static str = "HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Speech";
pub const SPCAT_AUDIOOUT: &'static str = "HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Speech\\AudioOutput";
pub const SPCAT_AUDIOIN: &'static str = "HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Speech\\AudioInput";
pub const SPCAT_VOICES: &'static str = "HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Speech\\Voices";
pub const SPCAT_RECOGNIZERS: &'static str = "HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Speech\\Recognizers";
pub const SPCAT_APPLEXICONS: &'static str = "HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Speech\\AppLexicons";
pub const SPCAT_PHONECONVERTERS: &'static str = "HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Speech\\PhoneConverters";
pub const SPCAT_TEXTNORMALIZERS: &'static str = "HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Speech\\TextNormalizers";
pub const SPCAT_RECOPROFILES: &'static str = "HKEY_CURRENT_USER\\SOFTWARE\\Microsoft\\Speech\\RecoProfiles";
pub const SPMMSYS_AUDIO_IN_TOKEN_ID: &'static str = "HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Speech\\AudioInput\\TokenEnums\\MMAudioIn\\";
pub const SPMMSYS_AUDIO_OUT_TOKEN_ID: &'static str = "HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Speech\\AudioOutput\\TokenEnums\\MMAudioOut\\";
pub const SPCURRENT_USER_LEXICON_TOKEN_ID: &'static str = "HKEY_CURRENT_USER\\SOFTWARE\\Microsoft\\Speech\\CurrentUserLexicon";
pub const SPCURRENT_USER_SHORTCUT_TOKEN_ID: &'static str = "HKEY_CURRENT_USER\\SOFTWARE\\Microsoft\\Speech\\CurrentUserShortcut";
pub const SPTOKENVALUE_CLSID: &'static str = "CLSID";
pub const SPTOKENKEY_FILES: &'static str = "Files";
pub const SPTOKENKEY_UI: &'static str = "UI";
pub const SPTOKENKEY_ATTRIBUTES: &'static str = "Attributes";
pub const SPTOKENKEY_RETAINEDAUDIO: &'static str = "SecondsPerRetainedAudioEvent";
pub const SPTOKENKEY_AUDIO_LATENCY_WARNING: &'static str = "LatencyWarningThreshold";
pub const SPTOKENKEY_AUDIO_LATENCY_TRUNCATE: &'static str = "LatencyTruncateThreshold";
pub const SPTOKENKEY_AUDIO_LATENCY_UPDATE_INTERVAL: &'static str = "LatencyUpdateInterval";
pub const SPVOICECATEGORY_TTSRATE: &'static str = "DefaultTTSRate";
pub const SPPROP_RESOURCE_USAGE: &'static str = "ResourceUsage";
pub const SPPROP_HIGH_CONFIDENCE_THRESHOLD: &'static str = "HighConfidenceThreshold";
pub const SPPROP_NORMAL_CONFIDENCE_THRESHOLD: &'static str = "NormalConfidenceThreshold";
pub const SPPROP_LOW_CONFIDENCE_THRESHOLD: &'static str = "LowConfidenceThreshold";
pub const SPPROP_RESPONSE_SPEED: &'static str = "ResponseSpeed";
pub const SPPROP_COMPLEX_RESPONSE_SPEED: &'static str = "ComplexResponseSpeed";
pub const SPPROP_ADAPTATION_ON: &'static str = "AdaptationOn";
pub const SPPROP_PERSISTED_BACKGROUND_ADAPTATION: &'static str = "PersistedBackgroundAdaptation";
pub const SPPROP_PERSISTED_LANGUAGE_MODEL_ADAPTATION: &'static str = "PersistedLanguageModelAdaptation";
pub const SPPROP_UX_IS_LISTENING: &'static str = "UXIsListening";
pub const SPTOPIC_SPELLING: &'static str = "Spelling";
pub const SPWILDCARD: &'static str = "...";
pub const SPDICTATION: &'static str = "*";
pub const SPINFDICTATION: &'static str = "*+";
pub const SPREG_SAFE_USER_TOKENS: &'static str = "HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Speech\\UserTokens";
pub const SP_LOW_CONFIDENCE: i32 = -1;
pub const SP_NORMAL_CONFIDENCE: i32 = 0;
pub const SP_HIGH_CONFIDENCE: i32 = 1;
pub const DEFAULT_WEIGHT: i32 = 1;
pub const SP_MAX_WORD_LENGTH: i32 = 128;
pub const SP_MAX_PRON_LENGTH: i32 = 384;
pub const SP_EMULATE_RESULT: i32 = 0x40000000;
RIDL!(
interface ISpNotifyCallback(ISpNotifyCallbackVtbl) {
    fn NotifyCallback(&mut self, wParam: ::WPARAM, lParam: ::LPARAM) -> ::HRESULT
}
);
pub type SPNOTIFYCALLBACK = unsafe extern "system" fn(wParam: ::WPARAM, lParam: ::LPARAM);
RIDL!(
interface ISpNotifySource(ISpNotifySourceVtbl): IUnknown(IUnknownVtbl) {
    fn SetNotifySink(&mut self, pNotifySink: *mut ISpNotifySink) -> ::HRESULT,
    fn SetNotifyWindowMessage(
        &mut self, hWnd: ::HWND, Msg: ::UINT, wParam: ::WPARAM, lParam: ::LPARAM
    ) -> ::HRESULT,
    fn SetNotifyCallbackFunction(
        &mut self, pfnCallback: SPNOTIFYCALLBACK, wParam: ::WPARAM, lParam: ::LPARAM
    ) -> ::HRESULT,
    fn SetNotifyCallbackInterface(
        &mut self, pSpCallback: *mut ISpNotifyCallback, wParam: ::WPARAM, lParam: ::LPARAM
    ) -> ::HRESULT,
    fn SetNotifyWin32Event(&mut self) -> ::HRESULT,
    fn WaitForNotifyEvent(&mut self, dwMilliseconds: ::DWORD) -> ::HRESULT,
    fn GetNotifyEventHandle(&mut self) -> ::HANDLE
}
);
RIDL!(
interface ISpNotifySink(ISpNotifySinkVtbl): IUnknown(IUnknownVtbl) {
    fn Notify(&mut self) -> ::HRESULT
}
);
RIDL!(
interface ISpNotifyTranslator(ISpNotifyTranslatorVtbl): ISpNotifySink(ISpNotifySinkVtbl) {
    fn InitWindowMessage(
        &mut self, hWnd: ::HWND, Msg: ::UINT, wParam: ::WPARAM, lParam: ::LPARAM
    ) -> ::HRESULT,
    fn InitCallback(
        &mut self, pfnCallback: SPNOTIFYCALLBACK, wParam: ::WPARAM, lParam: ::LPARAM
    ) -> ::HRESULT,
    fn InitSpNotifyCallback(
        &mut self, pSpCallback: *mut ISpNotifyCallback, wParam: ::WPARAM, lParam: ::LPARAM
    ) -> ::HRESULT,
    fn InitWin32Event(&mut self, hEvent: ::HANDLE, fCloseHandleOnRelease: ::BOOL) -> ::HRESULT,
    fn Wait(&mut self, dwMilliseconds: ::DWORD) -> ::HRESULT,
    fn GetEventHandle(&mut self) -> ::HANDLE
}
);
RIDL!(
interface ISpDataKey(ISpDataKeyVtbl): IUnknown(IUnknownVtbl) {
    fn SetData(
        &mut self, pszValueName: ::LPCWSTR, cbData: ::ULONG, pData: *const ::BYTE
    ) -> ::HRESULT,
    fn GetData(
        &mut self, pszValueName: ::LPCWSTR, pcbData: *mut ::ULONG, pData: *mut ::BYTE
    ) -> ::HRESULT,
    fn SetStringValue(&mut self, pszValueName: ::LPCWSTR, pszValue: ::LPCWSTR) -> ::HRESULT,
    fn GetStringValue(&mut self, pszValueName: ::LPCWSTR, ppszValue: *mut ::LPWSTR) -> ::HRESULT,
    fn SetDWORD(&mut self, pszValueName: ::LPCWSTR, dwValue: ::DWORD) -> ::HRESULT,
    fn GetDWORD(&mut self, pszValueName: ::LPCWSTR, pdwValue: *mut ::DWORD) -> ::HRESULT,
    fn OpenKey(&mut self, pszSubKeyName: ::LPCWSTR, ppSubKey: *mut *mut ISpDataKey) -> ::HRESULT,
    fn CreateKey(&mut self, pszSubKey: ::LPCWSTR, ppSubKey: *mut *mut ISpDataKey) -> ::HRESULT,
    fn DeleteKey(&mut self, pszSubKey: ::LPCWSTR) -> ::HRESULT,
    fn DeleteValue(&mut self, pszValueName: ::LPCWSTR) -> ::HRESULT,
    fn EnumKeys(&mut self, Index: ::ULONG, ppszSubKeyName: *mut ::LPWSTR) -> ::HRESULT,
    fn EnumValues(&mut self, Index: ::ULONG, ppszValueName: *mut ::LPWSTR) -> ::HRESULT
}
);
RIDL!(
interface ISpRegDataKey(ISpRegDataKeyVtbl): ISpDataKey(ISpDataKeyVtbl) {
    fn SetKey(&mut self, hkey: ::HKEY, fReadOnly: ::BOOL) -> ::HRESULT
}
);
RIDL!(
interface ISpObjectTokenCategory(ISpObjectTokenCategoryVtbl): ISpDataKey(ISpDataKeyVtbl) {
    fn SetId(&mut self, pszCategoryId: ::LPCWSTR, fCreateIfNotExist: ::BOOL) -> ::HRESULT,
    fn GetId(&mut self, ppszCoMemCategoryId: *mut ::LPWSTR) -> ::HRESULT,
    fn GetDataKey(
        &mut self, spdkl: SPDATAKEYLOCATION, pppDataKey: *mut *mut ISpDataKey
    ) -> ::HRESULT,
    fn EnumTokens(
        &mut self, pzsReqAttribs: ::LPCWSTR, pszOptAttribs: ::LPCWSTR,
        ppEnum: *mut *mut IEnumSpObjectTokens
    ) -> ::HRESULT,
    fn SetDefaultTokenId(&mut self, pszTokenId: ::LPCWSTR) -> ::HRESULT,
    fn GetDefaultTokenId(&mut self, ppszCoMemTokenId: *mut ::LPWSTR) -> ::HRESULT
}
);
RIDL!(
interface ISpObjectToken(ISpObjectTokenVtbl): ISpDataKey(ISpDataKeyVtbl) {
    fn SetId(
        &mut self, pszCategoryId: ::LPCWSTR, pszTokenId: ::LPCWSTR, fCreateIfNotExist: ::BOOL
    ) -> ::HRESULT,
    fn GetId(&mut self, ppszCoMemTokenId: *mut ::LPWSTR) -> ::HRESULT,
    fn GetCategory(&mut self, ppTokenCategory: *mut *mut ISpObjectTokenCategory) -> ::HRESULT,
    fn CreateInstance(
        &mut self, pUnkOuter: *mut ::IUnknown, dwClsContext: ::DWORD, riid: ::REFIID,
        ppvObject: *mut *mut ::c_void
    ) -> ::HRESULT,
    fn GetStorageFileName(
        &mut self, clsidCaller: ::REFCLSID, pszValueName: ::LPCWSTR,
        pszFileNameSpecifier: ::LPCWSTR, nFolder: ::ULONG, ppszFilePath: *mut ::LPWSTR
    ) -> ::HRESULT,
    fn RemoveStorageFileName(&mut self, pszKeyName: ::LPCWSTR, fDeleteFile: ::BOOL) -> ::HRESULT,
    fn Remove(&mut self, pclsidCaller: *const ::CLSID) -> ::HRESULT,
    fn IsUISupported(
        &mut self, pszTypeOfUI: ::LPCWSTR, pvExtraData: *mut ::c_void, cbExtraData: ::ULONG,
        punkObject: *mut ::IUnknown, pfSupported: *mut ::BOOL
    ) -> ::HRESULT,
    fn DisplayUI(
        &mut self, hwndParent: ::HWND, pszTitle: ::LPCWSTR, pszTypeOfUI: ::LPCWSTR,
        pvExtraData: *mut ::c_void, cbExtraData: ::ULONG, punkObject: *mut ::IUnknown
    ) -> ::HRESULT,
    fn MatchesAttributes(&mut self, pszAttributes: ::LPCWSTR, pfMatches: *mut ::BOOL) -> ::HRESULT
}
);
RIDL!(
interface ISpObjectTokenInit(ISpObjectTokenInitVtbl): ISpObjectToken(ISpObjectTokenVtbl) {
    fn InitFromDataKey(
        &mut self, pszCategoryId: ::LPCWSTR, pszTokenId: ::LPCWSTR, pDataKey: *mut ISpDataKey
    ) -> ::HRESULT
}
);
RIDL!(
interface IEnumSpObjectTokens(IEnumSpObjectTokensVtbl): IUnknown(IUnknownVtbl) {
    fn Next(
        &mut self, celt: ::ULONG, pelt: *mut *mut ISpObjectToken, pceltFetched: *mut ::ULONG
    ) -> ::HRESULT,
    fn Skip(&mut self, celt: ::ULONG) -> ::HRESULT,
    fn Reset(&mut self) -> ::HRESULT,
    fn Clone(&mut self, ppEnum: *mut *mut IEnumSpObjectTokens) -> ::HRESULT,
    fn Item(&mut self, Index: ::ULONG, ppToken: *mut *mut ISpObjectToken) -> ::HRESULT,
    fn GetCount(&mut self, pCount: *mut ::ULONG) -> ::HRESULT
}
);
RIDL!(
interface ISpObjectWithToken(ISpObjectWithTokenVtbl): IUnknown(IUnknownVtbl) {
    fn SetObjectToken(&mut self, pToken: *mut ISpObjectToken) -> ::HRESULT,
    fn GetObjectToken(&mut self, ppToken: *mut *mut ISpObjectToken) -> ::HRESULT
}
);
RIDL!(
interface ISpResourceManager(ISpResourceManagerVtbl): IServiceProvider(IServiceProviderVtbl) {
    fn SetObject(&mut self, guidServiceId: ::REFGUID, pUnkObject: *mut ::IUnknown) -> ::HRESULT,
    fn GetObject(
        &mut self, guidServiceId: ::REFGUID, ObjectCLSID: ::REFCLSID, ObjectIID: ::REFIID,
        fReleaseWhenLastExternalRefReleased: ::BOOL, ppObject: *mut *mut ::c_void
    ) -> ::HRESULT
}
);
ENUM!{enum SPEVENTLPARAMTYPE {
    SPET_LPARAM_IS_UNDEFINED = 0,
    SPET_LPARAM_IS_TOKEN,
    SPET_LPARAM_IS_OBJECT,
    SPET_LPARAM_IS_POINTER,
    SPET_LPARAM_IS_STRING,
}}
ENUM!{enum SPEVENTENUM {
    SPEI_UNDEFINED = 0,
    SPEI_START_INPUT_STREAM = 1,
    SPEI_END_INPUT_STREAM = 2,
    SPEI_VOICE_CHANGE = 3,
    SPEI_TTS_BOOKMARK = 4,
    SPEI_WORD_BOUNDARY = 5,
    SPEI_PHONEME = 6,
    SPEI_SENTENCE_BOUNDARY = 7,
    SPEI_VISEME = 8,
    SPEI_TTS_AUDIO_LEVEL = 9,
    SPEI_TTS_PRIVATE = 15,
    SPEI_END_SR_STREAM = 34,
    SPEI_SOUND_START = 35,
    SPEI_SOUND_END = 36,
    SPEI_PHRASE_START = 37,
    SPEI_RECOGNITION = 38,
    SPEI_HYPOTHESIS = 39,
    SPEI_SR_BOOKMARK = 40,
    SPEI_PROPERTY_NUM_CHANGE = 41,
    SPEI_PROPERTY_STRING_CHANGE = 42,
    SPEI_FALSE_RECOGNITION = 43,
    SPEI_INTERFERENCE = 44,
    SPEI_REQUEST_UI = 45,
    SPEI_RECO_STATE_CHANGE = 46,
    SPEI_ADAPTATION = 47,
    SPEI_START_SR_STREAM = 48,
    SPEI_RECO_OTHER_CONTEXT = 49,
    SPEI_SR_AUDIO_LEVEL = 50,
    SPEI_SR_RETAINEDAUDIO = 51,
    SPEI_SR_PRIVATE = 52,
    SPEI_ACTIVE_CATEGORY_CHANGED = 53,
    SPEI_RESERVED5 = 54,
    SPEI_RESERVED6 = 55,
    SPEI_RESERVED1 = 30,
    SPEI_RESERVED2 = 33,
    SPEI_RESERVED3 = 63,
}}
pub const SPEI_MIN_TTS: SPEVENTENUM = SPEI_START_INPUT_STREAM;
pub const SPEI_MAX_TTS: SPEVENTENUM = SPEI_TTS_PRIVATE;
pub const SPEI_MIN_SR: SPEVENTENUM = SPEI_END_SR_STREAM;
pub const SPEI_MAX_SR: SPEVENTENUM = SPEI_RESERVED6;
pub const SPFEI_FLAGCHECK: u64 = (1 << SPEI_RESERVED1.0 as u64) | (1 << SPEI_RESERVED2.0 as u64);
pub const SPFEI_ALL_TTS_EVENTS: u64 = 0x000000000000FFFE | SPFEI_FLAGCHECK;
pub const SPFEI_ALL_SR_EVENTS: u64 = 0x003FFFFC00000000 | SPFEI_FLAGCHECK;
pub const SPFEI_ALL_EVENTS: u64 = 0xEFFFFFFFFFFFFFFF;
#[inline]
pub fn SPFEI(SPEI_ord: u64) -> u64 {
    (1 << SPEI_ord) | SPFEI_FLAGCHECK
}
STRUCT!{struct SPEVENT {
    eEventId: ::WORD,
    elParamType: ::WORD,
    ulStreamNum: ::ULONG,
    ullAudioStreamOffset: ::ULONGLONG,
    wParam: ::WPARAM,
    lParam: ::LPARAM,
}}
STRUCT!{struct SPSERIALIZEDEVENT {
    eEventId: ::WORD,
    elParamType: ::WORD,
    ulStreamNum: ::ULONG,
    ullAudioStreamOffset: ::ULONGLONG,
    SerializedwParam: ::ULONG,
    SerializedlParam: ::LONG,
}}
STRUCT!{struct SPSERIALIZEDEVENT64 {
    eEventId: ::WORD,
    elParamType: ::WORD,
    ulStreamNum: ::ULONG,
    ullAudioStreamOffset: ::ULONGLONG,
    SerializedwParam: ::ULONGLONG,
    SerializedlParam: ::LONGLONG,
}}
STRUCT!{struct SPEVENTEX {
    eEventId: ::WORD,
    elParamType: ::WORD,
    ulStreamNum: ::ULONG,
    ullAudioStreamOffset: ::ULONGLONG,
    wParam: ::WPARAM,
    lParam: ::LPARAM,
    ullAudioTimeOffset: ::ULONGLONG,
}}
ENUM!{enum SPINTERFERENCE {
    SPINTERFERENCE_NONE = 0,
    SPINTERFERENCE_NOISE = 1,
    SPINTERFERENCE_NOSIGNAL = 2,
    SPINTERFERENCE_TOOLOUD = 3,
    SPINTERFERENCE_TOOQUIET = 4,
    SPINTERFERENCE_TOOFAST = 5,
    SPINTERFERENCE_TOOSLOW = 6,
    SPINTERFERENCE_LATENCY_WARNING = 7,
    SPINTERFERENCE_LATENCY_TRUNCATE_BEGIN = 8,
    SPINTERFERENCE_LATENCY_TRUNCATE_END = 9,
}}
FLAGS!{enum SPENDSRSTREAMFLAGS {
    SPESF_NONE = 0,
    SPESF_STREAM_RELEASED = 1 << 0,
    SPESF_EMULATED = 1 << 1,
}}
FLAGS!{enum SPVFEATURE {
    SPVFEATURE_STRESSED = 1 << 0,
    SPVFEATURE_EMPHASIS = 1 << 1,
}}
ENUM!{enum SPVISEMES {
    SP_VISEME_0 = 0,
    SP_VISEME_1,
    SP_VISEME_2,
    SP_VISEME_3,
    SP_VISEME_4,
    SP_VISEME_5,
    SP_VISEME_6,
    SP_VISEME_7,
    SP_VISEME_8,
    SP_VISEME_9,
    SP_VISEME_10,
    SP_VISEME_11,
    SP_VISEME_12,
    SP_VISEME_13,
    SP_VISEME_14,
    SP_VISEME_15,
    SP_VISEME_16,
    SP_VISEME_17,
    SP_VISEME_18,
    SP_VISEME_19,
    SP_VISEME_20,
    SP_VISEME_21,
}}
STRUCT!{struct SPEVENTSOURCEINFO {
    ullEventInterest: ::ULONGLONG,
    ullQueuedInterest: ::ULONGLONG,
    ulCount: ::ULONG,
}}
RIDL!(
interface ISpEventSource(ISpEventSourceVtbl): ISpNotifySource(ISpNotifySourceVtbl) {
    fn SetInterest(
        &mut self, ullEventInterest: ::ULONGLONG, ullQueuedInterest: ::ULONGLONG
    ) -> ::HRESULT,
    fn GetEvents(
        &mut self, ulCount: ::ULONG, pEventArray: *mut SPEVENT, pulFetched: *mut ::ULONG
    ) -> ::HRESULT,
    fn GetInfo(&mut self, pInfo: *mut SPEVENTSOURCEINFO) -> ::HRESULT
}
);
RIDL!(
interface ISpEventSource2(ISpEventSource2Vtbl): ISpEventSource(ISpEventSourceVtbl) {
    fn GetEventsEx(
        &mut self, ulCount: ::ULONG, pEventArray: *mut SPEVENTEX, pulFetched: *mut ::ULONG
    ) -> ::HRESULT
}
);
RIDL!(
interface ISpEventSink(ISpEventSinkVtbl): IUnknown(IUnknownVtbl) {
    fn AddEvents(&mut self, pEventArray: *const SPEVENT, ulCount: ::ULONG) -> ::HRESULT,
    fn GetEventInterest(&mut self, pullEventInterest: *mut ::ULONGLONG) -> ::HRESULT
}
);
RIDL!(
interface ISpStreamFormat(ISpStreamFormatVtbl): IStream(IStreamVtbl) {
    fn GetFormat(
        &mut self, pguidFormatId: *mut ::GUID, ppCoMemWaveFormatEx: *mut *mut ::WAVEFORMATEX
    ) -> ::HRESULT
}
);
ENUM!{enum SPFILEMODE {
    SPFM_OPEN_READONLY = 0,
    SPFM_OPEN_READWRITE = 1,
    SPFM_CREATE = 2,
    SPFM_CREATE_ALWAYS = 3,
    SPFM_NUM_MODES = 4,
}}
RIDL!(
interface ISpStream(ISpStreamVtbl): ISpStreamFormat(ISpStreamFormatVtbl) {
    fn SetBaseStream(
        &mut self, pStream: *mut ::IStream, rguidFormat: ::REFGUID,
        pWaveFormatEx: *const ::WAVEFORMATEX
    ) -> ::HRESULT,
    fn GetBaseStream(&mut self, ppStream: *mut *mut ::IStream) -> ::HRESULT,
    fn BindToFile(
        &mut self, pszFileName: ::LPCWSTR, eMode: SPFILEMODE, pFormatId: *const ::GUID,
        pWaveFormatEx: *const ::WAVEFORMATEX, ullEventInterest: ::ULONGLONG
    ) -> ::HRESULT,
    fn Close(&mut self) -> ::HRESULT
}
);
RIDL!(
interface ISpStreamFormatConverter(ISpStreamFormatConverterVtbl)
    : ISpStreamFormat(ISpStreamFormatVtbl) {
    fn SetBaseStream(
        &mut self, pStream: *mut ISpStreamFormat, fSetFormatToBaseStreamFormat: ::BOOL,
        fWriteToBaseStream: ::BOOL
    ) -> ::HRESULT,
    fn GetBaseStream(&mut self, ppStream: *mut *mut ISpStreamFormat) -> ::HRESULT,
    fn SetFormat(
        &mut self, rguidFormatIdOfConvertedStream: ::REFGUID,
        pWaveFormatExOfConvertedStream: *const ::WAVEFORMATEX
    ) -> ::HRESULT,
    fn ResetSeekPosition(&mut self) -> ::HRESULT,
    fn ScaleConvertedToBaseOffset(
        &mut self, ullOffsetConvertedStream: ::ULONGLONG, pullOffsetBaseStream: *mut ::ULONGLONG
    ) -> ::HRESULT,
    fn ScaleBaseToConvertedOffset(
        &mut self, ullOffsetBaseStream: ::ULONGLONG, pullOffsetConvertedStream: *mut ::ULONGLONG
    ) -> ::HRESULT
}
);
ENUM!{enum SPAUDIOSTATE {
    SPAS_CLOSED = 0,
    SPAS_STOP = 1,
    SPAS_PAUSE = 2,
    SPAS_RUN = 3,
}}
STRUCT!{struct SPAUDIOSTATUS {
    cbFreeBuffSpace: ::LONG,
    cbNonBlockingIO: ::ULONG,
    State: SPAUDIOSTATE,
    CurSeekPos: ::ULONGLONG,
    CurDevicePos: ::ULONGLONG,
    dwAudioLevel: ::DWORD,
    dwReserved2: ::DWORD,
}}
STRUCT!{struct SPAUDIOBUFFERINFO {
    ulMsMinNotification: ::ULONG,
    ulMsBufferSize: ::ULONG,
    ulMsEventBias: ::ULONG,
}}
RIDL!(
interface ISpAudio(ISpAudioVtbl): ISpStreamFormat(ISpStreamFormatVtbl) {
    fn SetState(&mut self, NewState: SPAUDIOSTATE, ullReserved: ::ULONGLONG) -> ::HRESULT,
    fn SetFormat(
        &mut self, rguidFmtId: ::REFGUID, pWaveFormatEx: *const ::WAVEFORMATEX
    ) -> ::HRESULT,
    fn GetStatus(&mut self, pStatus: *mut SPAUDIOSTATUS) -> ::HRESULT,
    fn SetBufferInfo(&mut self, pBuffInfo: *const SPAUDIOBUFFERINFO) -> ::HRESULT,
    fn GetBufferInfo(&mut self, pBuffInfo: *mut SPAUDIOBUFFERINFO) -> ::HRESULT,
    fn GetDefaultFormat(
        &mut self, pFormatId: *mut ::GUID, ppCoMemWaveFormatEx: *mut *mut ::WAVEFORMATEX
    ) -> ::HRESULT,
    fn EventHandle(&mut self) -> ::HANDLE,
    fn GetVolumeLevel(&mut self, pLevel: *mut ::ULONG) -> ::HRESULT,
    fn SetVolumeLevel(&mut self, Level: ::ULONG) -> ::HRESULT,
    fn GetBufferNotifySize(&mut self, pcbSize: *mut ::ULONG) -> ::HRESULT,
    fn SetBufferNotifySize(&mut self, cbSize: ::ULONG) -> ::HRESULT
}
);
RIDL!(
interface ISpMMSysAudio(ISpMMSysAudioVtbl): ISpAudio(ISpAudioVtbl) {
    fn GetDeviceId(&mut self, puDeviceId: *mut ::UINT) -> ::HRESULT,
    fn SetDeviceId(&mut self, uDeviceId: ::UINT) -> ::HRESULT,
    fn GetMMHandle(&mut self, pHandle: *mut *mut ::c_void) -> ::HRESULT,
    fn GetLineId(&mut self, puLineId: *mut ::UINT) -> ::HRESULT,
    fn SetLineId(&mut self, uLineId: ::UINT) -> ::HRESULT
}
);
RIDL!(
interface ISpTranscript(ISpTranscriptVtbl): IUnknown(IUnknownVtbl) {
    fn GetTranscript(&mut self, ppszTranscript: *mut ::LPWSTR) -> ::HRESULT,
    fn AppendTranscript(&mut self, pszTranscript: ::LPCWSTR) -> ::HRESULT
}
);
FLAGS!{enum SPDISPLYATTRIBUTES {
    SPAF_ONE_TRAILING_SPACE = 0x2,
    SPAF_TWO_TRAILING_SPACES = 0x4,
    SPAF_CONSUME_LEADING_SPACES = 0x8,
    SPAF_BUFFER_POSITION = 0x10,
    SPAF_ALL = 0x1f,
    SPAF_USER_SPECIFIED = 0x80,
}}
pub type SPPHONEID = ::WCHAR;
pub type PSPPHONEID = ::LPWSTR;
pub type PCSPPHONEID = ::LPCWSTR;
STRUCT!{struct SPPHRASEELEMENT {
    ulAudioTimeOffset: ::ULONG,
    ulAudioSizeTime: ::ULONG,
    ulAudioStreamOffset: ::ULONG,
    ulAudioSizeBytes: ::ULONG,
    ulRetainedStreamOffset: ::ULONG,
    ulRetainedSizeBytes: ::ULONG,
    pszDisplayText: ::LPCWSTR,
    pszLexicalForm: ::LPCWSTR,
    pszPronunciation: *const SPPHONEID,
    bDisplayAttributes: ::BYTE,
    RequiredConfidence: ::c_char,
    ActualConfidence: ::c_char,
    Reserved: ::BYTE,
    SREngineConfidence: ::c_float,
}}
STRUCT!{struct SPPHRASERULE {
    pszName: ::LPCWSTR,
    ulId: ::ULONG,
    ulFirstElement: ::ULONG,
    ulCountOfElements: ::ULONG,
    pNextSibling: *const SPPHRASERULE,
    pFirstChild: *const SPPHRASERULE,
    SREngineConfidence: ::c_float,
    Confidence: ::c_char,
}}
ENUM!{enum SPPHRASEPROPERTYUNIONTYPE {
    SPPPUT_UNUSED = 0,
    SPPPUT_ARRAY_INDEX,
}}
STRUCT!{struct SPPHRASEPROPERTY {
    pszName: ::LPCWSTR,
    bType: ::BYTE,
    bReserved: ::BYTE,
    usArrayIndex: u16,
    pszValue: ::LPCWSTR,
    vValue: ::VARIANT,
    ulFirstElement: ::ULONG,
    ulCountOfElements: ::ULONG,
    pNextSibling: *const SPPHRASEPROPERTY,
    pFirstChild: *const SPPHRASEPROPERTY,
    SREngineConfidence: ::c_float,
    Confidence: ::c_char,
}}
UNION!(SPPHRASEPROPERTY, bType, ulId, ulId_mut, ::ULONG);
STRUCT!{struct SPPHRASEREPLACEMENT {
    bDisplayAttributes: ::BYTE,
    pszReplacementText: ::LPCWSTR,
    ulFirstElement: ::ULONG,
    ulCountOfElements: ::ULONG,
}}
STRUCT!{struct SPSEMANTICERRORINFO {
    ulLineNumber: ::ULONG,
    pszScriptLine: ::LPWSTR,
    pszSource: ::LPWSTR,
    pszDescription: ::LPWSTR,
    hrResultCode: ::HRESULT,
}}
ENUM!{enum SPSEMANTICFORMAT {
    SPSMF_SAPI_PROPERTIES = 0,
    SPSMF_SRGS_SEMANTICINTERPRETATION_MS = 1,
    SPSMF_SRGS_SAPIPROPERTIES = 2,
    SPSMF_UPS = 4,
    SPSMF_SRGS_SEMANTICINTERPRETATION_W3C = 8,
}}
STRUCT!{struct SPPHRASE_50 {
    cbSize: ::ULONG,
    LangID: ::WORD,
    wHomophoneGroupId: ::WORD,
    ullGrammarID: ::ULONGLONG,
    ftStartTime: ::ULONGLONG,
    ullAudioStreamPosition: ::ULONGLONG,
    ulAudioSizeBytes: ::ULONG,
    ulRetainedSizeBytes: ::ULONG,
    ulAudioSizeTime: ::ULONG,
    Rule: ::SPPHRASERULE,
    pProperties: *const ::SPPHRASEPROPERTY,
    pElements: *const ::SPPHRASEELEMENT,
    cReplacements: ::ULONG,
    pReplacements: *const ::SPPHRASEREPLACEMENT,
    SREngineID: ::GUID,
    ulSREnginePrivateDataSize: ::ULONG,
    pSREnginePrivateData: *const ::BYTE,
}}
STRUCT!{struct SPPHRASE_53 {
    cbSize: ::ULONG,
    LangID: ::WORD,
    wHomophoneGroupId: ::WORD,
    ullGrammarID: ::ULONGLONG,
    ftStartTime: ::ULONGLONG,
    ullAudioStreamPosition: ::ULONGLONG,
    ulAudioSizeBytes: ::ULONG,
    ulRetainedSizeBytes: ::ULONG,
    ulAudioSizeTime: ::ULONG,
    Rule: ::SPPHRASERULE,
    pProperties: *const ::SPPHRASEPROPERTY,
    pElements: *const ::SPPHRASEELEMENT,
    cReplacements: ::ULONG,
    pReplacements: *const ::SPPHRASEREPLACEMENT,
    SREngineID: ::GUID,
    ulSREnginePrivateDataSize: ::ULONG,
    pSREnginePrivateData: *const ::BYTE,
    pSML: ::LPWSTR,
    pSemanticErrorInfo: *mut SPSEMANTICERRORINFO,
}}
STRUCT!{struct SPPHRASE {
    cbSize: ::ULONG,
    LangID: ::WORD,
    wHomophoneGroupId: ::WORD,
    ullGrammarID: ::ULONGLONG,
    ftStartTime: ::ULONGLONG,
    ullAudioStreamPosition: ::ULONGLONG,
    ulAudioSizeBytes: ::ULONG,
    ulRetainedSizeBytes: ::ULONG,
    ulAudioSizeTime: ::ULONG,
    Rule: ::SPPHRASERULE,
    pProperties: *const ::SPPHRASEPROPERTY,
    pElements: *const ::SPPHRASEELEMENT,
    cReplacements: ::ULONG,
    pReplacements: *const ::SPPHRASEREPLACEMENT,
    SREngineID: ::GUID,
    ulSREnginePrivateDataSize: ::ULONG,
    pSREnginePrivateData: *const ::BYTE,
    pSML: ::LPWSTR,
    pSemanticErrorInfo: *mut SPSEMANTICERRORINFO,
    SemanticTagFormat: SPSEMANTICFORMAT,
}}
STRUCT!{struct SPSERIALIZEDPHRASE {
    ulSerializedSize: ::ULONG,
}}
STRUCT!{struct SPRULE {
    pszRuleName: ::LPCWSTR,
    ulRuleId: ::ULONG,
    dwAttributes: ::DWORD,
}}
FLAGS!{enum SPVALUETYPE {
    SPDF_PROPERTY = 0x1,
    SPDF_REPLACEMENT = 0x2,
    SPDF_RULE = 0x4,
    SPDF_DISPLAYTEXT = 0x8,
    SPDF_LEXICALFORM = 0x10,
    SPDF_PRONUNCIATION = 0x20,
    SPDF_AUDIO = 0x40,
    SPDF_ALTERNATES = 0x80,
    SPDF_ALL = 0xff,
}}
STRUCT!{struct SPBINARYGRAMMAR {
    ulTotalSerializedSize: ::ULONG,
}}
ENUM!{enum SPPHRASERNG {
    SPPR_ALL_ELEMENTS = -1i32 as u32,
}}
pub const SP_GETWHOLEPHRASE: SPPHRASERNG = SPPR_ALL_ELEMENTS;
pub const SPRR_ALL_ELEMENTS: SPPHRASERNG = SPPR_ALL_ELEMENTS;
DECLARE_HANDLE!(SPSTATEHANDLE, SPSTATEHANDLE__);
FLAGS!{enum SPRECOEVENTFLAGS {
    SPREF_AutoPause = 1 << 0,
    SPREF_Emulated = 1 << 1,
    SPREF_SMLTimeout = 1 << 2,
    SPREF_ExtendableParse = 1 << 3,
    SPREF_ReSent = 1 << 4,
    SPREF_Hypothesis = 1 << 5,
    SPREF_FalseRecognition = 1 << 6,
}}
ENUM!{enum SPPARTOFSPEECH {
    SPPS_NotOverriden = -1i32 as u32,
    SPPS_Unknown = 0,
    SPPS_Noun = 0x1000,
    SPPS_Verb = 0x2000,
    SPPS_Modifier = 0x3000,
    SPPS_Function = 0x4000,
    SPPS_Interjection = 0x5000,
    SPPS_Noncontent = 0x6000,
    SPPS_LMA = 0x7000,
    SPPS_SuppressWord = 0xf000,
}}
FLAGS!{enum SPLEXICONTYPE {
    eLEXTYPE_USER = 1 << 0,
    eLEXTYPE_APP = 1 << 1,
    eLEXTYPE_VENDORLEXICON = 1 << 2,
    eLEXTYPE_LETTERTOSOUND = 1 << 3,
    eLEXTYPE_MORPHOLOGY = 1 << 4,
    eLEXTYPE_RESERVED4 = 1 << 5,
    eLEXTYPE_USER_SHORTCUT = 1 << 6,
    eLEXTYPE_RESERVED6 = 1 << 7,
    eLEXTYPE_RESERVED7 = 1 << 8,
    eLEXTYPE_RESERVED8 = 1 << 9,
    eLEXTYPE_RESERVED9 = 1 << 10,
    eLEXTYPE_RESERVED10 = 1 << 11,
    eLEXTYPE_PRIVATE1 = 1 << 12,
    eLEXTYPE_PRIVATE2 = 1 << 13,
    eLEXTYPE_PRIVATE3 = 1 << 14,
    eLEXTYPE_PRIVATE4 = 1 << 15,
    eLEXTYPE_PRIVATE5 = 1 << 16,
    eLEXTYPE_PRIVATE6 = 1 << 17,
    eLEXTYPE_PRIVATE7 = 1 << 18,
    eLEXTYPE_PRIVATE8 = 1 << 19,
    eLEXTYPE_PRIVATE9 = 1 << 20,
    eLEXTYPE_PRIVATE10 = 1 << 21,
    eLEXTYPE_PRIVATE11 = 1 << 22,
    eLEXTYPE_PRIVATE12 = 1 << 23,
    eLEXTYPE_PRIVATE13 = 1 << 24,
    eLEXTYPE_PRIVATE14 = 1 << 25,
    eLEXTYPE_PRIVATE15 = 1 << 26,
    eLEXTYPE_PRIVATE16 = 1 << 27,
    eLEXTYPE_PRIVATE17 = 1 << 28,
    eLEXTYPE_PRIVATE18 = 1 << 29,
    eLEXTYPE_PRIVATE19 = 1 << 30,
    eLEXTYPE_PRIVATE20 = 1 << 31,
}}
FLAGS!{enum SPWORDTYPE {
    eWORDTYPE_ADDED = 1 << 0,
    eWORDTYPE_DELETED = 1 << 1,
}}
FLAGS!{enum SPPRONUNCIATIONFLAGS {
    ePRONFLAG_USED = 1 << 0,
}}
STRUCT!{struct SPWORDPRONUNCIATION {
    pNextWordPronunciation: *mut SPWORDPRONUNCIATION,
    eLexiconType: SPLEXICONTYPE,
    LangID: ::WORD,
    wPronunciationFlags: ::WORD,
    ePartOfSpeech: SPPARTOFSPEECH,
    szPronunciation: [SPPHONEID; 1],
}}
STRUCT!{struct SPWORDPRONUNCIATIONLIST {
    ulSize: ::ULONG,
    pvBuffer: *mut ::BYTE,
    pFirstWordPronunciation: *mut SPWORDPRONUNCIATION,
}}
STRUCT!{struct SPWORD {
    pNextWord: *mut SPWORD,
    LangID: ::WORD,
    wReserved: ::WORD,
    eWordType: SPWORDTYPE,
    pszWord: ::LPWSTR,
    pFirstWordPronunciation: *mut SPWORDPRONUNCIATION,
}}
STRUCT!{struct SPWORDLIST {
    ulSize: ::ULONG,
    pvBuffer: *mut ::BYTE,
    pFirstWord: *mut SPWORD,
}}
RIDL!(
interface ISpLexicon(ISpLexiconVtbl): IUnknown(IUnknownVtbl) {
    fn GetPronunciations(
        &mut self, pszWord: ::LPCWSTR, LangID: ::WORD, dwFlags: ::DWORD,
        pWordPronunciationList: *mut SPWORDPRONUNCIATIONLIST
    ) -> ::HRESULT,
    fn AddPronunciation(
        &mut self, pszWord: ::LPCWSTR, LangID: ::WORD, ePartOfSpeech: SPPARTOFSPEECH,
        pszPronunciation: PCSPPHONEID
    ) -> ::HRESULT,
    fn RemovePronunciation(
        &mut self, pszWord: ::LPCWSTR, LangID: ::WORD, ePartOfSpeech: SPPARTOFSPEECH,
        pszPronunciation: PCSPPHONEID
    ) -> ::HRESULT,
    fn GetGeneration(&mut self, pdwGeneration: *mut ::DWORD) -> ::HRESULT,
    fn GetGenerationChange(
        &mut self, dwFlags: ::DWORD, pdwGeneration: *mut ::DWORD, pWordList: *mut SPWORDLIST
    ) -> ::HRESULT,
    fn GetWords(
        &mut self, dwFlags: ::DWORD, pdwGeneration: *mut ::DWORD, pdwCookie: *mut ::DWORD,
        pWordList: *mut SPWORDLIST
    ) -> ::HRESULT
}
);
RIDL!(
interface ISpContainerLexicon(ISpContainerLexiconVtbl): ISpLexicon(ISpLexiconVtbl) {
    fn AddLexicon(&mut self, pAddLexicon: *mut ISpLexicon, dwFlags: ::DWORD) -> ::HRESULT
}
);
ENUM!{enum SPSHORTCUTTYPE {
    SPSHT_NotOverriden = -1i32 as u32,
    SPSHT_Unknown = 0,
    SPSHT_EMAIL = 0x1000,
    SPSHT_OTHER = 0x2000,
    SPPS_RESERVED1 = 0x3000,
    SPPS_RESERVED2 = 0x4000,
    SPPS_RESERVED3 = 0x5000,
    SPPS_RESERVED4 = 0xf000,
}}
STRUCT!{struct SPSHORTCUTPAIR {
    pNextSHORTCUTPAIR: *mut SPSHORTCUTPAIR,
    LangID: ::WORD,
    shType: SPSHORTCUTTYPE,
    pszDisplay: ::LPWSTR,
    pszSpoken: ::LPWSTR,
}}
STRUCT!{struct SPSHORTCUTPAIRLIST {
    ulSize: ::ULONG,
    pvBuffer: *mut ::BYTE,
    pFirstShortcutPair: *mut SPSHORTCUTPAIR,
}}
RIDL!(
interface ISpShortcut(ISpShortcutVtbl): IUnknown(IUnknownVtbl) {
    fn AddShortcut(
        &mut self, pszDisplay: ::LPCWSTR, LangID: ::WORD, pszSpoken: ::LPCWSTR,
        shType: SPSHORTCUTTYPE
    ) -> ::HRESULT,
    fn RemoveShortcut(
        &mut self, pszDisplay: ::LPCWSTR, LangID: ::WORD, pszSpoken: ::LPCWSTR,
        shType: SPSHORTCUTTYPE
    ) -> ::HRESULT,
    fn GetShortcuts(
        &mut self, LangId: ::WORD, pShortcutpairList: *mut SPSHORTCUTPAIRLIST
    ) -> ::HRESULT,
    fn GetGeneration(&mut self, pdwGeneration: *mut ::DWORD) -> ::HRESULT,
    fn GetWordsFromGenerationChange(
        &mut self, pdwGeneration: *mut ::DWORD, pWordList: *mut SPWORDLIST
    ) -> ::HRESULT,
    fn GetWords(
        &mut self, pdwGeneration: *mut ::DWORD, pdwCookie: *mut ::DWORD, pWordList: *mut SPWORDLIST
    ) -> ::HRESULT,
    fn GetShortcutsForGeneration(
        &mut self, pdwGeneration: *mut ::DWORD, pdwCookie: *mut ::DWORD,
        pShortcutpairList: *mut SPSHORTCUTPAIRLIST
    ) -> ::HRESULT,
    fn GetGenerationChange(
        &mut self, pdwGeneration: *mut ::DWORD, pShortcutpairList: *mut SPSHORTCUTPAIRLIST
    ) -> ::HRESULT
}
);
RIDL!(
interface ISpPhoneConverter(ISpPhoneConverterVtbl): ISpObjectWithToken(ISpObjectWithTokenVtbl) {
    fn PhoneToId(&mut self, pszPhone: ::LPCWSTR, pId: *mut SPPHONEID) -> ::HRESULT,
    fn IdToPhone(&mut self, pId: PCSPPHONEID, pszPhone: *mut ::WCHAR) -> ::HRESULT
}
);
RIDL!(
interface ISpPhoneticAlphabetConverter(ISpPhoneticAlphabetConverterVtbl): IUnknown(IUnknownVtbl) {
    fn GetLangId(&mut self, pLangID: *mut ::WORD) -> ::HRESULT,
    fn SetLangId(&mut self, LangID: *mut ::WORD) -> ::HRESULT,
    fn SAPI2UPS(
        &mut self, pszSAPIId: *const SPPHONEID, pszUPSId: *mut SPPHONEID, cMaxLength: ::DWORD
    ) -> ::HRESULT,
    fn UPS2SAPI(
        &mut self, pszUPSId: *const SPPHONEID, pszSAPIId: *mut SPPHONEID, cMaxLength: ::DWORD
    ) -> ::HRESULT,
    fn GetMaxConvertLength(
        &mut self, cSrcLength: ::DWORD, bSAPI2UPS: ::BOOL, pcMaxDestLength: *mut ::DWORD
    ) -> ::HRESULT
}
);
RIDL!(
interface ISpPhoneticAlphabetSelection(ISpPhoneticAlphabetSelectionVtbl): IUnknown(IUnknownVtbl) {
    fn IsAlphabetUPS(&mut self, pfIsUPS: *mut ::BOOL) -> ::HRESULT,
    fn SetAlphabetToUPS(&mut self, fForceUPS: ::BOOL) -> ::HRESULT
}
);
STRUCT!{struct SPVPITCH {
    MiddleAdj: ::c_long,
    RangeAdj: ::c_long,
}}
ENUM!{enum SPVACTIONS {
    SPVA_Speak = 0,
    SPVA_Silence,
    SPVA_Pronounce,
    SPVA_Bookmark,
    SPVA_SpellOut,
    SPVA_Section,
    SPVA_ParseUnknownTag,
}}
STRUCT!{struct SPVCONTEXT {
    pCategory: ::LPCWSTR,
    pBefore: ::LPCWSTR,
    pAfter: ::LPCWSTR,
}}
STRUCT!{struct SPVSTATE {
    eAction: SPVACTIONS,
    LangID: ::WORD,
    wReserved: ::WORD,
    EmphAdj: ::c_long,
    RateAdj: ::c_long,
    Volume: ::ULONG,
    PitchAdj: SPVPITCH,
    SilenceMSecs: ::ULONG,
    pPhoneIds: *mut SPPHONEID,
    ePartOfSpeech: SPPARTOFSPEECH,
    Context: SPVCONTEXT,
}}
ENUM!{enum SPRUNSTATE {
    SPRS_DONE = 1 << 0,
    SPRS_IS_SPEAKING = 1 << 1,
}}
ENUM!{enum SPVLIMITS {
    SPMIN_VOLUME = 0,
    SPMAX_VOLUME = 100,
    SPMIN_RATE = -10i32 as u32,
    SPMAX_RATE = 10,
}}
ENUM!{enum SPVPRIORITY {
    SPVPRI_NORMAL = 0,
    SPVPRI_ALERT = 1 << 0,
    SPVPRI_OVER = 1 << 1,
}}
STRUCT!{struct SPVOICESTATUS {
    ulCurrentStream: ::ULONG,
    ulLastStreamQueued: ::ULONG,
    hrLastResult: ::HRESULT,
    dwRunningState: ::DWORD,
    ulInputWordPos: ::ULONG,
    ulInputWordLen: ::ULONG,
    ulInputSentPos: ::ULONG,
    ulInputSentLen: ::ULONG,
    lBookmarkId: ::LONG,
    PhonemeId: SPPHONEID,
    VisemeId: SPVISEMES,
    dwReserved1: ::DWORD,
    dwReserved2: ::DWORD,
}}
FLAGS!{enum SPEAKFLAGS {
    SPF_DEFAULT = 0,
    SPF_ASYNC = 1 << 0,
    SPF_PURGEBEFORESPEAK = 1 << 1,
    SPF_IS_FILENAME = 1 << 2,
    SPF_IS_XML = 1 << 3,
    SPF_IS_NOT_XML = 1 << 4,
    SPF_PERSIST_XML = 1 << 5,
    SPF_NLP_SPEAK_PUNC = 1 << 6,
    SPF_PARSE_SAPI = 1 << 7,
    SPF_PARSE_SSML = 1 << 8,
}}
pub const SPF_PARSE_AUTODETECT: SPEAKFLAGS = SPF_DEFAULT;
pub const SPF_NLP_MASK: SPEAKFLAGS = SPF_NLP_SPEAK_PUNC;
pub const SPF_PARSE_MASK: i32 = SPF_PARSE_SAPI.0 as i32 | SPF_PARSE_SSML.0 as i32;
pub const SPF_VOICE_MASK: i32 =
    SPF_ASYNC.0 as i32 | SPF_PURGEBEFORESPEAK.0 as i32 | SPF_IS_FILENAME.0 as i32 | SPF_IS_XML.0 as i32 |
    SPF_IS_NOT_XML.0 as i32 | SPF_NLP_MASK.0 as i32 | SPF_PERSIST_XML.0 as i32 | SPF_PARSE_MASK;
pub const SPF_UNUSED_FLAGS: i32 = !SPF_VOICE_MASK;
RIDL!(
interface ISpVoice(ISpVoiceVtbl): ISpEventSource(ISpEventSourceVtbl) {
    fn SetOutput(&mut self, pUnkOutput: *mut ::IUnknown, fAllowFormatChanges: ::BOOL) -> ::HRESULT,
    fn GetOutputObjectToken(&mut self, ppObjectToken: *mut *mut ISpObjectToken) -> ::HRESULT,
    fn GetOutputStream(&mut self, ppStream: *mut *mut ISpStreamFormat) -> ::HRESULT,
    fn Pause(&mut self) -> ::HRESULT,
    fn Resume(&mut self) -> ::HRESULT,
    fn SetVoice(&mut self, pToken: *mut ISpObjectToken) -> ::HRESULT,
    fn GetVoice(&mut self, ppToken: *mut *mut ISpObjectToken) -> ::HRESULT,
    fn Speak(
        &mut self, pwcs: ::LPCWSTR, dwFlags: ::DWORD, pulStreamNumber: *mut ::ULONG
    ) -> ::HRESULT,
    fn SpeakStream(
        &mut self, pStream: *mut ::IStream, dwFlags: ::DWORD, pulStreamNumber: *mut ::ULONG
    ) -> ::HRESULT,
    fn GetStatus(
        &mut self, pStatus: *mut SPVOICESTATUS, ppszLastBookmark: *mut ::LPWSTR
    ) -> ::HRESULT,
    fn Skip(
        &mut self, pItemType: ::LPCWSTR, lNumItems: ::c_long, pulNumSkipped: *mut ::ULONG
    ) -> ::HRESULT,
    fn SetPriority(&mut self, ePriority: SPVPRIORITY) -> ::HRESULT,
    fn GetPriority(&mut self, pePriority: *mut SPVPRIORITY) -> ::HRESULT,
    fn SetAlertBoundary(&mut self, eBoundary: SPEVENTENUM) -> ::HRESULT,
    fn GetAlertBoundary(&mut self, peBoundary: *mut SPEVENTENUM) -> ::HRESULT,
    fn SetRate(&mut self, RateAdjust: ::c_long) -> ::HRESULT,
    fn GetRate(&mut self, pRateAdjust: *mut ::c_long) -> ::HRESULT,
    fn SetVolume(&mut self, usVolume: ::USHORT) -> ::HRESULT,
    fn GetVolume(&mut self, pusVolume: *mut ::USHORT) -> ::HRESULT,
    fn WaitUntilDone(&mut self, msTimeout: ::ULONG) -> ::HRESULT,
    fn SetSyncSpeakTimeout(&mut self, msTimeout: ::ULONG) -> ::HRESULT,
    fn GetSyncSpeakTimeout(&mut self, pmsTimeout: *mut ::ULONG) -> ::HRESULT,
    fn SpeakCompleteEvent(&mut self) -> ::HANDLE,
    fn IsUISupported(
        &mut self, pszTypeOfUI: ::LPCWSTR, pvExtraData: *mut ::c_void, cbExtraData: ::ULONG,
        pfSupported: *mut ::BOOL
    ) -> ::HRESULT,
    fn DisplayUI(
        &mut self, hwndParent: ::HWND, pszTitle: ::LPCWSTR, pszTypeOfUI: ::LPCWSTR,
        pvExtraData: *mut ::c_void, cbExtraData: ::ULONG
    ) -> ::HRESULT
}
);
DEFINE_GUID!(
    UuidOfISpVoice,
    0x6C44DF74, 0x72B9, 0x4992, 0xA1, 0xEC, 0xEF, 0x99, 0x6E, 0x04, 0x22, 0xD4
);
RIDL!(
interface ISpPhrase(ISpPhraseVtbl): IUnknown(IUnknownVtbl) {
    fn GetPhrase(&mut self, ppCoMemPhrase: *mut *mut SPPHRASE) -> ::HRESULT,
    fn GetSerializedPhrase(&mut self, ppCoMemPhrase: *mut *mut SPSERIALIZEDPHRASE) -> ::HRESULT,
    fn GetText(
        &mut self, ulStart: ::ULONG, ulCount: ::ULONG, fUseTextReplacements: ::BOOL,
        ppszCoMemText: *mut ::LPWSTR, pbDisplayAttributes: *mut ::BYTE
    ) -> ::HRESULT,
    fn Discard(&mut self, dwValueTypes: ::DWORD) -> ::HRESULT
}
);
RIDL!(
interface ISpPhraseAlt(ISpPhraseAltVtbl): ISpPhrase(ISpPhraseVtbl) {
    fn GetAltInfo(
        &mut self, pParent: *mut *mut ISpPhrase, pulStartElementInParent: *mut ::ULONG,
        pcElementsInParent: *mut ::ULONG, pcElementsInAlt: *mut ::ULONG
    ) -> ::HRESULT,
    fn Commit(&mut self) -> ::HRESULT
}
);
ENUM!{enum SPXMLRESULTOPTIONS {
    SPXRO_SML = 0,
    SPXRO_Alternates_SML = 1,
}}
RIDL!(
interface ISpPhrase2(ISpPhrase2Vtbl): ISpPhrase(ISpPhraseVtbl) {
    fn GetXMLResult(
        &mut self, ppszCoMemXMLResult: *mut ::LPWSTR, Options: SPXMLRESULTOPTIONS
    ) -> ::HRESULT,
    fn GetXMLErrorInfo(&mut self, pSemanticErrorInfo: *mut SPSEMANTICERRORINFO) -> ::HRESULT,
    fn GetAudio(
        &mut self, ulStartElement: ::ULONG, cElements: ::ULONG, ppStream: *mut *mut ISpStreamFormat
    ) -> ::HRESULT
}
);
STRUCT!{struct SPRECORESULTTIMES {
    ftStreamTime: ::FILETIME,
    ullLength: ::ULONGLONG,
    dwTickCount: ::DWORD,
    ullStart: ::ULONGLONG,
}}
STRUCT!{struct SPSERIALIZEDRESULT {
    ulSerializedSize: ::ULONG,
}}
RIDL!(
interface ISpRecoResult(ISpRecoResultVtbl): ISpPhrase(ISpPhraseVtbl) {
    fn GetResultTimes(&mut self, pTimes: *mut SPRECORESULTTIMES) -> ::HRESULT,
    fn GetAlternates(
        &mut self, ulStartElement: ::ULONG, cElements: ::ULONG, ulRequestCount: ::ULONG,
        ppPhrases: *mut *mut ISpPhraseAlt, pcPhrasesReturned: *mut ::ULONG
    ) -> ::HRESULT,
    fn GetAudio(
        &mut self, ulStartElement: ::ULONG, cElements: ::ULONG, ppStream: *mut *mut ISpStreamFormat
    ) -> ::HRESULT,
    fn SpeakAudio(
        &mut self, ulStartElement: ::ULONG, cElements: ::ULONG, dwFlags: ::DWORD,
        pulStreamNumber: *mut ::ULONG
    ) -> ::HRESULT,
    fn Serialize(&mut self, ppCoMemSerializedResult: *mut *mut SPSERIALIZEDRESULT) -> ::HRESULT,
    fn ScaleAudio(
        &mut self, pAudioFormatId: *const ::GUID, pWaveFormatEx: *const ::WAVEFORMATEX
    ) -> ::HRESULT,
    fn GetRecoContext(&mut self, ppRecoContext: *mut *mut ISpRecoContext) -> ::HRESULT
}
);
FLAGS!{enum SPCOMMITFLAGS {
    SPCF_NONE = 0,
    SPCF_ADD_TO_USER_LEXICON = 1 << 0,
    SPCF_DEFINITE_CORRECTION = 1 << 1,
}}
RIDL!(
interface ISpRecoResult2(ISpRecoResult2Vtbl): ISpRecoResult(ISpRecoResultVtbl) {
    fn CommitAlternate(
        &mut self, pPhraseAlt: *mut ISpPhraseAlt, ppNewResult: *mut *mut ISpRecoResult
    ) -> ::HRESULT,
    fn CommitText(
        &mut self, ulStartElement: ::ULONG, cElements: ::ULONG, pszCorrectedData: ::LPCWSTR,
        eCommitFlags: ::DWORD
    ) -> ::HRESULT,
    fn SetTextFeedback(&mut self, pszFeedback: ::LPCWSTR, fSuccessful: ::BOOL) -> ::HRESULT
}
);
RIDL!(
interface ISpXMLRecoResult(ISpXMLRecoResultVtbl): ISpRecoResult(ISpRecoResultVtbl) {
    fn GetXMLResult(
        &mut self, ppszCoMemXMLResult: *mut ::LPWSTR, Options: SPXMLRESULTOPTIONS
    ) -> ::HRESULT,
    fn GetXMLErrorInfo(&mut self, pSemanticErrorInfo: *mut SPSEMANTICERRORINFO) -> ::HRESULT
}
);
STRUCT!{struct SPTEXTSELECTIONINFO {
    ulStartActiveOffset: ::ULONG,
    cchActiveChars: ::ULONG,
    ulStartSelection: ::ULONG,
    cchSelection: ::ULONG,
}}
ENUM!{enum SPWORDPRONOUNCEABLE {
    SPWP_UNKNOWN_WORD_UNPRONOUNCEABLE = 0,
    SPWP_UNKNOWN_WORD_PRONOUNCEABLE = 1,
    SPWP_KNOWN_WORD_PRONOUNCEABLE = 2,
}}
ENUM!{enum SPGRAMMARSTATE {
    SPGS_DISABLED = 0,
    SPGS_ENABLED = 1,
    SPGS_EXCLUSIVE = 3,
}}
ENUM!{enum SPCONTEXTSTATE {
    SPCS_DISABLED = 0,
    SPCS_ENABLED = 1,
}}
ENUM!{enum SPRULESTATE {
    SPRS_INACTIVE = 0,
    SPRS_ACTIVE = 1,
    SPRS_ACTIVE_WITH_AUTO_PAUSE = 3,
    SPRS_ACTIVE_USER_DELIMITED = 4,
}}
pub const SP_STREAMPOS_ASAP: ::INT = 0;
pub const SP_STREAMPOS_REALTIME: ::INT = -1;
pub const SPRULETRANS_TEXTBUFFER: SPSTATEHANDLE = -1isize as SPSTATEHANDLE;
pub const SPRULETRANS_WILDCARD: SPSTATEHANDLE = -2isize as SPSTATEHANDLE;
pub const SPRULETRANS_DICTATION: SPSTATEHANDLE = -3isize as SPSTATEHANDLE;
ENUM!{enum SPGRAMMARWORDTYPE {
    SPWT_DISPLAY = 0,
    SPWT_LEXICAL = 1,
    SPWT_PRONUNCIATION = 2,
    SPWT_LEXICAL_NO_SPECIAL_CHARS = 3,
}}
STRUCT!{struct SPPROPERTYINFO {
    pszName: ::LPCWSTR,
    ulId: ::ULONG,
    pszValue: ::LPCWSTR,
    vValue: ::VARIANT,
}}
FLAGS!{enum SPCFGRULEATTRIBUTES {
    SPRAF_TopLevel = 1 << 0,
    SPRAF_Active = 1 << 1,
    SPRAF_Export = 1 << 2,
    SPRAF_Import = 1 << 3,
    SPRAF_Interpreter = 1 << 4,
    SPRAF_Dynamic = 1 << 5,
    SPRAF_Root = 1 << 6,
    SPRAF_AutoPause = 1 << 16,
    SPRAF_UserDelimited = 1 << 17,
}}
RIDL!(
interface ISpGrammarBuilder(ISpGrammarBuilderVtbl): IUnknown(IUnknownVtbl) {
    fn ResetGrammar(&mut self, NewLanguage: ::WORD) -> ::HRESULT,
    fn GetRule(
        &mut self, pszRuleName: ::LPCWSTR, dwRuleId: ::DWORD, dwAttributes: ::DWORD,
        fCreateIfNotExist: ::BOOL, phInitialState: *mut SPSTATEHANDLE
    ) -> ::HRESULT,
    fn ClearRule(&mut self, hState: SPSTATEHANDLE) -> ::HRESULT,
    fn CreateNewState(&mut self, hState: SPSTATEHANDLE, phState: *mut SPSTATEHANDLE) -> ::HRESULT,
    fn AddWordTransition(
        &mut self, hFromState: SPSTATEHANDLE, hToState: SPSTATEHANDLE, psz: ::LPCWSTR,
        pszSeparators: ::LPCWSTR, eWordType: SPGRAMMARWORDTYPE, Weight: ::c_float,
        pPropInfo: *const SPPROPERTYINFO
    ) -> ::HRESULT,
    fn AddRuleTransition(
        &mut self, hFromState: SPSTATEHANDLE, hToState: SPSTATEHANDLE, hRule: SPSTATEHANDLE,
        Weight: ::c_float, pPropInfo: *const SPPROPERTYINFO
    ) -> ::HRESULT,
    fn AddResource(
        &mut self, hRuleState: SPSTATEHANDLE, pszResourceName: ::LPCWSTR,
        pszResourceValue: ::LPCWSTR
    ) -> ::HRESULT,
    fn Commit(&mut self, dwReserved: ::DWORD) -> ::HRESULT
}
);
ENUM!{enum SPLOADOPTIONS {
    SPLO_STATIC = 0,
    SPLO_DYNAMIC = 1,
}}
RIDL!(
interface ISpRecoGrammar(ISpRecoGrammarVtbl): ISpGrammarBuilder(ISpGrammarBuilderVtbl) {
    fn GetGrammarId(&mut self, pullGrammarId: *mut ::ULONGLONG) -> ::HRESULT,
    fn GetRecoContext(&mut self, ppRecoCtxt: *mut *mut ISpRecoContext) -> ::HRESULT,
    fn LoadCmdFromFile(&mut self, pszFileName: ::LPCWSTR, Options: SPLOADOPTIONS) -> ::HRESULT,
    fn LoadCmdFromObject(
        &mut self, rcid: ::REFCLSID, pszGrammarName: ::LPCWSTR, Options: SPLOADOPTIONS
    ) -> ::HRESULT,
    fn LoadCmdFromResource(
        &mut self, hModule: ::HMODULE, pszResourceName: ::LPCWSTR, pszResourceType: ::LPCWSTR,
        wLanguage: ::WORD, Options: SPLOADOPTIONS
    ) -> ::HRESULT,
    fn LoadCmdFromMemory(
        &mut self, pGrammar: *const SPBINARYGRAMMAR, Options: SPLOADOPTIONS
    ) -> ::HRESULT,
    fn LoadCmdFromProprietaryGrammar(
        &mut self, rguidParam: ::REFGUID, pszStringParam: ::LPCWSTR, pvDataPrarm: *const ::c_void,
        cbDataSize: ::ULONG, Options: SPLOADOPTIONS
    ) -> ::HRESULT,
    fn SetRuleState(
        &mut self, pszName: ::LPCWSTR, pReserved: *mut ::c_void, NewState: SPRULESTATE
    ) -> ::HRESULT,
    fn SetRuleIdState(&mut self, ulRuleId: ::ULONG, NewState: SPRULESTATE) -> ::HRESULT,
    fn LoadDictation(&mut self, pszTopicName: ::LPCWSTR, Options: SPLOADOPTIONS) -> ::HRESULT,
    fn UnloadDictation(&mut self) -> ::HRESULT,
    fn SetDictationState(&mut self, NewState: SPRULESTATE) -> ::HRESULT,
    fn SetWordSequenceData(
        &mut self, pText: *const ::WCHAR, cchText: ::ULONG, pInfo: *const SPTEXTSELECTIONINFO
    ) -> ::HRESULT,
    fn SetTextSelection(&mut self, pInfo: *const SPTEXTSELECTIONINFO) -> ::HRESULT,
    fn IsPronounceable(
        &mut self, pszWord: ::LPCWSTR, pWordPronounceable: *mut SPWORDPRONOUNCEABLE
    ) -> ::HRESULT,
    fn SetGrammarState(&mut self, eGrammarState: SPGRAMMARSTATE) -> ::HRESULT,
    fn SaveCmd(&mut self, pStream: *mut ::IStream, ppszCoMemErrorText: *mut ::LPWSTR) -> ::HRESULT,
    fn GetGrammarState(&mut self, peGrammarState: *mut SPGRAMMARSTATE) -> ::HRESULT
}
);
ENUM!{enum SPMATCHINGMODE {
    AllWords = 0,
    Subsequence = 1,
    OrderedSubset = 3,
    SubsequenceContentRequired = 5,
    OrderedSubsetContentRequired = 7,
}}
ENUM!{enum PHONETICALPHABET {
    PA_Ipa = 0,
    PA_Ups = 1,
    PA_Sapi = 2,
}}
RIDL!(
interface ISpGrammarBuilder2(ISpGrammarBuilder2Vtbl): IUnknown(IUnknownVtbl) {
    fn AddTextSubset(
        &mut self, hFromState: SPSTATEHANDLE, hToState: SPSTATEHANDLE, psz: ::LPCWSTR,
        eMatchMode: SPMATCHINGMODE
    ) -> ::HRESULT,
    fn SetPhoneticAlphabet(&mut self, phoneticALphabet: PHONETICALPHABET) -> ::HRESULT
}
);
RIDL!(
interface ISpRecoGrammar2(ISpRecoGrammar2Vtbl): IUnknown(IUnknownVtbl) {
    fn GetRules(&mut self, ppCoMemRules: *mut *mut SPRULE, puNumRules: *mut ::UINT) -> ::HRESULT,
    fn LoadCmdFromFile2(
        &mut self, pszFileName: ::LPCWSTR, Options: SPLOADOPTIONS, pszSharingUri: ::LPCWSTR,
        pszBaseUri: ::LPCWSTR
    ) -> ::HRESULT,
    fn LoadCmdFromMemory2(
        &mut self, pGrammar: *const SPBINARYGRAMMAR, Options: SPLOADOPTIONS,
        pszSharingUri: ::LPCWSTR, pszBaseUri: ::LPCWSTR
    ) -> ::HRESULT,
    fn SetRulePriority(
        &mut self, pszRuleName: ::LPCWSTR, ulRuleId: ::ULONG, nRulePriority: ::c_int
    ) -> ::HRESULT,
    fn SetRuleWeight(
        &mut self, pszRuleName: ::LPCWSTR, ulRuleId: ::ULONG, flWeight: ::c_float
    ) -> ::HRESULT,
    fn SetDictationWeight(&mut self, flWeight: ::c_float) -> ::HRESULT,
    fn SetGrammarLoader(&mut self, pLoader: *mut ISpeechResourceLoader) -> ::HRESULT,
    fn SetSMLSecurityManager(
        &mut self, pSMLSecurityManager: *mut ::IInternetSecurityManager
    ) -> ::HRESULT
}
);
RIDL!(
interface ISpeechResourceLoader(ISpeechResourceLoaderVtbl): IDispatch(IDispatchVtbl) {
    fn LoadResource(
        &mut self, bstrResourceUri: ::BSTR, fAlwaysReload: ::VARIANT_BOOL,
        pStream: *mut *mut ::IUnknown, pbstrMIMEType: *mut ::BSTR, pfModified: *mut ::VARIANT_BOOL,
        pbstrRedirectUrl: *mut ::BSTR
    ) -> ::HRESULT,
    fn GetLocalCopy(
        &mut self, bstrResourceUri: ::BSTR, pbstrLocalPath: *mut ::BSTR,
        pbstrMIMEType: *mut ::BSTR, pbstrRedirectUrl: *mut ::BSTR
    ) -> ::HRESULT,
    fn ReleaseLocalCopy(&mut self, pbstrLocalPath: ::BSTR) -> ::HRESULT
}
);
STRUCT!{nodebug struct SPRECOCONTEXTSTATUS {
    eInterference: SPINTERFERENCE,
    szRequestTypeOfUI: [::WCHAR; 255],
    dwReserved1: ::DWORD,
    dwReserved2: ::DWORD,
}}
FLAGS!{enum SPBOOKMARKOPTIONS {
    SPBO_NONE = 0,
    SPBO_PAUSE = 1 << 0,
    SPBO_AHEAD = 1 << 1,
    SPBO_TIME_UNITS = 1 << 2,
}}
FLAGS!{enum SPAUDIOOPTIONS {
    SPAO_NONE = 0,
    SPAO_RETAIN_AUDIO = 1 << 0,
}}
RIDL!(
interface ISpRecoContext(ISpRecoContextVtbl): ISpEventSource(ISpEventSourceVtbl) {
    fn GetRecognizer(&mut self, ppRecognizer: *mut *mut ISpRecognizer) -> ::HRESULT,
    fn CreateGrammer(
        &mut self, ullGrammarId: ::ULONGLONG, ppGrammar: *mut *mut ISpRecoGrammar
    ) -> ::HRESULT,
    fn GetStatus(&mut self, pState: *mut SPRECOCONTEXTSTATUS) -> ::HRESULT,
    fn GetMaxAlternates(&mut self, pcAlternates: *mut ::ULONG) -> ::HRESULT,
    fn SetMaxAlternates(&mut self, cAlternates: ::ULONG) -> ::HRESULT,
    fn SetAudioOptions(
        &mut self, Options: SPAUDIOOPTIONS, pAudioFormatId: *const ::GUID,
        pWaveFormatEx: *const ::WAVEFORMATEX
    ) -> ::HRESULT,
    fn GetAudioOptions(
        &mut self, pOptions: *mut SPAUDIOOPTIONS, pAudioFormatId: *mut ::GUID,
        ppCoMemWFEX: *mut *mut ::WAVEFORMATEX
    ) -> ::HRESULT,
    fn DeserializeResult(
        &mut self, pSerializedResult: *const SPSERIALIZEDRESULT, ppResult: *mut *mut ISpRecoResult
    ) -> ::HRESULT,
    fn Bookmark(
        &mut self, Options: SPBOOKMARKOPTIONS, ullStreamPosition: ::ULONGLONG,
        lparamEvent: ::LPARAM
    ) -> ::HRESULT,
    fn SetAdaptionData(&mut self, pAdaptionData: ::LPCWSTR, cch: ::ULONG) -> ::HRESULT,
    fn Pause(&mut self, dwReserved: ::DWORD) -> ::HRESULT,
    fn Resume(&mut self, dwReserved: ::DWORD) -> ::HRESULT,
    fn SetVoice(&mut self, pVoice: *mut ISpVoice, fAllowFormatChanges: ::BOOL) -> ::HRESULT,
    fn GetVoice(&mut self, ppVoice: *mut *mut ISpVoice) -> ::HRESULT,
    fn SetVoicePurgeEvent(&mut self, ullEventIntereset: ::ULONGLONG) -> ::HRESULT,
    fn GetVoicePurgeEvent(&mut self, pullEventIntereset: *mut ::ULONGLONG) -> ::HRESULT,
    fn SetContextState(&mut self, eContextState: SPCONTEXTSTATE) -> ::HRESULT,
    fn GetContextState(&mut self, peContextState: *mut SPCONTEXTSTATE) -> ::HRESULT
}
);
FLAGS!{enum SPGRAMMAROPTIONS {
    SPGO_SAPI = 0x1,
    SPGO_SRGS = 0x2,
    SPGO_UPS = 0x4,
    SPGO_SRGS_MS_SCRIPT = 0x8,
    SPGO_SRGS_W3C_SCRIPT = 0x100,
    SPGO_SRGS_STG_SCRIPT = 0x200,
    SPGO_SRGS_SCRIPT =
        SPGO_SRGS.0 | SPGO_SRGS_MS_SCRIPT.0 | SPGO_SRGS_W3C_SCRIPT.0 |
             SPGO_SRGS_STG_SCRIPT.0,
    SPGO_FILE = 0x10,
    SPGO_HTTP = 0x20,
    SPGO_RES = 0x40,
    SPGO_OBJECT = 0x80,
    SPGO_DEFAULT = 0x3fb,
    SPGO_ALL = 0x3ff,
}}
FLAGS!{enum SPADAPTATIONSETTINGS {
    SPADS_Default = 0,
    SPADS_CurrentRecognizer = 0x1,
    SPADS_RecoProfile = 0x2,
    SPADS_Immediate = 0x4,
    SPADS_Reset = 0x8,
    SPADS_HighVolumeDataSource = 0x10,
}}
ENUM!{enum SPADAPTATIONRELEVANCE {
    SPAR_Unknown = 0,
    SPAR_Low = 1,
    SPAR_Medium = 2,
    SPAR_High = 3,
}}
RIDL!(
interface ISpRecoContext2(ISpRecoContext2Vtbl): IUnknown(IUnknownVtbl) {
    fn SetGrammarOptions(&mut self, eGrammarOptions: ::DWORD) -> ::HRESULT,
    fn GetGrammarOptions(&mut self, peGrammarOptions: *mut ::DWORD) -> ::HRESULT,
    fn SetAdaptationData2(
        &mut self, pAdaptationData: ::LPCWSTR, cch: ::ULONG, pTopicName: ::LPCWSTR,
        eAdaptationSettings: ::DWORD, eRelevance: SPADAPTATIONRELEVANCE
    ) -> ::HRESULT
}
);
RIDL!(
interface ISpProperties(ISpPropertiesVtbl): IUnknown(IUnknownVtbl) {
    fn SetPropertyNum(&mut self, pName: ::LPCWSTR, lValue: ::LONG) -> ::HRESULT,
    fn GetPropertyNum(&mut self, pName: ::LPCWSTR, plValue: *mut ::LONG) -> ::HRESULT,
    fn SetPropertyString(&mut self, pName: ::LPCWSTR, pValue: ::LPCWSTR) -> ::HRESULT,
    fn GetPropertyString(&mut self, pName: ::LPCWSTR, ppCoMemValue: *mut ::LPWSTR) -> ::HRESULT
}
);
STRUCT!{struct SPRECOGNIZERSTATUS {
    AudioStatus: SPAUDIOSTATUS,
    ullRecognitionStreamPos: ::ULONGLONG,
    ulStreamNumber: ::ULONG,
    ulNumActive: ::ULONG,
    clsidEngine: ::CLSID,
    cLangIDs: ::ULONG,
    aLangID: [::WORD; 20],
    ullRecognitionStreamTime: ::ULONGLONG,
}}
ENUM!{enum SPWAVEFORMATTYPE {
    SPWF_INPUT = 0,
    SPWF_SRENGINE = 1,
}}
pub type SPSTREAMFORMATTYPE = SPWAVEFORMATTYPE;
ENUM!{enum SPRECOSTATE {
    SPRST_INACTIVE = 0,
    SPRST_ACTIVE = 1,
    SPRST_ACTIVE_ALWAYS = 2,
    SPRST_INACTIVE_WITH_PURGE = 3,
    SPRST_NUM_STATES = 4,
}}
RIDL!(
interface ISpRecognizer(ISpRecognizerVtbl): ISpProperties(ISpPropertiesVtbl) {
    fn SetRecognizer(&mut self, pRecognizer: *mut ISpObjectToken) -> ::HRESULT,
    fn GetRecognizer(&mut self, ppRecognizer: *mut *mut ISpObjectToken) -> ::HRESULT,
    fn SetInput(&mut self, pUnkInput: *mut ::IUnknown, fAllowFormatChanges: ::BOOL) -> ::HRESULT,
    fn GetInputObjectToken(&mut self, ppToken: *mut *mut ISpObjectToken) -> ::HRESULT,
    fn GetInputStream(&mut self, ppStream: *mut *mut ISpStreamFormat) -> ::HRESULT,
    fn CreateRecoContext(&mut self, ppNewCtxt: *mut *mut ISpRecoContext) -> ::HRESULT,
    fn GetRecoProfile(&mut self, ppToken: *mut *mut ISpObjectToken) -> ::HRESULT,
    fn SetRecoProfile(&mut self, pToken: *mut ISpObjectToken) -> ::HRESULT,
    fn IsSharedInstance(&mut self) -> ::HRESULT,
    fn GetRecoState(&mut self, pState: *mut SPRECOSTATE) -> ::HRESULT,
    fn SetRecoState(&mut self, NewState: SPRECOSTATE) -> ::HRESULT,
    fn GetStatus(&mut self, pStatus: *mut SPRECOGNIZERSTATUS) -> ::HRESULT,
    fn GetFormat(
        &mut self, WaveFormatType: SPSTREAMFORMATTYPE, pFormatId: *mut ::GUID,
        ppCoMemWFEX: *mut ::WAVEFORMATEX
    ) -> ::HRESULT,
    fn IsUISupported(
        &mut self, pszTypeOfUI: ::LPCWSTR, pvExtraData: *mut ::c_void, cbExtraData: ::ULONG,
        pfSupported: *mut ::BOOL
    ) -> ::HRESULT,
    fn DisplayUI(
        &mut self, hwndParent: ::HWND, pszTitle: ::LPCWSTR, pszTypeOfUI: ::LPCWSTR,
        pvExtraData: *mut ::c_void, cbExtraData: ::ULONG
    ) -> ::HRESULT,
    fn EmulateRecognition(&mut self, pPhrase: *mut ISpPhrase) -> ::HRESULT
}
);
RIDL!(
interface ISpSerializeState(ISpSerializeStateVtbl): IUnknown(IUnknownVtbl) {
    fn GetSerializedState(
        &mut self, ppbData: *mut *mut ::BYTE, pulSize: *mut ::ULONG, dwReserved: ::DWORD
    ) -> ::HRESULT,
    fn SetSerializedState(
        &mut self, pbData: *mut ::BYTE, ulSize: ::ULONG, dwReserved: ::DWORD
    ) -> ::HRESULT
}
);
RIDL!(
interface ISpRecognizer2(ISpRecognizer2Vtbl): IUnknown(IUnknownVtbl) {
    fn EmulateRecognitionEx(
        &mut self, pPhrase: *mut ISpPhrase, dwCompareFlags: ::DWORD
    ) -> ::HRESULT,
    fn SetTrainingState(
        &mut self, fDoingTraining: ::BOOL, fAdaptFromTrainingData: ::BOOL
    ) -> ::HRESULT,
    fn ResetAcousticModelAdaptation(&mut self) -> ::HRESULT
}
);
ENUM!{enum SPCATEGORYTYPE {
    SPCT_COMMAND = 0,
    SPCT_DICTATION,
    SPCT_SLEEP,
    SPCT_SUB_COMMAND,
    SPCT_SUB_DICTATION,
}}
RIDL!(
interface ISpRecoCategory(ISpRecoCategoryVtbl): IUnknown(IUnknownVtbl) {
    fn GetType(&mut self, peCategoryType: *mut SPCATEGORYTYPE) -> ::HRESULT
}
);
RIDL!(
interface ISpRecognizer3(ISpRecognizer3Vtbl): IUnknown(IUnknownVtbl) {
    fn GetCategory(
        &mut self, categoryType: SPCATEGORYTYPE, ppCategory: *mut *mut ISpRecoCategory
    ) -> ::HRESULT,
    fn SetActiveCategory(&mut self, pCategory: *mut ISpRecoCategory) -> ::HRESULT,
    fn GetActiveCategory(&mut self, ppCategory: *mut *mut ISpRecoCategory) -> ::HRESULT
}
);
STRUCT!{struct SPNORMALIZATIONLIST {
    ulSize: ::ULONG,
    ppszzNormalizedList: *mut *mut ::WCHAR,
}}
RIDL!(
interface ISpEnginePronunciation(ISpEnginePronunciationVtbl): IUnknown(IUnknownVtbl) {
    fn Normalize(
        &mut self, pszWord: ::LPCWSTR, pszLeftContext: ::LPCWSTR, pszRightContext: ::LPCWSTR,
        LangID: ::WORD, pNormalizationList: *mut SPNORMALIZATIONLIST
    ) -> ::HRESULT,
    fn GetPronunciations(
        &mut self, pszWord: ::LPCWSTR, pszLeftContext: ::LPCWSTR, pszRightContext: ::LPCWSTR,
        LangID: ::WORD, pEnginePronunciationList: *mut SPWORDPRONUNCIATIONLIST
    ) -> ::HRESULT
}
);
STRUCT!{struct SPDISPLAYTOKEN {
    pszLexical: *const ::WCHAR,
    pszDisplay: *const ::WCHAR,
    bDisplayAttributes: ::BYTE,
}}
STRUCT!{struct SPDISPLAYPHRASE {
    ulNumTokens: ::ULONG,
    pTokens: *mut SPDISPLAYTOKEN,
}}
RIDL!(
interface ISpDisplayAlternates(ISpDisplayAlternatesVtbl): IUnknown(IUnknownVtbl) {
    fn GetDisplayAlternates(
        &mut self, pPhrase: *const SPDISPLAYPHRASE, cRequestCount: ::ULONG,
        ppCoMemPhrases: *mut *mut SPDISPLAYPHRASE, pcPhrasesReturned: *mut ::ULONG
    ) -> ::HRESULT,
    fn SetFullStopTrailSpace(&mut self, ulTrailSpace: ::ULONG) -> ::HRESULT
}
);
pub type SpeechLanguageId = ::c_long;
ENUM!{enum DISPID_SpeechDataKey {
    DISPID_SDKSetBinaryValue = 1,
    DISPID_SDKGetBinaryValue,
    DISPID_SDKSetStringValue,
    DISPID_SDKGetStringValue,
    DISPID_SDKSetLongValue,
    DISPID_SDKGetlongValue,
    DISPID_SDKOpenKey,
    DISPID_SDKCreateKey,
    DISPID_SDKDeleteKey,
    DISPID_SDKDeleteValue,
    DISPID_SDKEnumKeys,
    DISPID_SDKEnumValues,
}}
ENUM!{enum DISPID_SpeechObjectToken {
    DISPID_SOTId = 1,
    DISPID_SOTDataKey,
    DISPID_SOTCategory,
    DISPID_SOTGetDescription,
    DISPID_SOTSetId,
    DISPID_SOTGetAttribute,
    DISPID_SOTCreateInstance,
    DISPID_SOTRemove,
    DISPID_SOTGetStorageFileName,
    DISPID_SOTRemoveStorageFileName,
    DISPID_SOTIsUISupported,
    DISPID_SOTDisplayUI,
    DISPID_SOTMatchesAttributes,
}}
ENUM!{enum SpeechDataKeyLocation {
    SDKLDefaultLocation = SPDKL_DefaultLocation.0,
    SDKLCurrentUser = SPDKL_CurrentUser.0,
    SDKLLocalMachine = SPDKL_LocalMachine.0,
    SDKLCurrentConfig = SPDKL_CurrentConfig.0,
}}
ENUM!{enum SpeechTokenContext {
    STCInprocServer = ::CLSCTX_INPROC_SERVER,
    STCInprocHandler = ::CLSCTX_INPROC_HANDLER,
    STCLocalServer = ::CLSCTX_LOCAL_SERVER,
    STCRemoteServer = ::CLSCTX_REMOTE_SERVER,
    STCAll = ::CLSCTX_INPROC_SERVER | ::CLSCTX_INPROC_HANDLER |
             ::CLSCTX_LOCAL_SERVER | ::CLSCTX_REMOTE_SERVER,
}}
ENUM!{enum SpeechTokenShellFolder {
    STSF_AppData = 0x1a,
    STSF_LocalAppData = 0x1c,
    STSF_CommonAppData = 0x23,
    STSF_FlagCreate = 0x8000,
}}
ENUM!{enum DISPID_SpeechObjectTokens {
    DISPID_SOTsCount = 1,
    DISPID_SOTsItem = ::DISPID_VALUE as u32,
    DISPID_SOTs_NewEnum = ::DISPID_NEWENUM as u32,
}}
ENUM!{enum DISPID_SpeechObjectTokenCategory {
    DISPID_SOTCId = 1,
    DISPID_SOTCDefault,
    DISPID_SOTCSetId,
    DISPID_SOTCGetDataKey,
    DISPID_SOTCEnumerateTokens,
}}
ENUM!{enum SpeechAudioFormatType {
    SAFTDefault = -1i32 as u32,
    SAFTNoAssignedFormat = 0,
    SAFTText = 1,
    SAFTNonStandardFormat = 2,
    SAFTExtendedAudioFormat = 3,
    SAFT8kHz8BitMono = 4,
    SAFT8kHz8BitStereo = 5,
    SAFT8kHz16BitMono = 6,
    SAFT8kHz16BitStereo = 7,
    SAFT11kHz8BitMono = 8,
    SAFT11kHz8BitStereo = 9,
    SAFT11kHz16BitMono = 10,
    SAFT11kHz16BitStereo = 11,
    SAFT12kHz8BitMono = 12,
    SAFT12kHz8BitStereo = 13,
    SAFT12kHz16BitMono = 14,
    SAFT12kHz16BitStereo = 15,
    SAFT16kHz8BitMono = 16,
    SAFT16kHz8BitStereo = 17,
    SAFT16kHz16BitMono = 18,
    SAFT16kHz16BitStereo = 19,
    SAFT22kHz8BitMono = 20,
    SAFT22kHz8BitStereo = 21,
    SAFT22kHz16BitMono = 22,
    SAFT22kHz16BitStereo = 23,
    SAFT24kHz8BitMono = 24,
    SAFT24kHz8BitStereo = 25,
    SAFT24kHz16BitMono = 26,
    SAFT24kHz16BitStereo = 27,
    SAFT32kHz8BitMono = 28,
    SAFT32kHz8BitStereo = 29,
    SAFT32kHz16BitMono = 30,
    SAFT32kHz16BitStereo = 31,
    SAFT44kHz8BitMono = 32,
    SAFT44kHz8BitStereo = 33,
    SAFT44kHz16BitMono = 34,
    SAFT44kHz16BitStereo = 35,
    SAFT48kHz8BitMono = 36,
    SAFT48kHz8BitStereo = 37,
    SAFT48kHz16BitMono = 38,
    SAFT48kHz16BitStereo = 39,
    SAFTTrueSpeech_8kHz1BitMono = 40,
    SAFTCCITT_ALaw_8kHzMono = 41,
    SAFTCCITT_ALaw_8kHzStereo = 42,
    SAFTCCITT_ALaw_11kHzMono = 43,
    SAFTCCITT_ALaw_11kHzStereo = 44,
    SAFTCCITT_ALaw_22kHzMono = 45,
    SAFTCCITT_ALaw_22kHzStereo = 46,
    SAFTCCITT_ALaw_44kHzMono = 47,
    SAFTCCITT_ALaw_44kHzStereo = 48,
    SAFTCCITT_uLaw_8kHzMono = 49,
    SAFTCCITT_uLaw_8kHzStereo = 50,
    SAFTCCITT_uLaw_11kHzMono = 51,
    SAFTCCITT_uLaw_11kHzStereo = 52,
    SAFTCCITT_uLaw_22kHzMono = 53,
    SAFTCCITT_uLaw_22kHzStereo = 54,
    SAFTCCITT_uLaw_44kHzMono = 55,
    SAFTCCITT_uLaw_44kHzStereo = 56,
    SAFTADPCM_8kHzMono = 57,
    SAFTADPCM_8kHzStereo = 58,
    SAFTADPCM_11kHzMono = 59,
    SAFTADPCM_11kHzStereo = 60,
    SAFTADPCM_22kHzMono = 61,
    SAFTADPCM_22kHzStereo = 62,
    SAFTADPCM_44kHzMono = 63,
    SAFTADPCM_44kHzStereo = 64,
    SAFTGSM610_8kHzMono = 65,
    SAFTGSM610_11kHzMono = 66,
    SAFTGSM610_22kHzMono = 67,
    SAFTGSM610_44kHzMono = 68,
}}
ENUM!{enum DISPID_SpeechAudioFormat {
    DISPID_SAFType = 1,
    DISPID_SAFGuid,
    DISPID_SAFGetWaveFormatEx,
    DISPID_SAFSetWaveFormatEx,
}}
ENUM!{enum DISPID_SpeechBaseStream {
    DISPID_SBSFormat = 1,
    DISPID_SBSRead,
    DISPID_SBSWrite,
    DISPID_SBSSeek,
}}
ENUM!{enum SpeechStreamSeekPositionType {
    SSSPTRelativeToStart = ::STREAM_SEEK_SET.0,
    SSSPTRelativeToCurrentPosition = ::STREAM_SEEK_CUR.0,
    SSSPTRelativeToEnd = ::STREAM_SEEK_END.0,
}}
ENUM!{enum DISPID_SpeechAudio {
    DISPID_SAStatus = 200,
    DISPID_SABufferInfo,
    DISPID_SADefaultFormat,
    DISPID_SAVolume,
    DISPID_SABufferNotifySize,
    DISPID_SAEventHandle,
    DISPID_SASetState,
}}
ENUM!{enum SpeechAudioState {
    SASClosed = SPAS_CLOSED.0,
    SASStop = SPAS_STOP.0,
    SASPause = SPAS_PAUSE.0,
    SASRun = SPAS_RUN.0,
}}
ENUM!{enum DISPID_SpeechMMSysAudio {
    DISPID_SMSADeviceId = 300,
    DISPID_SMSALineId,
    DISPID_SMSAMMHandle,
}}
ENUM!{enum DISPID_SpeechFileStream {
    DISPID_SFSOpen = 100,
    DISPID_SFSClose,
}}
ENUM!{enum SpeechStreamFileMode {
    SSFMOpenForRead = SPFM_OPEN_READONLY.0,
    SSFMOpenReadWrite = SPFM_OPEN_READWRITE.0,
    SSFMCreate = SPFM_CREATE.0,
    SSFMCreateForWrite = SPFM_CREATE_ALWAYS.0,
}}
ENUM!{enum DISPID_SpeechCustomStream {
    DISPID_SCSBaseStream = 100,
}}
ENUM!{enum DISPID_SpeechMemoryStream {
    DISPID_SMSSetData = 100,
    DISPID_SMSGetData,
}}
ENUM!{enum DISPID_SpeechAudioStatus {
    DISPID_SASFreeBufferSpace = 1,
    DISPID_SASNonBlockingIO,
    DISPID_SASState,
    DISPID_SASCurrentSeekPosition,
    DISPID_SASCurrentDevicePosition,
}}
ENUM!{enum DISPID_SpeechAudioBufferInfo {
    DISPID_SABIMinNotification = 1,
    DISPID_SABIBufferSize,
    DISPID_SABIEventBias,
}}
ENUM!{enum DISPID_SpeechWaveFormatEx {
    DISPID_SWFEFormatTag = 1,
    DISPID_SWFEChannels,
    DISPID_SWFESamplesPerSec,
    DISPID_SWFEAvgBytesPerSec,
    DISPID_SWFEBlockAlign,
    DISPID_SWFEBitsPerSample,
    DISPID_SWFEExtraData,
}}
ENUM!{enum DISPID_SpeechVoice {
    DISPID_SVStatus = 1,
    DISPID_SVVoice,
    DISPID_SVAudioOutput,
    DISPID_SVAudioOutputStream,
    DISPID_SVRate,
    DISPID_SVVolume,
    DISPID_SVAllowAudioOuputFormatChangesOnNextSet,
    DISPID_SVEventInterests,
    DISPID_SVPriority,
    DISPID_SVAlertBoundary,
    DISPID_SVSyncronousSpeakTimeout,
    DISPID_SVSpeak,
    DISPID_SVSpeakStream,
    DISPID_SVPause,
    DISPID_SVResume,
    DISPID_SVSkip,
    DISPID_SVGetVoices,
    DISPID_SVGetAudioOutputs,
    DISPID_SVWaitUntilDone,
    DISPID_SVSpeakCompleteEvent,
    DISPID_SVIsUISupported,
    DISPID_SVDisplayUI,
}}
ENUM!{enum SpeechVoicePriority {
    SVPNormal = SPVPRI_NORMAL.0,
    SVPAlert = SPVPRI_ALERT.0,
    SVPOver = SPVPRI_OVER.0,
}}
FLAGS!{enum SpeechVoiceSpeakFlags {
    SVSFDefault = SPF_DEFAULT.0,
    SVSFlagsAsync = SPF_ASYNC.0,
    SVSFPurgeBeforeSpeak = SPF_PURGEBEFORESPEAK.0,
    SVSFIsFilename = SPF_IS_FILENAME.0,
    SVSFIsXML = SPF_IS_XML.0,
    SVSFIsNotXML = SPF_IS_NOT_XML.0,
    SVSFPersistXML = SPF_PERSIST_XML.0,
    SVSFNLPSpeakPunc = SPF_NLP_SPEAK_PUNC.0,
    SVSFParseSapi = SPF_PARSE_SAPI.0,
    SVSFParseSsml = SPF_PARSE_SSML.0,
    SVSFParseMask = SPF_PARSE_MASK as u32,
    SVSFVoiceMask = SPF_VOICE_MASK as u32,
    SVSFUnusedFlags = SPF_UNUSED_FLAGS as u32,
}}
pub const SVSFParseAutodetect: SpeechVoiceSpeakFlags = SVSFDefault;
pub const SVSFNLPMask: SpeechVoiceSpeakFlags = SVSFNLPSpeakPunc;
FLAGS!{enum SpeechVoiceEvents {
    SVEStartInputStream = 1 << 1,
    SVEEndInputStream = 1 << 2,
    SVEVoiceChange = 1 << 3,
    SVEBookmark = 1 << 4,
    SVEWordBoundary = 1 << 5,
    SVEPhoneme = 1 << 6,
    SVESentenceBoundary = 1 << 7,
    SVEViseme = 1 << 8,
    SVEAudioLevel = 1 << 9,
    SVEPrivate = 1 << 15,
    SVEAllEvents = 0x83fe,
}}
ENUM!{enum DISPID_SpeechVoiceStatus {
    DISPID_SVSCurrentStreamNumber = 1,
    DISPID_SVSLastStreamNumberQueued,
    DISPID_SVSLastResult,
    DISPID_SVSRunningState,
    DISPID_SVSInputWordPosition,
    DISPID_SVSInputWordLength,
    DISPID_SVSInputSentencePosition,
    DISPID_SVSInputSentenceLength,
    DISPID_SVSLastBookmark,
    DISPID_SVSLastBookmarkId,
    DISPID_SVSPhonemeId,
    DISPID_SVSVisemeId,
}}
ENUM!{enum SpeechRunState {
    SRSEDone = SPRS_DONE.0,
    SRSEIsSpeaking = SPRS_IS_SPEAKING.0,
}}
ENUM!{enum SpeechVisemeType {
    SVP_0 = 0,
    SVP_1,
    SVP_2,
    SVP_3,
    SVP_4,
    SVP_5,
    SVP_6,
    SVP_7,
    SVP_8,
    SVP_9,
    SVP_10,
    SVP_11,
    SVP_12,
    SVP_13,
    SVP_14,
    SVP_15,
    SVP_16,
    SVP_17,
    SVP_18,
    SVP_19,
    SVP_20,
    SVP_21,
}}
ENUM!{enum SpeechVisemeFeature {
    SVF_None = 0,
    SVF_Stressed = SPVFEATURE_STRESSED.0,
    SVF_Emphasis = SPVFEATURE_EMPHASIS.0,
}}
ENUM!{enum DISPID_SpeechVoiceEvent {
    DISPID_SVEStreamStart = 1,
    DISPID_SVEStreamEnd,
    DISPID_SVEVoiceChange,
    DISPID_SVEBookmark,
    DISPID_SVEWord,
    DISPID_SVEPhoneme,
    DISPID_SVESentenceBoundary,
    DISPID_SVEViseme,
    DISPID_SVEAudioLevel,
    DISPID_SVEEnginePrivate,
}}
ENUM!{enum DISPID_SpeechRecognizer {
    DISPID_SRRecognizer = 1,
    DISPID_SRAllowAudioInputFormatChangesOnNextSet,
    DISPID_SRAudioInput,
    DISPID_SRAudioInputStream,
    DISPID_SRIsShared,
    DISPID_SRState,
    DISPID_SRStatus,
    DISPID_SRProfile,
    DISPID_SREmulateRecognition,
    DISPID_SRCreateRecoContext,
    DISPID_SRGetFormat,
    DISPID_SRSetPropertyNumber,
    DISPID_SRGetPropertyNumber,
    DISPID_SRSetPropertyString,
    DISPID_SRGetPropertyString,
    DISPID_SRIsUISupported,
    DISPID_SRDisplayUI,
    DISPID_SRGetRecognizers,
    DISPID_SVGetAudioInputs,
    DISPID_SVGetProfiles,
}}
ENUM!{enum SpeechRecognizerState {
    SRSInactive = SPRST_INACTIVE.0,
    SRSActive = SPRST_ACTIVE.0,
    SRSActiveAlways = SPRST_ACTIVE_ALWAYS.0,
    SRSInactiveWithPurge = SPRST_INACTIVE_WITH_PURGE.0,
}}
ENUM!{enum SpeechDisplayAttributes {
    SDA_No_Trailing_Space = 0,
    SDA_One_Trailing_Space = SPAF_ONE_TRAILING_SPACE.0,
    SDA_Two_Trailing_Spaces = SPAF_TWO_TRAILING_SPACES.0,
    SDA_Consume_Leading_Spaces = SPAF_CONSUME_LEADING_SPACES.0,
}}
ENUM!{enum SpeechFormatType {
    SFTInput = SPWF_INPUT.0,
    SFTSREngine = SPWF_SRENGINE.0,
}}
FLAGS!{enum SpeechEmulationCompareFlags {
    SECFIgnoreCase = 0x1,
    SECFIgnoreKanaType = 0x10000,
    SECFIgnoreWidth = 0x20000,
    SECFNoSpecialChars = 0x20000000,
    SECFEmulateResult = 0x40000000,
    SECFDefault = SECFIgnoreCase.0 | SECFIgnoreKanaType.0 | SECFIgnoreWidth.0,
}}
ENUM!{enum DISPID_SpeechRecognizerStatus {
    DISPID_SRSAudioStatus = 1,
    DISPID_SRSCurrentStreamPosition,
    DISPID_SRSCurrentStreamNumber,
    DISPID_SRSNumberOfActiveRules,
    DISPID_SRSClsidEngine,
    DISPID_SRSSupportedLanguages,
}}
ENUM!{enum DISPID_SpeechRecoContext {
    DISPID_SRCRecognizer = 1,
    DISPID_SRCAudioInInterferenceStatus,
    DISPID_SRCRequestedUIType,
    DISPID_SRCVoice,
    DISPID_SRAllowVoiceFormatMatchingOnNextSet,
    DISPID_SRCVoicePurgeEvent,
    DISPID_SRCEventInterests,
    DISPID_SRCCmdMaxAlternates,
    DISPID_SRCState,
    DISPID_SRCRetainedAudio,
    DISPID_SRCRetainedAudioFormat,
    DISPID_SRCPause,
    DISPID_SRCResume,
    DISPID_SRCCreateGrammar,
    DISPID_SRCCreateResultFromMemory,
    DISPID_SRCBookmark,
    DISPID_SRCSetAdaptationData,
}}
ENUM!{enum SpeechRetainedAudioOptions {
    SRAONone = SPAO_NONE.0,
    SRAORetainAudio = SPAO_RETAIN_AUDIO.0,
}}
ENUM!{enum SpeechBookmarkOptions {
    SBONone = SPBO_NONE.0,
    SBOPause = SPBO_PAUSE.0,
}}
ENUM!{enum SpeechInterference {
    SINone = SPINTERFERENCE_NONE.0,
    SINoise = SPINTERFERENCE_NOISE.0,
    SINoSignal = SPINTERFERENCE_NOSIGNAL.0,
    SITooLoud = SPINTERFERENCE_TOOLOUD.0,
    SITooQuiet = SPINTERFERENCE_TOOQUIET.0,
    SITooFast = SPINTERFERENCE_TOOFAST.0,
    SITooSlow = SPINTERFERENCE_TOOSLOW.0,
}}
FLAGS!{enum SpeechRecoEvents {
    SREStreamEnd = 1 << 0,
    SRESoundStart = 1 << 1,
    SRESoundEnd = 1 << 2,
    SREPhraseStart = 1 << 3,
    SRERecognition = 1 << 4,
    SREHypothesis = 1 << 5,
    SREBookmark = 1 << 6,
    SREPropertyNumChange = 1 << 7,
    SREPropertyStringChange = 1 << 8,
    SREFalseRecognition = 1 << 9,
    SREInterference = 1 << 10,
    SRERequestUI = 1 << 11,
    SREStateChange = 1 << 12,
    SREAdaptation = 1 << 13,
    SREStreamStart = 1 << 14,
    SRERecoOtherContext = 1 << 15,
    SREAudioLevel = 1 << 16,
    SREPrivate = 1 << 18,
    SREAllEvents = 0x5ffff,
}}
ENUM!{enum SpeechRecoContextState {
    SRCS_Disabled = SPCS_DISABLED.0,
    SRCS_Enabled = SPCS_ENABLED.0,
}}
ENUM!{enum DISPIDSPRG {
    DISPID_SRGId = 1,
    DISPID_SRGRecoContext,
    DISPID_SRGState,
    DISPID_SRGRules,
    DISPID_SRGReset,
    DISPID_SRGCommit,
    DISPID_SRGCmdLoadFromFile,
    DISPID_SRGCmdLoadFromObject,
    DISPID_SRGCmdLoadFromResource,
    DISPID_SRGCmdLoadFromMemory,
    DISPID_SRGCmdLoadFromProprietaryGrammar,
    DISPID_SRGCmdSetRuleState,
    DISPID_SRGCmdSetRuleIdState,
    DISPID_SRGDictationLoad,
    DISPID_SRGDictationUnload,
    DISPID_SRGDictationSetState,
    DISPID_SRGSetWordSequenceData,
    DISPID_SRGSetTextSelection,
    DISPID_SRGIsPronounceable,
}}
ENUM!{enum SpeechLoadOption {
    SLOStatic = SPLO_STATIC.0,
    SLODynamic = SPLO_DYNAMIC.0,
}}
ENUM!{enum SpeechWordPronounceable {
    SWPUnknownWordUnpronounceable = SPWP_UNKNOWN_WORD_UNPRONOUNCEABLE.0,
    SWPUnknownWordPronounceable = SPWP_UNKNOWN_WORD_PRONOUNCEABLE.0,
    SWPKnownWordPronounceable = SPWP_KNOWN_WORD_PRONOUNCEABLE.0,
}}
ENUM!{enum SpeechGrammarState {
    SGSEnabled = SPGS_ENABLED.0,
    SGSDisabled = SPGS_DISABLED.0,
    SGSExclusive = SPGS_EXCLUSIVE.0,
}}
ENUM!{enum SpeechRuleState {
    SGDSInactive = SPRS_INACTIVE.0,
    SGDSActive = SPRS_ACTIVE.0,
    SGDSActiveWithAutoPause = SPRS_ACTIVE_WITH_AUTO_PAUSE.0,
    SGDSActiveUserDelimited = SPRS_ACTIVE_USER_DELIMITED.0,
}}
ENUM!{enum SpeechRuleAttributes {
    SRATopLevel = SPRAF_TopLevel.0,
    SRADefaultToActive = SPRAF_Active.0,
    SRAExport = SPRAF_Export.0,
    SRAImport = SPRAF_Import.0,
    SRAInterpreter = SPRAF_Interpreter.0,
    SRADynamic = SPRAF_Dynamic.0,
    SRARoot = SPRAF_Root.0,
}}
ENUM!{enum SpeechGrammarWordType {
    SGDisplay = SPWT_DISPLAY.0,
    SGLexical = SPWT_LEXICAL.0,
    SGPronounciation = SPWT_PRONUNCIATION.0,
    SGLexicalNoSpecialChars = SPWT_LEXICAL_NO_SPECIAL_CHARS.0,
}}
ENUM!{enum DISPID_SpeechRecoContextEvents {
    DISPID_SRCEStartStream = 1,
    DISPID_SRCEEndStream,
    DISPID_SRCEBookmark,
    DISPID_SRCESoundStart,
    DISPID_SRCESoundEnd,
    DISPID_SRCEPhraseStart,
    DISPID_SRCERecognition,
    DISPID_SRCEHypothesis,
    DISPID_SRCEPropertyNumberChange,
    DISPID_SRCEPropertyStringChange,
    DISPID_SRCEFalseRecognition,
    DISPID_SRCEInterference,
    DISPID_SRCERequestUI,
    DISPID_SRCERecognizerStateChange,
    DISPID_SRCEAdaptation,
    DISPID_SRCERecognitionForOtherContext,
    DISPID_SRCEAudioLevel,
    DISPID_SRCEEnginePrivate,
}}
ENUM!{enum SpeechRecognitionType {
    SRTStandard = 0,
    SRTAutopause = SPREF_AutoPause.0,
    SRTEmulated = SPREF_Emulated.0,
    SRTSMLTimeout = SPREF_SMLTimeout.0,
    SRTExtendableParse = SPREF_ExtendableParse.0,
    SRTReSent = SPREF_ReSent.0,
}}
ENUM!{enum DISPID_SpeechGrammarRule {
    DISPID_SGRAttributes = 1,
    DISPID_SGRInitialState,
    DISPID_SGRName,
    DISPID_SGRId,
    DISPID_SGRClear,
    DISPID_SGRAddResource,
    DISPID_SGRAddState,
}}
ENUM!{enum DISPID_SpeechGrammarRules {
    DISPID_SGRsCount = 1,
    DISPID_SGRsDynamic,
    DISPID_SGRsAdd,
    DISPID_SGRsCommit,
    DISPID_SGRsCommitAndSave,
    DISPID_SGRsFindRule,
    DISPID_SGRsItem = ::DISPID_VALUE as u32,
    DISPID_SGRs_NewEnum = ::DISPID_NEWENUM as u32,
}}
ENUM!{enum DISPID_SpeechGrammarRuleState {
    DISPID_SGRSRule = 1,
    DISPID_SGRSTransitions,
    DISPID_SGRSAddWordTransition,
    DISPID_SGRSAddRuleTransition,
    DISPID_SGRSAddSpecialTransition,
}}
ENUM!{enum SpeechSpecialTransitionType {
    SSTTWildcard = 1,
    SSTTDictation,
    SSTTTextBuffer,
}}
ENUM!{enum DISPID_SpeechGrammarRuleStateTransitions {
    DISPID_SGRSTsCount = 1,
    DISPID_SGRSTsItem = ::DISPID_VALUE as u32,
    DISPID_SGRSTs_NewEnum = ::DISPID_NEWENUM as u32,
}}
ENUM!{enum DISPID_SpeechGrammarRuleStateTransition {
    DISPID_SGRSTType = 1,
    DISPID_SGRSTText,
    DISPID_SGRSTRule,
    DISPID_SGRSTWeight,
    DISPID_SGRSTPropertyName,
    DISPID_SGRSTPropertyId,
    DISPID_SGRSTPropertyValue,
    DISPID_SGRSTNextState,
}}
ENUM!{enum SpeechGrammarRuleStateTransitionType {
    SGRSTTEpsilon = 0,
    SGRSTTWord,
    SGRSTTRule,
    SGRSTTDictation,
    SGRSTTWildcard,
    SGRSTTTextBuffer,
}}
ENUM!{enum DISPIDSPTSI {
    DISPIDSPTSI_ActiveOffset = 1,
    DISPIDSPTSI_ActiveLength,
    DISPIDSPTSI_SelectionOffset,
    DISPIDSPTSI_SelectionLength,
}}
ENUM!{enum DISPID_SpeechRecoResult {
    DISPID_SRRRecoContext = 1,
    DISPID_SRRTimes,
    DISPID_SRRAudioFormat,
    DISPID_SRRPhraseInfo,
    DISPID_SRRAlternates,
    DISPID_SRRAudio,
    DISPID_SRRSpeakAudio,
    DISPID_SRRSaveToMemory,
    DISPID_SRRDiscardResultInfo,
}}
ENUM!{enum SpeechDiscardType {
    SDTProperty = SPDF_PROPERTY.0,
    SDTReplacement = SPDF_REPLACEMENT.0,
    SDTRule = SPDF_RULE.0,
    SDTDisplayText = SPDF_DISPLAYTEXT.0,
    SDTLexicalForm = SPDF_LEXICALFORM.0,
    SDTPronunciation = SPDF_PRONUNCIATION.0,
    SDTAudio = SPDF_AUDIO.0,
    SDTAlternates = SPDF_ALTERNATES.0,
    SDTAll = SPDF_ALL.0,
}}
ENUM!{enum DISPID_SpeechXMLRecoResult {
    DISPID_SRRGetXMLResult,
    DISPID_SRRGetXMLErrorInfo,
}}
ENUM!{enum DISPID_SpeechRecoResult2 {
    DISPID_SRRSetTextFeedback,
}}
ENUM!{enum DISPID_SpeechPhraseBuilder {
    DISPID_SPPBRestorePhraseFromMemory = 1,
}}
ENUM!{enum DISPID_SpeechRecoResultTimes {
    DISPID_SRRTStreamTime = 1,
    DISPID_SRRTLength,
    DISPID_SRRTTickCount,
    DISPID_SRRTOffsetFromStart,
}}
ENUM!{enum DISPID_SpeechPhraseAlternate {
    DISPID_SPARecoResult = 1,
    DISPID_SPAStartElementInResult,
    DISPID_SPANumberOfElementsInResult,
    DISPID_SPAPhraseInfo,
    DISPID_SPACommit,
}}
ENUM!{enum DISPID_SpeechPhraseAlternates {
    DISPID_SPAsCount = 1,
    DISPID_SPAsItem = ::DISPID_VALUE as u32,
    DISPID_SPAs_NewEnum = ::DISPID_NEWENUM as u32,
}}
ENUM!{enum DISPID_SpeechPhraseInfo {
    DISPID_SPILanguageId = 1,
    DISPID_SPIGrammarId,
    DISPID_SPIStartTime,
    DISPID_SPIAudioStreamPosition,
    DISPID_SPIAudioSizeBytes,
    DISPID_SPIRetainedSizeBytes,
    DISPID_SPIAudioSizeTime,
    DISPID_SPIRule,
    DISPID_SPIProperties,
    DISPID_SPIElements,
    DISPID_SPIReplacements,
    DISPID_SPIEngineId,
    DISPID_SPIEnginePrivateData,
    DISPID_SPISaveToMemory,
    DISPID_SPIGetText,
    DISPID_SPIGetDisplayAttributes,
}}
ENUM!{enum DISPID_SpeechPhraseElement {
    DISPID_SPEAudioTimeOffset = 1,
    DISPID_SPEAudioSizeTime,
    DISPID_SPEAudioStreamOffset,
    DISPID_SPEAudioSizeBytes,
    DISPID_SPERetainedStreamOffset,
    DISPID_SPERetainedSizeBytes,
    DISPID_SPEDisplayText,
    DISPID_SPELexicalForm,
    DISPID_SPEPronunciation,
    DISPID_SPEDisplayAttributes,
    DISPID_SPERequiredConfidence,
    DISPID_SPEActualConfidence,
    DISPID_SPEEngineConfidence,
}}
ENUM!{enum SpeechEngineConfidence {
    SECLowConfidence = -1i32 as u32,
    SECNormalConfidence = 0,
    SECHighConfidence = 1,
}}
ENUM!{enum DISPID_SpeechPhraseElements {
    DISPID_SPEsCount = 1,
    DISPID_SPEsItem = ::DISPID_VALUE as u32,
    DISPID_SPEs_NewEnum = ::DISPID_NEWENUM as u32,
}}
ENUM!{enum DISPID_SpeechPhraseReplacement {
    DISPID_SPRDisplayAttributes = 1,
    DISPID_SPRText,
    DISPID_SPRFirstElement,
    DISPID_SPRNumberOfElements,
}}
ENUM!{enum DISPID_SpeechPhraseReplacements {
    DISPID_SPRsCount = 1,
    DISPID_SPRsItem = ::DISPID_VALUE as u32,
    DISPID_SPRs_NewEnum = ::DISPID_NEWENUM as u32,
}}
ENUM!{enum DISPID_SpeechPhraseProperty {
    DISPID_SPPName = 1,
    DISPID_SPPId,
    DISPID_SPPValue,
    DISPID_SPPFirstElement,
    DISPID_SPPNumberOfElements,
    DISPID_SPPEngineConfidence,
    DISPID_SPPConfidence,
    DISPID_SPPParent,
    DISPID_SPPChildren,
}}
ENUM!{enum DISPID_SpeechPhraseProperties {
    DISPID_SPPsCount = 1,
    DISPID_SPPsItem = ::DISPID_VALUE as u32,
    DISPID_SPPs_NewEnum = ::DISPID_NEWENUM as u32,
}}
ENUM!{enum DISPID_SpeechPhraseRule {
    DISPID_SPRuleName = 1,
    DISPID_SPRuleId,
    DISPID_SPRuleFirstElement,
    DISPID_SPRuleNumberOfElements,
    DISPID_SPRuleParent,
    DISPID_SPRuleChildren,
    DISPID_SPRuleConfidence,
    DISPID_SPRuleEngineConfidence,
}}
ENUM!{enum DISPID_SpeechPhraseRules {
    DISPID_SPRulesCount = 1,
    DISPID_SPRulesItem = ::DISPID_VALUE as u32,
    DISPID_SPRules_NewEnum = ::DISPID_NEWENUM as u32,
}}
ENUM!{enum DISPID_SpeechLexicon {
    DISPID_SLGenerationId = 1,
    DISPID_SLGetWords,
    DISPID_SLAddPronunciation,
    DISPID_SLAddPronunciationByPhoneIds,
    DISPID_SLRemovePronunciation,
    DISPID_SLRemovePronunciationByPhoneIds,
    DISPID_SLGetPronunciations,
    DISPID_SLGetGenerationChange,
}}
ENUM!{enum SpeechLexiconType {
    SLTUser = eLEXTYPE_USER.0,
    SLTApp = eLEXTYPE_APP.0,
}}
ENUM!{enum SpeechPartOfSpeech {
    SPSNotOverriden = SPPS_NotOverriden.0,
    SPSUnknown = SPPS_Unknown.0,
    SPSNoun = SPPS_Noun.0,
    SPSVerb = SPPS_Verb.0,
    SPSModifier = SPPS_Modifier.0,
    SPSFunction = SPPS_Function.0,
    SPSInterjection = SPPS_Interjection.0,
    SPSLMA = SPPS_LMA.0,
    SPSSuppressWord = SPPS_SuppressWord.0,
}}
ENUM!{enum DISPID_SpeechLexiconWords {
    DISPID_SLWsCount = 1,
    DISPID_SLWsItem = ::DISPID_VALUE as u32,
    DISPID_SLWs_NewEnum = ::DISPID_NEWENUM as u32,
}}
ENUM!{enum SpeechWordType {
    SWTAdded = eWORDTYPE_ADDED.0,
    SWTDeleted = eWORDTYPE_DELETED.0,
}}
ENUM!{enum DISPID_SpeechLexiconWord {
    DISPID_SLWLangId = 1,
    DISPID_SLWType,
    DISPID_SLWWord,
    DISPID_SLWPronunciations,
}}
ENUM!{enum DISPID_SpeechLexiconProns {
    DISPID_SLPsCount = 1,
    DISPID_SLPsItem = ::DISPID_VALUE as u32,
    DISPID_SLPs_NewEnum = ::DISPID_NEWENUM as u32,
}}
ENUM!{enum DISPID_SpeechLexiconPronunciation {
    DISPID_SLPType = 1,
    DISPID_SLPLangId,
    DISPID_SLPPartOfSpeech,
    DISPID_SLPPhoneIds,
    DISPID_SLPSymbolic,
}}
ENUM!{enum DISPID_SpeechPhoneConverter {
    DISPID_SPCLangId = 1,
    DISPID_SPCPhoneToId,
    DISPID_SPCIdToPhone,
}}
RIDL!(
interface ISpeechDataKey(ISpeechDataKeyVtbl): IDispatch(IDispatchVtbl) {
    fn SetBinaryValue(&mut self, ValueName: ::BSTR, Value: ::VARIANT) -> ::HRESULT,
    fn GetBinaryValue(&mut self, ValueName: ::BSTR, Value: *mut ::VARIANT) -> ::HRESULT,
    fn SetStringValue(&mut self, ValueName: ::BSTR, Value: ::BSTR) -> ::HRESULT,
    fn GetStringValue(&mut self, ValueName: ::BSTR, Value: *mut ::BSTR) -> ::HRESULT,
    fn SetLongValue(&mut self, ValueName: ::BSTR, Value: ::c_long) -> ::HRESULT,
    fn GetLongValue(&mut self, ValueName: ::BSTR, Value: *mut ::c_long) -> ::HRESULT,
    fn OpenKey(&mut self, SubKeyName: ::BSTR, SubKey: *mut *mut ISpeechDataKey) -> ::HRESULT,
    fn CreateKey(&mut self, SubKeyName: ::BSTR, SubKey: *mut *mut ISpeechDataKey) -> ::HRESULT,
    fn DeleteKey(&mut self, SubKeyName: ::BSTR) -> ::HRESULT,
    fn DeleteValue(&mut self, ValueName: ::BSTR) -> ::HRESULT,
    fn EnumKeys(&mut self, Index: ::c_long, SubKeyName: *mut ::BSTR) -> ::HRESULT,
    fn EnumValues(&mut self, Index: ::c_long, ValueName: *mut ::BSTR) -> ::HRESULT
}
);
RIDL!(
interface ISpeechObjectToken(ISpeechObjectTokenVtbl): IDispatch(IDispatchVtbl) {
    fn get_Id(&mut self, ObjectId: *mut ::BSTR) -> ::HRESULT,
    fn get_DataKey(&mut self, DataKey: *mut *mut ISpeechDataKey) -> ::HRESULT,
    fn get_Category(&mut self, Category: *mut *mut ISpeechObjectTokenCategory) -> ::HRESULT,
    fn GetDescription(&mut self, Locale: ::c_long, Description: *mut ::BSTR) -> ::HRESULT,
    fn SetId(
        &mut self, Id: ::BSTR, CategoryId: ::BSTR, CreateIfNotExist: ::VARIANT_BOOL
    ) -> ::HRESULT,
    fn GetAttribute(&mut self, AttributeName: ::BSTR, AttributeValue: *mut ::BSTR) -> ::HRESULT,
    fn CreateInstance(
        &mut self, pUnkOuter: *mut ::IUnknown, ClsContext: SpeechTokenContext,
        Object: *mut *mut ::IUnknown
    ) -> ::HRESULT,
    fn Remove(&mut self, ObjectStorageCLSID: ::BSTR) -> ::HRESULT,
    fn GetStorageFileName(
        &mut self, ObjectStorageCLSID: ::BSTR, KeyName: ::BSTR, FileName: ::BSTR, Folder: ::BSTR,
        FilePath: *mut ::BSTR
    ) -> ::HRESULT,
    fn RemoveStorageFileName(
        &mut self, ObjectStorageCLSID: ::BSTR, KeyName: ::BSTR, DeleteFile: ::VARIANT_BOOL
    ) -> ::HRESULT,
    fn IsUISupported(
        &mut self, TypeOfUI: ::BSTR, ExtraData: *const ::VARIANT, Object: *mut ::IUnknown,
        Supported: *mut ::VARIANT_BOOL
    ) -> ::HRESULT,
    fn DisplayUI(
        &mut self, hWnd: ::c_long, Title: ::BSTR, TypeOfUI: ::BSTR, ExtraData: *const ::VARIANT,
        Object: *mut ::IUnknown
    ) -> ::HRESULT,
    fn MatchesAttributes(&mut self, Attributes: ::BSTR, Matches: *mut ::VARIANT_BOOL) -> ::HRESULT
}
);
RIDL!(
interface ISpeechObjectTokens(ISpeechObjectTokensVtbl): IDispatch(IDispatchVtbl) {
    fn get_Count(&mut self, Count: *mut ::c_long) -> ::HRESULT,
    fn Item(&mut self, Index: ::c_long, Token: *mut *mut ISpeechObjectToken) -> ::HRESULT,
    fn get__NewEnum(&mut self, ppEnumVARIANT: *mut *mut ::IUnknown) -> ::HRESULT
}
);
RIDL!(
interface ISpeechObjectTokenCategory(ISpeechObjectTokenCategoryVtbl): IDispatch(IDispatchVtbl) {
    fn get_Id(&mut self, Id: *mut ::BSTR) -> ::HRESULT,
    fn put_Default(&mut self, TokenId: ::BSTR) -> ::HRESULT,
    fn get_Default(&mut self, TokenId: *mut ::BSTR) -> ::HRESULT,
    fn SetId(&mut self, Id: ::BSTR, CreateIfNotExist: ::VARIANT_BOOL) -> ::HRESULT,
    fn GetDataKey(
        &mut self, Location: SpeechDataKeyLocation, DataKey: *mut *mut ISpeechDataKey
    ) -> ::HRESULT,
    fn EnumerateTokens(
        &mut self, RequiredAttributes: ::BSTR, OptionalAttributes: ::BSTR,
        Tokens: *mut *mut ISpeechObjectTokens
    ) -> ::HRESULT
}
);
RIDL!(
interface ISpeechAudioBufferInfo(ISpeechAudioBufferInfoVtbl): IDispatch(IDispatchVtbl) {
    fn get_MinNotification(&mut self, MinNotification: *mut ::c_long) -> ::HRESULT,
    fn put_MinNotification(&mut self, MinNotification: ::c_long) -> ::HRESULT,
    fn get_BufferSize(&mut self, BufferSize: *mut ::c_long) -> ::HRESULT,
    fn put_BufferSize(&mut self, BufferSize: ::c_long) -> ::HRESULT,
    fn get_EventBias(&mut self, EventBias: *mut ::c_long) -> ::HRESULT,
    fn put_EventBias(&mut self, EventBias: ::c_long) -> ::HRESULT
}
);
RIDL!(
interface ISpeechAudioStatus(ISpeechAudioStatusVtbl): IDispatch(IDispatchVtbl) {
    fn get_FreeBufferSpace(&mut self, FreeBufferSpace: *mut ::c_long) -> ::HRESULT,
    fn get_NonBlockingIO(&mut self, NonBlockingIO: *mut ::c_long) -> ::HRESULT,
    fn get_State(&mut self, State: *mut SpeechAudioState) -> ::HRESULT,
    fn get_CurrentSeekPosition(&mut self, CurrentSeekPosition: *mut ::VARIANT) -> ::HRESULT,
    fn get_CurrentDevicePosition(&mut self, CurrentDevicePosition: *mut ::VARIANT) -> ::HRESULT
}
);
RIDL!(
interface ISpeechAudioFormat(ISpeechAudioFormatVtbl): IDispatch(IDispatchVtbl) {
    fn get_Type(&mut self, AudioFormat: *mut SpeechAudioFormatType) -> ::HRESULT,
    fn put_Type(&mut self, AudioFormat: SpeechAudioFormatType) -> ::HRESULT,
    fn get_Guid(&mut self, Guid: *mut ::BSTR) -> ::HRESULT,
    fn put_Guid(&mut self, Guid: ::BSTR) -> ::HRESULT,
    fn GetWaveFormatEx(&mut self, SpeechWaveFormatEx: *mut *mut ISpeechWaveFormatEx) -> ::HRESULT,
    fn SetWaveFormatEx(&mut self, SpeechWaveFormatEx: *mut ISpeechWaveFormatEx) -> ::HRESULT
}
);
RIDL!(
interface ISpeechWaveFormatEx(ISpeechWaveFormatExVtbl): IDispatch(IDispatchVtbl) {
    fn get_FormatTag(&mut self, FormatTag: *mut ::c_short) -> ::HRESULT,
    fn put_FormatTag(&mut self, FormatTag: ::c_short) -> ::HRESULT,
    fn get_Channels(&mut self, Channels: *mut ::c_short) -> ::HRESULT,
    fn put_Channels(&mut self, Channels: ::c_short) -> ::HRESULT,
    fn get_SamplesPerSec(&mut self, SamplesPerSec: *mut ::c_long) -> ::HRESULT,
    fn put_SamplesPerSec(&mut self, SamplesPerSec: ::c_long) -> ::HRESULT,
    fn get_AvgBytesPerSec(&mut self, AvgBytesPerSec: *mut ::c_long) -> ::HRESULT,
    fn put_AvgBytesPerSec(&mut self, AvgBytesPerSec: ::c_long) -> ::HRESULT,
    fn get_BlockAlign(&mut self, BlockAlign: *mut ::c_short) -> ::HRESULT,
    fn put_BlockAlign(&mut self, BlockAlign: ::c_short) -> ::HRESULT,
    fn get_BitsPerSample(&mut self, BitsPerSample: *mut ::c_short) -> ::HRESULT,
    fn put_BitsPerSample(&mut self, BitsPerSample: ::c_short) -> ::HRESULT,
    fn get_ExtraData(&mut self, ExtraData: *mut ::VARIANT) -> ::HRESULT,
    fn put_ExtraData(&mut self, ExtraData: ::VARIANT) -> ::HRESULT
}
);
