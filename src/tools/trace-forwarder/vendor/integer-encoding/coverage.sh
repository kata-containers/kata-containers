#!/bin/bash

set -x

KCOV=kcov
KCOV_OPTS="--exclude-pattern=/.cargo,/glibc,/usr/lib,/usr/include"
KCOV_OUT="./kcov-out/"

export RUSTFLAGS="-C link-dead-code"

TEST_BIN=$(cargo test 2>&1 > /dev/null | awk '/^     Running target\/debug\/deps\// { print $2 }')

echo $TEST_BIN
${KCOV} ${KCOV_OPTS} ${KCOV_OUT} ${TEST_BIN} && xdg-open ${KCOV_OUT}/index.html
