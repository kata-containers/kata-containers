RIDL!(
interface IRestrictedErrorInfo(IRestrictedErrorInfoVtbl): IUnknown(IUnknownVtbl) {
    fn GetErrorDetails(
        &mut self, description: *mut ::BSTR, error: *mut ::HRESULT,
        restrictedDescription: *mut ::BSTR, capabilitySid: *mut ::BSTR
    ) -> ::HRESULT,
    fn GetReference(&mut self, reference: *mut ::BSTR) -> ::HRESULT
}
);
