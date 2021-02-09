RIDL!(
interface IActivationFactory(IActivationFactoryVtbl): IInspectable(IInspectableVtbl) {
    fn ActivateInstance(&mut self, instance: *mut *mut ::IInspectable) -> ::HRESULT
}
);