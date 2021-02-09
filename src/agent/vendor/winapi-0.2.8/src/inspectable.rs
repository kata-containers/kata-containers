// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
pub type LPINSPECTABLE = *mut IInspectable;
ENUM!{enum TrustLevel {
    BaseTrust = 0,
    PartialTrust,
    FullTrust,
}}
RIDL!(
interface IInspectable(IInspectableVtbl): IUnknown(IUnknownVtbl) {
    fn GetIids(&mut self, iidCount: *mut ::ULONG, iids: *mut *mut ::IID) -> ::HRESULT,
    fn GetRuntimeClassName(&mut self, className: *mut ::HSTRING) -> ::HRESULT,
    fn GetTrustLevel(&mut self, trustLevel: *mut TrustLevel) -> ::HRESULT
}
);
