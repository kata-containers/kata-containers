use sys;

bitflags! {
    /// Flags to VMAR routines
    #[repr(C)]
    pub struct VmarFlags: u32 {
        const PERM_READ          = sys::ZX_VM_FLAG_PERM_READ;
        const PERM_WRITE         = sys::ZX_VM_FLAG_PERM_WRITE;
        const PERM_EXECUTE       = sys::ZX_VM_FLAG_PERM_EXECUTE;
        const COMPACT            = sys::ZX_VM_FLAG_COMPACT;
        const SPECIFIC           = sys::ZX_VM_FLAG_SPECIFIC;
        const SPECIFIC_OVERWRITE = sys::ZX_VM_FLAG_SPECIFIC_OVERWRITE;
        const CAN_MAP_SPECIFIC   = sys::ZX_VM_FLAG_CAN_MAP_SPECIFIC;
        const CAN_MAP_READ       = sys::ZX_VM_FLAG_CAN_MAP_READ;
        const CAN_MAP_WRITE      = sys::ZX_VM_FLAG_CAN_MAP_WRITE;
        const CAN_MAP_EXECUTE    = sys::ZX_VM_FLAG_CAN_MAP_EXECUTE;
    }
}