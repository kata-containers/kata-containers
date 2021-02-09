use crate::scope::Scope;

pub(crate) fn file_descriptor_proto_expr(_scope: &Scope) -> String {
    format!("file_descriptor_proto()")
}
