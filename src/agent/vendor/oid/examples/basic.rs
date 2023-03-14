use oid::prelude::*;

fn main() -> Result<(), ObjectIdentifierError> {
    // define OID as string
    let oid_string = "0.1.2.3";
    println!("OID String Test Value: {}", oid_string);

    // parse an OID from a string
    let oid = ObjectIdentifier::try_from(oid_string)?;
    println!("OID from String: {:#?}", oid);

    // encode the OID back to the same string
    let oid_string2: String = (&oid).into();
    assert_eq!(oid_string, oid_string2);
    println!("OID String Encoded Value: {}", oid_string2);

    // skip a line on output
    println!("\n");

    // define OID as bytes
    let oid_bytes = vec![0x01, 0x02, 0x03];
    println!("OID Binary Test Value: {:?}", oid_bytes);

    // parse an OID from bytes
    let oid = ObjectIdentifier::try_from(oid_bytes.clone())?;
    println!("OID from Binary: {:#?}", oid);

    // encode the OID back to the same bytes
    let oid_bytes2: Vec<u8> = (&oid).into();
    assert_eq!(oid_bytes, oid_bytes2);
    println!("OID Binary Encoded Value: {:?}", oid_bytes2);

    Ok(())
}
