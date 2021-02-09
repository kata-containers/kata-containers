// Copyright © 2015, skdltmxn
// Licensed under the MIT License <LICENSE.md>
//! Microsoft Internet Security Catalog API Prototypes and Definitions
STRUCT!{struct CRYPTCATSTORE {
    cbStruct: ::DWORD,
    dwPublicVersion: ::DWORD,
    pwszP7File: ::LPWSTR,
    hProv: ::HCRYPTPROV,
    dwEncodingType: ::DWORD,
    fdwStoreFlags: ::DWORD,
    hReserved: ::HANDLE,
    hAttrs: ::HANDLE,
    hCryptMsg: ::HCRYPTMSG,
    hSorted: ::HANDLE,
}}
STRUCT!{struct CRYPTCATMEMBER {
    cbStruct: ::DWORD,
    pwszReferenceTag: ::LPWSTR,
    pwszFileName: ::LPWSTR,
    gSubjectType: ::GUID,
    fdwMemberFlags: ::DWORD,
    pIndirectData: *mut ::SIP_INDIRECT_DATA,
    dwCertVersion: ::DWORD,
    dwReserved: ::DWORD,
    hReserved: ::HANDLE,
    sEncodedIndirectData: ::CRYPT_ATTR_BLOB,
    sEncodedMemberInfo: ::CRYPT_ATTR_BLOB,
}}
