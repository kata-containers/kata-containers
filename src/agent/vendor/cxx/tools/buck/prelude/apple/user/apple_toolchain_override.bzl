# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//apple:apple_toolchain_types.bzl", "AppleToolchainInfo")
load("@prelude//cxx:cxx_toolchain_types.bzl", "CxxToolchainInfo")
load("@prelude//user:rule_spec.bzl", "RuleRegistrationSpec")

def _impl(ctx: "context") -> ["provider"]:
    base = ctx.attrs.base[AppleToolchainInfo]
    cxx_toolchain_override = ctx.attrs.cxx_toolchain[CxxToolchainInfo]
    return [
        DefaultInfo(),
        AppleToolchainInfo(
            actool = base.actool,
            ibtool = base.ibtool,
            dsymutil = base.dsymutil,
            dwarfdump = base.dwarfdump,
            lipo = base.lipo,
            cxx_platform_info = base.cxx_platform_info,
            cxx_toolchain_info = cxx_toolchain_override if cxx_toolchain_override != None else base.cxx_toolchain_info,
            codesign = base.codesign,
            codesign_allocate = base.codesign_allocate,
            compile_resources_locally = base.compile_resources_locally,
            libtool = base.libtool,
            momc = base.momc,
            min_version = base.min_version,
            xctest = base.xctest,
            platform_path = base.platform_path,
            sdk_name = base.sdk_name,
            sdk_path = base.sdk_path,
            sdk_version = base.sdk_version,
            sdk_build_version = base.sdk_build_version,
            swift_toolchain_info = base.swift_toolchain_info,
            watch_kit_stub_binary = base.watch_kit_stub_binary,
            xcode_version = base.xcode_version,
            xcode_build_version = base.xcode_build_version,
        ),
    ]

registration_spec = RuleRegistrationSpec(
    name = "apple_toolchain_override",
    impl = _impl,
    attrs = {
        "base": attrs.dep(providers = [AppleToolchainInfo]),
        "cxx_toolchain": attrs.dep(providers = [CxxToolchainInfo]),
    },
)
