# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//apple:apple_toolchain_types.bzl", "AppleToolchainInfo")
load("@prelude//apple:swift_toolchain_types.bzl", "SwiftToolchainInfo")
load("@prelude//cxx:cxx_toolchain_types.bzl", "CxxPlatformInfo", "CxxToolchainInfo")

def apple_toolchain_impl(ctx: "context") -> ["provider"]:
    sdk_path = ctx.attrs._internal_sdk_path or ctx.attrs.sdk_path
    platform_path = ctx.attrs._internal_platform_path or ctx.attrs.platform_path
    return [
        DefaultInfo(),
        AppleToolchainInfo(
            actool = ctx.attrs.actool[RunInfo],
            ibtool = ctx.attrs.ibtool[RunInfo],
            dsymutil = ctx.attrs.dsymutil[RunInfo],
            dwarfdump = ctx.attrs.dwarfdump[RunInfo] if ctx.attrs.dwarfdump else None,
            lipo = ctx.attrs.lipo[RunInfo],
            cxx_platform_info = ctx.attrs.cxx_toolchain[CxxPlatformInfo],
            cxx_toolchain_info = ctx.attrs.cxx_toolchain[CxxToolchainInfo],
            codesign = ctx.attrs.codesign[RunInfo],
            codesign_allocate = ctx.attrs.codesign_allocate[RunInfo],
            codesign_identities_command = ctx.attrs.codesign_identities_command[RunInfo] if ctx.attrs.codesign_identities_command else None,
            compile_resources_locally = ctx.attrs.compile_resources_locally,
            libtool = ctx.attrs.libtool[RunInfo],
            momc = ctx.attrs.momc[RunInfo],
            min_version = ctx.attrs.min_version,
            xctest = ctx.attrs.xctest[RunInfo],
            platform_path = platform_path,
            sdk_name = ctx.attrs.sdk_name,
            sdk_path = sdk_path,
            sdk_version = ctx.attrs.version,
            sdk_build_version = ctx.attrs.build_version,
            swift_toolchain_info = ctx.attrs.swift_toolchain[SwiftToolchainInfo] if ctx.attrs.swift_toolchain else None,
            watch_kit_stub_binary = ctx.attrs.watch_kit_stub_binary,
            xcode_version = ctx.attrs.xcode_version,
            xcode_build_version = ctx.attrs.xcode_build_version,
        ),
    ]
