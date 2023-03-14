use libc::{c_char, c_int, c_void, size_t, ssize_t};

extern "C" {
    pub fn flistxattr(fd: c_int, buf: *mut c_char, size: size_t) -> ssize_t;
    pub fn fgetxattr(fd: c_int, name: *const c_char, value: *mut c_void, size: size_t) -> ssize_t;
    pub fn fremovexattr(fd: c_int, name: *const c_char) -> c_int;

    pub fn llistxattr(path: *const c_char, buf: *mut c_char, size: size_t) -> ssize_t;
    pub fn lgetxattr(
        path: *const c_char,
        name: *const c_char,
        value: *mut c_void,
        size: size_t,
    ) -> ssize_t;
    pub fn lremovexattr(path: *const c_char, name: *const c_char) -> c_int;
}

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
            flags: c_int,
        ) -> ssize_t;
    }
    fsetxattr(fd, name, value, size, 0)
}

pub unsafe fn lsetxattr(
    path: *const c_char,
    name: *const c_char,
    value: *const c_void,
    size: size_t,
) -> ssize_t {
    extern "C" {
        fn lsetxattr(
            path: *const c_char,
            name: *const c_char,
            value: *const c_void,
            size: size_t,
            flags: c_int,
        ) -> ssize_t;
    }
    lsetxattr(path, name, value, size, 0)
}
