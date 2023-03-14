#!/usr/bin/env bash
# Copyright 2021 Amazon.com, Inc. or its affiliates. All Rights Reserved.
# SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

# This script generates the syscall tables for seccompiler.

set -e

# Full path to the seccompiler tools dir.
TOOLS_DIR=$(cd "$(dirname "$0")" && pwd)

# Full path to the seccompiler sources dir.
ROOT_DIR=$(cd "${TOOLS_DIR}/.." && pwd)

# Path to the temporary linux kernel directory.
KERNEL_DIR="${ROOT_DIR}/.kernel"

test_mode=0

PATH_TO_X86_TABLE="$ROOT_DIR/src/syscall_table/x86_64.rs"
PATH_TO_AARCH64_TABLE="$ROOT_DIR/src/syscall_table/aarch64.rs"

PATH_TO_X86_TEST_TABLE="$ROOT_DIR/src/syscall_table/test_x86_64.rs"
PATH_TO_AARCH64_TEST_TABLE="$ROOT_DIR/src/syscall_table/test_aarch64.rs"

generate_syscall_list_x86_64() {
    # the table for x86_64 is nicely formatted here:
    # linux/arch/x86/entry/syscalls/syscall_64.tbl
    echo $(cat linux/arch/x86/entry/syscalls/syscall_64.tbl | grep -v "^#" | \
        grep -v -e '^$' | awk '{print $2,$3,$1}' | grep -v "^x32" | \
        awk '{print "(\""$2"\", "$3"),"}' | \
        sort -d)
}

generate_syscall_list_aarch64() {
    # filter for substituting `#define`s that point to other macros;
    # values taken from linux/include/uapi/asm-generic/unistd.h
    replace+='s/__NR3264_fadvise64/223/;'
    replace+='s/__NR3264_fcntl/25/;'
    replace+='s/__NR3264_fstatat/79/;'
    replace+='s/__NR3264_fstatfs/44/;'
    replace+='s/__NR3264_fstat/80/;'
    replace+='s/__NR3264_ftruncate/46/;'
    replace+='s/__NR3264_lseek/62/;'
    replace+='s/__NR3264_sendfile/71/;'
    replace+='s/__NR3264_statfs/43/;'
    replace+='s/__NR3264_truncate/45/;'
    replace+='s/__NR3264_mmap/222/;'

    echo "$1" > $path_to_rust_file

    # the aarch64 syscall table is not located in a .tbl file, like x86;
    # we run gcc's pre-processor to extract the numeric constants from header
    # files.
    echo $(gcc -Ilinux/include/uapi -E -dM -D__ARCH_WANT_RENAMEAT\
        -D__BITS_PER_LONG=64 linux/arch/arm64/include/uapi/asm/unistd.h |\
        grep "#define __NR_" | grep -v "__NR_syscalls" |\
        grep -v "__NR_arch_specific_syscall" | awk -F '__NR_' '{print $2}' |\
        sed $replace | awk '{ print "(\""$1"\", "$2")," }' | sort -d)
}

write_rust_syscall_table() {
    kernel_version=$1
    platform=$2
    path_to_rust_file=$3

    if [ "$platform" == "x86_64" ]; then
        syscall_list=$(generate_syscall_list_x86_64)
    elif [ "$platform" == "aarch64" ]; then
        syscall_list=$(generate_syscall_list_aarch64)
    else
        die "Invalid platform"
    fi

    echo "$(get_rust_file_header "$kernel_version")" > $path_to_rust_file

    printf "
    use std::collections::HashMap;

    pub(crate) fn make_syscall_table() -> HashMap<&'static str, i64> {
    vec![%s].into_iter().collect() }" "${syscall_list}" >> $path_to_rust_file

    rustfmt $path_to_rust_file

    echo "Generated at: $path_to_rust_file"
}

# Validate the user supplied kernel version number.
# It must be composed of 2 groups of integers separated by dot, with an
# optional third group.
validate_kernel_version() {
    local version_regex="^([0-9]+.)[0-9]+(.[0-9]+)?$"
    version="$1"

    if [ -z "$version" ]; then
        die "Version cannot be empty."
    elif [[ ! "$version" =~ $version_regex ]]; then
        die "Invalid version number: $version (expected: \$Major.\$Minor.\$Patch(optional))."
    fi

}

