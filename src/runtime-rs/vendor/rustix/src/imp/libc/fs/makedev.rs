use super::super::c;
use super::Dev;

#[cfg(not(any(target_os = "android", target_os = "emscripten")))]
#[inline]
pub fn makedev(maj: u32, min: u32) -> Dev {
    unsafe { c::makedev(maj, min) }
}

#[cfg(target_os = "android")]
#[inline]
pub fn makedev(maj: u32, min: u32) -> Dev {
    // Android's `makedev` oddly has signed argument types.
    unsafe { c::makedev(maj as i32, min as i32) }
}

#[cfg(target_os = "emscripten")]
#[inline]
pub fn makedev(maj: u32, min: u32) -> Dev {
    // Emscripten's `makedev` has a 32-bit return value.
    Dev::from(unsafe { c::makedev(maj, min) })
}

#[cfg(not(any(target_os = "android", target_os = "emscripten")))]
#[inline]
pub fn major(dev: Dev) -> u32 {
    unsafe { c::major(dev) }
}

#[cfg(target_os = "android")]
#[inline]
pub fn major(dev: Dev) -> u32 {
    // Android's `major` oddly has signed return types.
    (unsafe { c::major(dev) }) as u32
}

#[cfg(target_os = "emscripten")]
#[inline]
pub fn major(dev: Dev) -> u32 {
    // Emscripten's `major` has a 32-bit argument value.
    unsafe { c::major(dev as u32) }
}

#[cfg(not(any(target_os = "android", target_os = "emscripten")))]
#[inline]
pub fn minor(dev: Dev) -> u32 {
    unsafe { c::minor(dev) }
}

#[cfg(target_os = "android")]
#[inline]
pub fn minor(dev: Dev) -> u32 {
    // Android's `minor` oddly has signed return types.
    (unsafe { c::minor(dev) }) as u32
}

#[cfg(target_os = "emscripten")]
#[inline]
pub fn minor(dev: Dev) -> u32 {
    // Emscripten's `minor` has a 32-bit argument value.
    unsafe { c::minor(dev as u32) }
}
