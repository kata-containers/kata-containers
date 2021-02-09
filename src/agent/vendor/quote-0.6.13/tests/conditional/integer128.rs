#[test]
fn test_integer128() {
    let ii128 = -1i128;
    let uu128 = 1u128;

    let tokens = quote! {
        #ii128 #uu128
    };
    let expected = "-1i128 1u128";
    assert_eq!(expected, tokens.to_string());
}
