use core_foundation_sys::base::{Boolean, CFRange, CFRelease, CFTypeRef};
use core_foundation_sys::string::{
    kCFStringEncodingUTF8, CFStringGetBytes, CFStringGetCStringPtr, CFStringGetLength,
};
use core_foundation_sys::timezone::{CFTimeZoneCopySystem, CFTimeZoneGetName};

pub(crate) fn get_timezone_inner() -> Result<String, crate::GetTimezoneError> {
    unsafe { get_timezone().ok_or(crate::GetTimezoneError::OsError) }
}

#[inline]
unsafe fn get_timezone() -> Option<String> {
    // The longest name in the IANA time zone database is 25 ASCII characters long.
    const MAX_LEN: usize = 32;

    // Get system time zone, and borrow its name.
    let tz = Dropping::new(CFTimeZoneCopySystem())?;
    let name = CFTimeZoneGetName(tz.0);
    if name.is_null() {
        return None;
    }

    // If the name is encoded in UTF-8, copy it directly.
    let cstr = CFStringGetCStringPtr(name, kCFStringEncodingUTF8);
    if !cstr.is_null() {
        let cstr = std::ffi::CStr::from_ptr(cstr);
        if let Ok(name) = cstr.to_str() {
            return Some(name.to_owned());
        }
    }

    // Otherwise convert the name to UTF-8.
    let mut buf = [0; MAX_LEN];
    let mut buf_bytes = 0;
    let range = CFRange {
        location: 0,
        length: CFStringGetLength(name),
    };
    if CFStringGetBytes(
        name,
        range,
        kCFStringEncodingUTF8,
        b'\0',
        false as Boolean,
        buf.as_mut_ptr(),
        buf.len() as isize,
        &mut buf_bytes,
    ) != range.length
    {
        // Could not convert the name.
        None
    } else if !(1..MAX_LEN as isize).contains(&buf_bytes) {
        // The name should not be empty, or excessively long.
        None
    } else {
        // Convert the name to a `String`.
        let name = core::str::from_utf8(&buf[..buf_bytes as usize]).ok()?;
        Some(name.to_owned())
    }
}

struct Dropping<T>(*const T);

impl<T> Drop for Dropping<T> {
    #[inline]
    fn drop(&mut self) {
        unsafe { CFRelease(self.0 as CFTypeRef) };
    }
}

impl<T> Dropping<T> {
    #[inline]
    unsafe fn new(v: *const T) -> Option<Self> {
        if v.is_null() {
            None
        } else {
            Some(Self(v))
        }
    }
}