download_kernel() {
    kernel_version=$1
    kernel_major=v$(echo ${kernel_version} | cut -d . -f 1).x
    kernel_baseurl=https://www.kernel.org/pub/linux/kernel/${kernel_major}
    kernel_archive=linux-${kernel_version}.tar.xz

    # Create the kernel clone directory
    rm -rf "$KERNEL_DIR"
    mkdir -p "$KERNEL_DIR" || die "Error: cannot create dir $dir"
        [ -x "$KERNEL_DIR" ] && [ -w "$dir" ] || \
            {
                chmod +x+w "$KERNEL_DIR"
            } || \
            die "Error: wrong permissions for $KERNEL_DIR. Should be +x+w"

    cd "$KERNEL_DIR"

    echo "Fetching linux kernel..."

    # Get sha256 checksum.
    curl -fsSLO ${kernel_baseurl}/sha256sums.asc
    kernel_sha256=$(grep ${kernel_archive} sha256sums.asc | cut -d ' ' -f 1)
    # Get kernel archive.
    curl -fsSLO "$kernel_baseurl/$kernel_archive"
    # Verify checksum.
    echo "${kernel_sha256}  ${kernel_archive}" | sha256sum -c -
    # Decompress the kernel source.
    xz -d "${kernel_archive}"
    cat linux-${kernel_version}.tar | tar -x && \
        mv linux-${kernel_version} linux
}

run_validation() {
    # We want to regenerate the tables and compare them with the existing ones.
    # This is to validate that the tables are actually correct and were not
    # mistakenly or maliciously modified.
    arch=$1
    kernel_version=$2

    if [[ $arch == "x86_64" ]]; then
        path_to_table=$PATH_TO_X86_TABLE
        path_to_test_table=$PATH_TO_X86_TEST_TABLE
    elif [[ $arch == "aarch64" ]]; then
        path_to_table=$PATH_TO_AARCH64_TABLE
        path_to_test_table=$PATH_TO_AARCH64_TEST_TABLE
    else
        die "Invalid platform"
    fi

    download_kernel "$kernel_version"

    # Generate new tables to validate against.
    write_rust_syscall_table \
        "$kernel_version" "$arch" "$path_to_test_table"

    # Perform comparison. Tables should be identical, except for the timestamp
    # comment line.
    diff -I "\/\/ Generated on:.*" $path_to_table $path_to_test_table || {
        echo ""
        echo "Syscall table validation failed."
        echo "Make sure they haven't been mistakenly altered."
        echo ""

        exit 1
    }

    echo "Validation successful."
}

get_rust_file_header() {
    echo "$(cat <<-END
// Copyright $(date +"%Y") Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

// This file is auto-generated by \`tools/generate_syscall_tables\`.
// Do NOT manually edit!
// Generated on: $(date)
// Kernel version: $1
END
    )"
}

# Exit with an error message
die() {
    echo -e "$1" 
    exit 1
}

help() {
    echo ""
    echo "Generates the syscall tables for seccompiler, according to a given kernel version."
    echo "Release candidate (rc) linux versions are not allowed."
    echo "Outputs a rust file for each supported arch: src/seccompiler/src/syscall_table/{arch}.rs"
    echo "Supported architectures: x86_64 and aarch64."
    echo ""
    echo "If passed the --test flag, it will validate that the generated syscall tables"
    echo "are correct by regenerating them and comparing the results."
    echo ""
}

cleanup () {
    rm -rf $KERNEL_DIR

    if [[ $test_mode -eq 1 ]]; then
        rm -rf $PATH_TO_X86_TEST_TABLE
        rm -rf $PATH_TO_AARCH64_TEST_TABLE
    fi
}

parse_cmdline() {
    # Parse command line args.
    while [ $# -gt 0 ]; do
        case "$1" in
            "-h"|"--help")      { help; exit 1;    } ;;
            "--test")           { test_mode=1; break;  } ;;
            *)                  { kernel_version="$1"; } ;;
        esac
        shift
    done
}

test() {
    # Run the validation for x86_64.
    echo "Validating table for x86_64..."

    kernel_version_x86_64=$(cat $PATH_TO_X86_TABLE | \
        awk -F '// Kernel version:' '{print $2}' | xargs)
    
    validate_kernel_version "$kernel_version_x86_64"
    
    run_validation "x86_64" "$kernel_version_x86_64"

    # Run the validation for aarch64.
    echo "Validating table for aarch64..."

    kernel_version_aarch64=$(cat $PATH_TO_AARCH64_TABLE | \
        awk -F '// Kernel version:' '{print $2}' | xargs)
    
    validate_kernel_version "$kernel_version_aarch64"
    
    run_validation "aarch64" "$kernel_version_aarch64"
}

main() {
    if [[ $test_mode -eq 1 ]]; then
        # When in test mode, re-generate the tables according to the version
        # from the rust files and validate that they are identical.
        test
    else
        # When not in test mode, we only want to re-generate the tables.

        validate_kernel_version "$kernel_version"
        download_kernel "$kernel_version"

        # generate syscall table for x86_64
        echo "Generating table for x86_64..."
        write_rust_syscall_table \
                "$kernel_version" "x86_64" "$PATH_TO_X86_TABLE"

        # generate syscall table for aarch64
        echo "Generating table for aarch64..."
        write_rust_syscall_table \
                "$kernel_version" "aarch64" "$PATH_TO_AARCH64_TABLE"
    fi
}

# Setup a cleanup trap on exit.
trap cleanup EXIT

parse_cmdline $@

main
