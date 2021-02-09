#!/usr/bin/env bash

set -eu

declare -a targets=(
	"x86_64-apple-darwin"
	"x86_64-unknown-freebsd"
	"x86_64-unknown-linux-gnu"
	"x86_64-pc-windows-gnu"
)

for target in "${targets[@]}"; do
	cargo check --target "$target" --all-targets --examples --bins --tests --no-default-features
	cargo check --target "$target" --all-targets --examples --bins --tests --all-features
done
