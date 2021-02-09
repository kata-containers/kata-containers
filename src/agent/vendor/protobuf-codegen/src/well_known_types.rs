static NAMES: &'static [&'static str] = &[
    "Any",
    "Api",
    "BoolValue",
    "BytesValue",
    "DoubleValue",
    "Duration",
    "Empty",
    "Enum",
    "EnumValue",
    "Field",
    // TODO: dotted names
    "Field.Cardinality",
    "Field.Kind",
    "FieldMask",
    "FloatValue",
    "Int32Value",
    "Int64Value",
    "ListValue",
    "Method",
    "Mixin",
    "NullValue",
    "Option",
    "SourceContext",
    "StringValue",
    "Struct",
    "Syntax",
    "Timestamp",
    "Type",
    "UInt32Value",
    "UInt64Value",
    "Value",
];

fn is_well_known_type(name: &str) -> bool {
    NAMES.iter().any(|&n| n == name)
}

pub fn is_well_known_type_full(name: &str) -> Option<&str> {
    if let Some(dot) = name.rfind('.') {
        if &name[..dot] == ".google.protobuf" && is_well_known_type(&name[dot + 1..]) {
            Some(&name[dot + 1..])
        } else {
            None
        }
    } else {
        None
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_is_well_known_type_full() {
        assert_eq!(
            Some("BoolValue"),
            is_well_known_type_full(".google.protobuf.BoolValue")
        );
        assert_eq!(None, is_well_known_type_full(".google.protobuf.Fgfg"));
    }
}
