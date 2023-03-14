# https://github.com/casey/just

fmt:
    cargo fmt --all

check: fmt
    cargo check
    cargo clippy -- -D warnings

test: check
    cargo test --all-features -- --test-threads=1 --nocapture
    cargo run --example nofile

doc:
    RUSTDOCFLAGS="--cfg docsrs" cargo +nightly doc --no-deps --open --all-features

codegen:
    python3 -m scripts.search_resource > tmp
    python3 -m scripts.replace tmp src/unix/resource.rs '// #begin-codegen' '// #end-codegen'

    python3 -m scripts.search_rlim > tmp
    python3 -m scripts.replace tmp src/unix.rs '// #begin-codegen' '// #end-codegen'

    python3 -m scripts.ident_cfg KERN_MAXFILESPERPROC 0 > tmp
    python3 -m scripts.replace tmp src/utils.rs '// #begin-codegen KERN_MAXFILESPERPROC' '// #end-codegen KERN_MAXFILESPERPROC'

    python3 -m scripts.ident_cfg RLIMIT_NOFILE 0 > tmp
    python3 -m scripts.replace tmp src/utils.rs '// #begin-codegen RLIMIT_NOFILE' '// #end-codegen RLIMIT_NOFILE'

    python3 -m scripts.ident_cfg RLIMIT_NOFILE 0 inverse > tmp
    python3 -m scripts.replace tmp src/utils.rs '// #begin-codegen not RLIMIT_NOFILE' '// #end-codegen not RLIMIT_NOFILE'

    rm tmp
    cargo fmt
