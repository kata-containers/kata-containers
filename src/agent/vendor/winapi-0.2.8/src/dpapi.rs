// Copyright © 2015, skdltmxn
// Licensed under the MIT License <LICENSE.md>
//! Data Protection API Prototypes and Definitions
pub const szFORCE_KEY_PROTECTION: &'static str = "ForceKeyProtection";
STRUCT!{struct CRYPTPROTECT_PROMPTSTRUCT {
    cbSize: ::DWORD,
    dwPromptFlags: ::DWORD,
    hwndApp: ::HWND,
    szPrompt: ::LPCWSTR,
}}
pub type PCRYPTPROTECT_PROMPTSTRUCT = *mut CRYPTPROTECT_PROMPTSTRUCT;
