use Customize;

/// Path to `protobuf` crate, different when `.proto` file is
/// used inside or outside of protobuf crate.
pub(crate) fn protobuf_crate_path(customize: &Customize) -> &str {
    match customize.inside_protobuf {
        Some(true) => "crate",
        _ => "::protobuf",
    }
}
