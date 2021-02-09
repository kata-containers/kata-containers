#!/bin/bash

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

cargo_test() {
   cargo test "$@" || { exit 101; }
}

test_failure_in() {
  cd $1
  cargo_test
  cargo_test --no-default-features
  cargo_test --features backtrace
  test_derive_in "$1/failure_derive"
  cd $DIR
}

test_derive_in() {
  cd $1
  cargo_test
  cd $DIR
}

test_nightly_features_in() {
  cd $1
  #cargo_test --features small-error
  cargo_test --all-features
  cd $DIR
}

main() {
  test_failure_in "$DIR/failure-1.X"
  test_failure_in "$DIR/failure-0.1.X"
  if [ "${TRAVIS_RUST_VERSION}" = "nightly" ]; then
    test_nightly_features_in "$DIR/failure-1.X"
  fi
}

main
