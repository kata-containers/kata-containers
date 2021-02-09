use serial_test::serial_core;

#[test]
fn test_empty_serial_call() {
    serial_core("beta", || {
        println!("Bar");
    });
}
