// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
//! this ALWAYS GENERATED file contains the definitions for the interfaces
#[repr(C)] #[allow(missing_copy_implementations)]
pub struct SHITEMID {
    pub cb: ::USHORT,
    pub abID: [::BYTE; 0],
}
pub type LPSHITEMID = *mut SHITEMID;
pub type LPCSHITEMID = *const SHITEMID;
#[repr(C)]
pub struct ITEMIDLIST {
    pub mkid: SHITEMID,
}
pub type ITEMIDLIST_RELATIVE = ITEMIDLIST;
pub type ITEMID_CHILD = ITEMIDLIST;
pub type ITEMIDLIST_ABSOLUTE = ITEMIDLIST;
pub type LPITEMIDLIST = *mut ITEMIDLIST;
pub type LPCITEMIDLIST = *const ITEMIDLIST;
pub type PIDLIST_ABSOLUTE = *mut ITEMIDLIST_ABSOLUTE;
pub type PCIDLIST_ABSOLUTE = *const ITEMIDLIST_ABSOLUTE;
pub type PCUIDLIST_ABSOLUTE = *const ITEMIDLIST_ABSOLUTE;
pub type PIDLIST_RELATIVE = *mut ITEMIDLIST_RELATIVE;
pub type PCIDLIST_RELATIVE = *const ITEMIDLIST_RELATIVE;
pub type PUIDLIST_RELATIVE = *mut ITEMIDLIST_RELATIVE;
pub type PCUIDLIST_RELATIVE = *const ITEMIDLIST_RELATIVE;
pub type PITEMID_CHILD = *mut ITEMID_CHILD;
pub type PCITEMID_CHILD = *const ITEMID_CHILD;
pub type PUITEMID_CHILD = *mut ITEMID_CHILD;
pub type PCUITEMID_CHILD = *const ITEMID_CHILD;
pub type PCUITEMID_CHILD_ARRAY = *const PCUITEMID_CHILD;
pub type PCUIDLIST_RELATIVE_ARRAY = *const PCUIDLIST_RELATIVE;
pub type PCIDLIST_ABSOLUTE_ARRAY = *const PCIDLIST_ABSOLUTE;
pub type PCUIDLIST_ABSOLUTE_ARRAY = *const PCUIDLIST_ABSOLUTE;
STRUCT!{struct COMDLG_FILTERSPEC {
    pszName: ::LPCWSTR,
    pszSpec: ::LPCWSTR,
}}
pub type KNOWNFOLDERID = ::GUID;
pub type REFKNOWNFOLDERID = *const KNOWNFOLDERID;
