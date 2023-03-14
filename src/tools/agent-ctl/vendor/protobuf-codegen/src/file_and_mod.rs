use rust_name::RustRelativePath;
use Customize;

#[allow(dead_code)]
pub(crate) struct FileAndMod {
    pub file: String,
    pub relative_mod: RustRelativePath,
    pub customize: Customize,
}
