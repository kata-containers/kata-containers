// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
//! this ALWAYS GENERATED file contains the definitions for the interfaces
RIDL!(
interface IUnknown(IUnknownVtbl) {
    fn QueryInterface(&mut self, riid: ::REFIID, ppvObject: *mut *mut ::c_void) -> ::HRESULT,
    fn AddRef(&mut self) -> ::ULONG,
    fn Release(&mut self) -> ::ULONG
}
);
pub type LPUNKNOWN = *mut IUnknown;
RIDL!(
interface AsyncIUnknown(AsyncIUnknownVtbl): IUnknown(IUnknownVtbl) {
    fn Begin_QueryInterface(&mut self, riid: ::REFIID) -> ::HRESULT,
    fn Finish_QueryInterface(&mut self, ppvObject: *mut *mut ::c_void) -> ::HRESULT,
    fn Begin_AddRef(&mut self) -> ::HRESULT,
    fn Finish_AddRef(&mut self) -> ::ULONG,
    fn Begin_Release(&mut self) -> ::HRESULT,
    fn Finish_Release(&mut self) -> ::ULONG
}
);
RIDL!(
interface IClassFactory(IClassFactoryVtbl): IUnknown(IUnknownVtbl) {
    fn CreateInstance(
        &mut self, pUnkOuter: *mut IUnknown, riid: ::REFIID, ppvObject: *mut *mut ::c_void
    ) -> ::HRESULT,
    fn LockServer(&mut self, fLock: ::BOOL) -> ::HRESULT
}
);
