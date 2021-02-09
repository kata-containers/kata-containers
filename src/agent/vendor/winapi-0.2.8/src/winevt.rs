// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
//! Windows Events API
pub type EVT_HANDLE = ::HANDLE;
pub type PEVT_HANDLE = *mut ::HANDLE;
ENUM!{enum EVT_VARIANT_TYPE {
    EvtVarTypeNull = 0,
    EvtVarTypeString = 1,
    EvtVarTypeAnsiString = 2,
    EvtVarTypeSByte = 3,
    EvtVarTypeByte = 4,
    EvtVarTypeInt16 = 5,
    EvtVarTypeUInt16 = 6,
    EvtVarTypeInt32 = 7,
    EvtVarTypeUInt32 = 8,
    EvtVarTypeInt64 = 9,
    EvtVarTypeUInt64 = 10,
    EvtVarTypeSingle = 11,
    EvtVarTypeDouble = 12,
    EvtVarTypeBoolean = 13,
    EvtVarTypeBinary = 14,
    EvtVarTypeGuid = 15,
    EvtVarTypeSizeT = 16,
    EvtVarTypeFileTime = 17,
    EvtVarTypeSysTime = 18,
    EvtVarTypeSid = 19,
    EvtVarTypeHexInt32 = 20,
    EvtVarTypeHexInt64 = 21,
    EvtVarTypeEvtHandle = 32,
    EvtVarTypeEvtXml = 35,
}}
pub const EVT_VARIANT_TYPE_MASK: ::DWORD = 0x7f;
pub const EVT_VARIANT_TYPE_ARRAY: ::DWORD = 128;
STRUCT!{struct EVT_VARIANT {
    u: u64,
    Count: ::DWORD,
    Type: ::DWORD,
}}
// TODO - All the UNION! for each variant
// TODO - The rest of this header
