#!/bin/sh

# Bump this to 1.64 (released Sep 2022) at some point. 6 months after release?
RUST_TARGET=1.47
bindgen="bindgen --no-layout-tests --blocklist-type=max_align_t --size_t-is-usize --rustified-enum=.* --use-core --rust-target $RUST_TARGET"
no_std="--ctypes-prefix libc"
experimental="-DZSTD_STATIC_LINKING_ONLY -DZDICT_STATIC_LINKING_ONLY"

run_bindgen()
{
        echo "/*
This file is auto-generated from the public API of the zstd library.
It is released under the same BSD license.

$(cat zstd/LICENSE)
*/"

    $bindgen $@
}

for NO_STD_ARG in "$no_std" ""; do
    for EXPERIMENTAL_ARG in "$experimental" ""; do
        if [ -z "$NO_STD_ARG" ]; then STD="_std"; else STD=""; fi
        if [ -z "$EXPERIMENTAL_ARG" ]; then EXPERIMENTAL=""; else EXPERIMENTAL="_experimental"; fi
        SUFFIX=${STD}${EXPERIMENTAL}
        filename=src/bindings${STD}${EXPERIMENTAL}.rs

        run_bindgen zstd.h --allowlist-type "ZSTD_.*" --allowlist-function "ZSTD_.*" --allowlist-var "ZSTD_.*" $NO_STD_ARG -- -Izstd/lib $EXPERIMENTAL_ARG > src/bindings_zstd${SUFFIX}.rs
        run_bindgen zdict.h --blocklist-type wchar_t $NO_STD_ARG -- -Izstd/lib $EXPERIMENTAL_ARG > src/bindings_zdict${SUFFIX}.rs
    done
done
