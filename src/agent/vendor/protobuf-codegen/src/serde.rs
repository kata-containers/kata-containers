use code_writer::CodeWriter;
use Customize;

/// Write serde attr according to specified codegen option.
pub fn write_serde_attr(w: &mut CodeWriter, customize: &Customize, attr: &str) {
    if customize.serde_derive.unwrap_or(false) {
        w.write_line(&format!("#[cfg_attr(feature = \"with-serde\", {})]", attr));
    }
}
