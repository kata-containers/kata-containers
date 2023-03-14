# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

AppleToolchainInfo = provider(fields = [
    "actool",  # "RunInfo"
    "ibtool",  # "RunInfo"
    "dsymutil",  # "RunInfo"
    "dwarfdump",  # ["RunInfo", None]
    "lipo",  # "RunInfo"
    "cxx_platform_info",  # "CxxPlatformInfo"
    "cxx_toolchain_info",  # "CxxToolchainInfo"
    "codesign",  # "RunInfo"
    "codesign_allocate",  # "RunInfo"
    "codesign_identities_command",  # ["RunInfo", None]
    "compile_resources_locally",  # bool.type
    "libtool",  # "RunInfo"
    "momc",  # "RunInfo"
    "min_version",  # [None, str.type]
    "xctest",  # "RunInfo"
    "platform_path",  # [str.type, artifact]
    # SDK name to be passed to tools (e.g. actool), equivalent to ApplePlatform::getExternalName() in v1.
    "sdk_name",  # str.type
    "sdk_path",  # [str.type, artifact]
    # TODO(T124581557) Make it non-optional once there is no "selected xcode" toolchain
    "sdk_version",  # [None, str.type]
    "sdk_build_version",  # "[None, str.type]"
    "swift_toolchain_info",  # "SwiftToolchainInfo"
    "watch_kit_stub_binary",  # "artifact"
    "xcode_version",  # "[None, str.type]"
    "xcode_build_version",  # "[None, str.type]"
])

AppleToolsInfo = provider(fields = [
    "assemble_bundle",  # RunInfo
    "info_plist_processor",  # RunInfo
    "make_modulemap",  # "RunInfo"
    "make_vfsoverlay",  # "RunInfo"
    "swift_objc_header_postprocess",  # "RunInfo"
])
