pub type PINSPECT_HSTRING_CALLBACK = Option<unsafe extern "system" fn(
    *const ::VOID, ::UINT_PTR, ::UINT32, *mut ::BYTE,
) -> ::HRESULT>;
