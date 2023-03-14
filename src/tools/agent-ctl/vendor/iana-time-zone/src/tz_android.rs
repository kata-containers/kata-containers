use android_system_properties::AndroidSystemProperties;

// From https://android.googlesource.com/platform/ndk/+/android-4.2.2_r1.2/docs/system/libc/OVERVIEW.html
// The system property named 'persist.sys.timezone' contains the name of the current timezone.

const TIMEZONE_PROP_KEY: &str = "persist.sys.timezone";

pub(crate) fn get_timezone_inner() -> Result<String, crate::GetTimezoneError> {
    AndroidSystemProperties::new()
        .get(TIMEZONE_PROP_KEY)
        .ok_or(crate::GetTimezoneError::OsError)
}
