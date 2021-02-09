extern crate libc;

#[link(name="errno", kind="static")]
extern {
    pub fn errno_location() -> *mut libc::c_int;
}
