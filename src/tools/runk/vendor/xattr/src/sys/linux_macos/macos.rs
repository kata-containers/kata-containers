use libc::{c_char, c_int, c_void, size_t, ssize_t, uint32_t};

const XATTR_NOFOLLOW: c_int = 0x0001;

#[inline(always)]
pub unsafe fn fremovexattr(fd: c_int, name: *const c_char) -> c_int {
    extern "C" {
        fn fremovexattr(fd: c_int, name: *const c_char, options: c_int) -> c_int;
    }
    fremovexattr(fd, name, 0)
}

#[inline(always)]
pub unsafe fn fsetxattr(
    fd: c_int,
    name: *const c_char,
    value: *const c_void,
    size: size_t,
) -> ssize_t {
    extern "C" {
        fn fsetxattr(
            fd: c_int,
            name: *const c_char,
            value: *const c_void,
            size: size_t,
            position: uint32_t,
            options: c_int,
        ) -> ssize_t;
    }
    fsetxattr(fd, name, value, size, 0, 0)
}

#[inline(always)]
pub unsafe fn fgetxattr(
    fd: c_int,
    name: *const c_char,
    value: *mut c_void,
    size: size_t,
) -> ssize_t {
    extern "C" {
        fn fgetxattr(
            fd: c_int,
            name: *const c_char,
            value: *mut c_void,
            size: size_t,
            position: uint32_t,
            options: c_int,
        ) -> ssize_t;
    }
    fgetxattr(fd, name, value, size, 0, 0)
}

#[inline(always)]
pub unsafe fn flistxattr(fd: c_int, buf: *mut c_char, size: size_t) -> ssize_t {
    extern "C" {
        fn flistxattr(fd: c_int, buf: *mut c_char, size: size_t, options: c_int) -> ssize_t;
    }
    flistxattr(fd, buf, size, 0)
}

#[inline(always)]
pub unsafe fn lremovexattr(path: *const c_char, name: *const c_char) -> c_int {
    extern "C" {
        fn removexattr(path: *const c_char, name: *const c_char, options: c_int) -> c_int;
    }
    removexattr(path, name, XATTR_NOFOLLOW)
}

#[inline(always)]
pub unsafe fn lsetxattr(
    path: *const c_char,
    name: *const c_char,
    value: *const c_void,
    size: size_t,
) -> ssize_t {
    extern "C" {
        fn setxattr(
            path: *const c_char,
            name: *const c_char,
            value: *const c_void,
            size: size_t,
            position: uint32_t,
            options: c_int,
        ) -> ssize_t;
    }
    setxattr(path, name, value, size, 0, XATTR_NOFOLLOW)
}

#[inline(always)]
pub unsafe fn lgetxattr(
    path: *const c_char,
    name: *const c_char,
    value: *mut c_void,
    size: size_t,
) -> ssize_t {
    extern "C" {
        fn getxattr(
            path: *const c_char,
            name: *const c_char,
            value: *mut c_void,
            size: size_t,
            position: uint32_t,
            options: c_int,
        ) -> ssize_t;
    }
    getxattr(path, name, value, size, 0, XATTR_NOFOLLOW)
}

#[inline(always)]
pub unsafe fn llistxattr(path: *const c_char, buf: *mut c_char, size: size_t) -> ssize_t {
    extern "C" {
        fn listxattr(
            path: *const c_char,
            buf: *mut c_char,
            size: size_t,
            options: c_int,
        ) -> ssize_t;
    }
    listxattr(path, buf, size, XATTR_NOFOLLOW)
}
