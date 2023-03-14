//! Check all target requirements. Note that SSE2 should be enabled by default.
#[cfg(not(any(target_arch = "x86_64", target_arch = "x86")))]
compile_error!("crate can only be used on x86 and x86_64 architectures");

#[cfg(all(
    feature = "ctr",
    not(all(
        target_feature = "aes",
        target_feature = "sse2",
        target_feature = "ssse3"
    )),
))]
compile_error!(
    "enable aes and ssse3 target features, e.g. with \
    RUSTFLAGS=\"-C target-feature=+aes,+ssse3\" environment variable. \
    For x86 target arch additionally enable sse2 target feature."
);

#[cfg(all(
    not(feature = "ctr"),
    not(all(target_feature = "aes", target_feature = "sse2")),
))]
compile_error!(
    "enable aes target feature, e.g. with \
    RUSTFLAGS=\"-C target-feature=+aes\" environment variable. \
    For x86 target arch additionally enable sse2 target feature."
);
