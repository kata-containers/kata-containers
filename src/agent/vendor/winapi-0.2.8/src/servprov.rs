// Copyright © 2015, Connor Hilarides
// Licensed under the MIT License <LICENSE.md>
//! Mappings for the contents of servprov.h
pub type LPSERVICEPROVIDER = *mut IServiceProvider;
RIDL!(
interface IServiceProvider(IServiceProviderVtbl): IUnknown(IUnknownVtbl) {
    fn QueryService(
        &mut self, guidService: ::REFGUID, riid: ::REFIID, ppvObject: *mut *mut ::c_void
    ) -> ::HRESULT
}
);
