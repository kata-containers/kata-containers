current_dir := $(shell dirname $(realpath $(firstword $(MAKEFILE_LIST))))

build:
	cargo build --features="fusedev"
	cargo build --features="virtiofs"
	cargo build --features="vhost-user-fs"
	cargo build --features="fusedev,async-io"
	cargo build --features="virtiofs,async-io"
	cargo build --features="vhost-user-fs,async-io"

build-macos:
	cargo build --features="fusedev"

check-macos: build-macos
	cargo fmt -- --check
	cargo clippy --features="fusedev" -- -Dwarnings
	cargo test --features="fusedev" -- --nocapture --skip integration

check: build
	cargo fmt -- --check
	cargo clippy --features="fusedev" --no-default-features -- -Dwarnings
	cargo clippy --features="virtiofs" --no-default-features -- -Dwarnings
	cargo clippy --features="vhost-user-fs" --no-default-features -- -Dwarnings
	cargo clippy --features="fusedev,virtiofs" --no-default-features -- -Dwarnings
	cargo test --features="fusedev" --no-default-features -- --nocapture --skip integration
	cargo test --features="virtiofs" --no-default-features  -- --nocapture --skip integration
	cargo test --features="vhost-user-fs" --no-default-features -- --nocapture --skip integration
	cargo test --features="fusedev,virtiofs" --no-default-features -- --nocapture --skip integration
	cargo test --features="fusedev,async-io" --no-default-features -- --nocapture --skip integration
	cargo test --features="virtiofs,async-io" --no-default-features -- --nocapture --skip integration
	cargo test --features="vhost-user-fs,async-io" --no-default-features -- --nocapture --skip integration
	cargo test --features="fusedev,virtiofs,async-io" --no-default-features -- --nocapture --skip integration

smoke: check
	cargo test --features="fusedev" -- --nocapture

smoke-all: smoke
	cargo test --features="fusedev" -- --nocapture --ignored

smoke-macos: check-macos
	cargo test --features="fusedev" -- --nocapture

docker-smoke:
	docker run --rm --privileged -v ${current_dir}:/fuse-rs rust:1.58.1 sh -c "rustup component add clippy rustfmt; cd /fuse-rs; make smoke-all"
