extern crate caps;
use caps::runtime;

#[test]
fn test_ambient_supported() {
    runtime::ambient_set_supported().unwrap();
}

#[test]
fn test_all_supported() {
    assert_eq!(runtime::all_supported(), caps::all());
}
