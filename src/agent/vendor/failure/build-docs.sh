#!/bin/bash
mkdir public
cargo doc --no-deps
cargo install mdbook --no-default-features
mdbook build ./book
cp -r ./target/doc/ ./public
cp -r ./book/book/* ./public
find $PWD/public | grep "\.html\$"
