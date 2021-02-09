// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
STRUCT!{struct OLECMD {
    cmdID: ::ULONG,
    cmdf: ::DWORD,
}}
STRUCT!{struct OLECMDTEXT {
    cmdtextf: ::DWORD,
    cwActual: ::ULONG,
    cwBuf: ::ULONG,
    rgwz: [::wchar_t; 0],
}}
RIDL!{interface IOleCommandTarget(IOleCommandTargetVtbl): IUnknown(IUnknownVtbl) {
    fn QueryStatus(
        &mut self, pguidCmdGroup: *const ::GUID, cCmds: ::ULONG, prgCmds: *mut OLECMD,
        pCmdText: *mut OLECMDTEXT
    ) -> ::HRESULT,
    fn Exec(
        &mut self, pguidCmdGroup: *const :: GUID, nCmdID: ::DWORD, nCmdexecopt: ::DWORD,
        pvaIn: *mut ::VARIANT, pvaOut: *mut ::VARIANT
    ) -> ::HRESULT
}}
