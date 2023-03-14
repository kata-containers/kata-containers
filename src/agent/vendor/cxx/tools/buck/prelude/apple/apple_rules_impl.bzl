# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//:attributes.bzl", "LinkableDepType", "Linkage")
load("@prelude//cxx:headers.bzl", "CPrecompiledHeaderInfo")
load("@prelude//cxx:omnibus.bzl", "omnibus_environment_attr")
load("@prelude//cxx/user:link_group_map.bzl", "link_group_map_attr")
load(":apple_asset_catalog.bzl", "apple_asset_catalog_impl")
load(":apple_binary.bzl", "apple_binary_impl")
load(":apple_bundle.bzl", "apple_bundle_impl")
load(":apple_code_signing_types.bzl", "CodeSignType")
load(":apple_core_data.bzl", "apple_core_data_impl")
load(":apple_library.bzl", "apple_library_impl")
load(":apple_package.bzl", "apple_package_impl")
load(":apple_resource.bzl", "apple_resource_impl")
load(
    ":apple_rules_impl_utility.bzl",
    "APPLE_ARCHIVE_OBJECTS_LOCALLY_OVERRIDE_ATTR_NAME",
    "APPLE_LINK_BINARIES_LOCALLY_OVERRIDE_ATTR_NAME",
    "APPLE_LINK_LIBRARIES_LOCALLY_OVERRIDE_ATTR_NAME",
    "apple_bundle_extra_attrs",
    "get_apple_toolchain_attr",
    "get_apple_xctoolchain_attr",
    "get_apple_xctoolchain_bundle_id_attr",
)
load(":apple_test.bzl", "apple_test_impl")
load(":apple_toolchain.bzl", "apple_toolchain_impl")
load(":apple_toolchain_types.bzl", "AppleToolsInfo")
load(":prebuilt_apple_framework.bzl", "prebuilt_apple_framework_impl")
load(":swift_toolchain.bzl", "swift_toolchain_impl")
load(":xcode_postbuild_script.bzl", "xcode_postbuild_script_impl")
load(":xcode_prebuild_script.bzl", "xcode_prebuild_script_impl")

implemented_rules = {
    "apple_asset_catalog": apple_asset_catalog_impl,
    "apple_binary": apple_binary_impl,
    "apple_bundle": apple_bundle_impl,
    "apple_library": apple_library_impl,
    "apple_package": apple_package_impl,
    "apple_resource": apple_resource_impl,
    "apple_test": apple_test_impl,
    "apple_toolchain": apple_toolchain_impl,
    "core_data_model": apple_core_data_impl,
    "prebuilt_apple_framework": prebuilt_apple_framework_impl,
    "swift_toolchain": swift_toolchain_impl,
    "xcode_postbuild_script": xcode_postbuild_script_impl,
    "xcode_prebuild_script": xcode_prebuild_script_impl,
}

extra_attributes = {
    "apple_asset_catalog": {
        "dirs": attrs.list(attrs.source(allow_directory = True), default = []),
    },
    "apple_binary": {
        "binary_linker_flags": attrs.list(attrs.arg(), default = []),
        "enable_distributed_thinlto": attrs.bool(default = False),
        "extra_xcode_sources": attrs.list(attrs.source(allow_directory = True), default = []),
        "link_group_map": link_group_map_attr(),
        "link_postprocessor": attrs.option(attrs.exec_dep(), default = None),
        "precompiled_header": attrs.option(attrs.dep(providers = [CPrecompiledHeaderInfo]), default = None),
        "prefer_stripped_objects": attrs.bool(default = False),
        "preferred_linkage": attrs.enum(Linkage, default = "any"),
        "stripped": attrs.bool(default = False),
        "_apple_toolchain": get_apple_toolchain_attr(),
        "_apple_xctoolchain": get_apple_xctoolchain_attr(),
        "_apple_xctoolchain_bundle_id": get_apple_xctoolchain_bundle_id_attr(),
        "_omnibus_environment": omnibus_environment_attr(),
        APPLE_LINK_BINARIES_LOCALLY_OVERRIDE_ATTR_NAME: attrs.option(attrs.bool(), default = None),
    },
    "apple_bundle": apple_bundle_extra_attrs(),
    "apple_library": {
        "extra_xcode_sources": attrs.list(attrs.source(allow_directory = True), default = []),
        "link_group_map": link_group_map_attr(),
        "link_postprocessor": attrs.option(attrs.exec_dep(), default = None),
        "precompiled_header": attrs.option(attrs.dep(providers = [CPrecompiledHeaderInfo]), default = None),
        "preferred_linkage": attrs.enum(Linkage, default = "any"),
        "serialize_debugging_options": attrs.bool(default = True),
        "stripped": attrs.bool(default = False),
        "use_archive": attrs.option(attrs.bool(), default = None),
        "_apple_toolchain": get_apple_toolchain_attr(),
        # FIXME: prelude// should be standalone (not refer to fbsource//)
        "_apple_tools": attrs.exec_dep(default = "fbsource//xplat/buck2/platform/apple:apple-tools", providers = [AppleToolsInfo]),
        "_apple_xctoolchain": get_apple_xctoolchain_attr(),
        "_apple_xctoolchain_bundle_id": get_apple_xctoolchain_bundle_id_attr(),
        "_omnibus_environment": omnibus_environment_attr(),
        APPLE_LINK_LIBRARIES_LOCALLY_OVERRIDE_ATTR_NAME: attrs.option(attrs.bool(), default = None),
        APPLE_ARCHIVE_OBJECTS_LOCALLY_OVERRIDE_ATTR_NAME: attrs.option(attrs.bool(), default = None),
    },
    "apple_resource": {
        "codesign_on_copy": attrs.bool(default = False),
        "content_dirs": attrs.list(attrs.source(allow_directory = True), default = []),
        "dirs": attrs.list(attrs.source(allow_directory = True), default = []),
    },
    # To build an `apple_test`, one needs to first build a shared `apple_library` then
    # wrap this test library into an `apple_bundle`. Because of this, `apple_test` has attributes
    # from both `apple_library` and `apple_bundle`.
    "apple_test": {
        # Expected by `apple_bundle`, for `apple_test` this field is always None.
        "binary": attrs.option(attrs.dep(), default = None),
        # The resulting test bundle should have .xctest extension.
        "extension": attrs.string(default = "xctest"),
        "extra_xcode_sources": attrs.list(attrs.source(allow_directory = True), default = []),
        "link_postprocessor": attrs.option(attrs.exec_dep(), default = None),
        # Used to create the shared test library. Any library deps whose `preferred_linkage` isn't "shared" will
        # be treated as "static" deps and linked into the shared test library.
        "link_style": attrs.enum(LinkableDepType, default = "static"),
        # The test source code and lib dependencies should be built into a shared library.
        "preferred_linkage": attrs.enum(Linkage, default = "shared"),
        # Expected by `apple_bundle`, for `apple_test` this field is always None.
        "resource_group": attrs.option(attrs.string(), default = None),
        # Expected by `apple_bundle`, for `apple_test` this field is always None.
        "resource_group_map": attrs.option(attrs.string(), default = None),
        "stripped": attrs.bool(default = False),
        "_apple_toolchain": get_apple_toolchain_attr(),
        # FIXME: prelude// should be standalone (not refer to fbsource//)
        "_apple_tools": attrs.exec_dep(default = "fbsource//xplat/buck2/platform/apple:apple-tools", providers = [AppleToolsInfo]),
        "_apple_xctoolchain": get_apple_xctoolchain_attr(),
        "_apple_xctoolchain_bundle_id": get_apple_xctoolchain_bundle_id_attr(),
        "_codesign_type": attrs.option(attrs.enum(CodeSignType.values()), default = None),
        "_compile_resources_locally_override": attrs.option(attrs.bool(), default = None),
        "_incremental_bundling_enabled": attrs.bool(default = False),
        "_omnibus_environment": omnibus_environment_attr(),
        "_profile_bundling_enabled": attrs.bool(default = False),
        APPLE_LINK_LIBRARIES_LOCALLY_OVERRIDE_ATTR_NAME: attrs.option(attrs.bool(), default = None),
    },
    "apple_toolchain": {
        # The Buck v1 attribute specs defines those as `attrs.source()` but
        # we want to properly handle any runnable tools that might have
        # addition runtime requirements.
        "actool": attrs.dep(providers = [RunInfo]),
        "codesign": attrs.dep(providers = [RunInfo]),
        "codesign_allocate": attrs.dep(providers = [RunInfo]),
        "codesign_identities_command": attrs.option(attrs.dep(providers = [RunInfo]), default = None),
        # Controls invocations of `ibtool`, `actool` and `momc`
        "compile_resources_locally": attrs.bool(default = False),
        "dsymutil": attrs.dep(providers = [RunInfo]),
        "dwarfdump": attrs.option(attrs.dep(providers = [RunInfo]), default = None),
        "ibtool": attrs.dep(providers = [RunInfo]),
        "libtool": attrs.dep(providers = [RunInfo]),
        "lipo": attrs.dep(providers = [RunInfo]),
        "min_version": attrs.option(attrs.string(), default = None),
        "momc": attrs.dep(providers = [RunInfo]),
        "platform_path": attrs.option(attrs.source()),  # Mark as optional until we remove `_internal_platform_path`
        "sdk_path": attrs.option(attrs.source()),  # Mark as optional until we remove `_internal_sdk_path`
        "version": attrs.option(attrs.string(), default = None),
        "xcode_build_version": attrs.option(attrs.string(), default = None),
        "xcode_version": attrs.option(attrs.string(), default = None),
        "xctest": attrs.dep(providers = [RunInfo]),
        # TODO(T111858757): Mirror of `platform_path` but treated as a string. It allows us to
        #                   pass abs paths during development and using the currently selected Xcode.
        "_internal_platform_path": attrs.option(attrs.string()),
        # TODO(T111858757): Mirror of `sdk_path` but treated as a string. It allows us to
        #                   pass abs paths during development and using the currently selected Xcode.
        "_internal_sdk_path": attrs.option(attrs.string()),
    },
    "core_data_model": {
        "path": attrs.source(allow_directory = True),
    },
    "prebuilt_apple_framework": {
        "framework": attrs.option(attrs.source(allow_directory = True), default = None),
        "preferred_linkage": attrs.enum(Linkage, default = "any"),
        "_apple_toolchain": get_apple_toolchain_attr(),
        "_omnibus_environment": omnibus_environment_attr(),
    },
    "scene_kit_assets": {
        "path": attrs.source(allow_directory = True),
    },
    "swift_library": {
        "preferred_linkage": attrs.enum(Linkage, default = "any"),
    },
    "swift_toolchain": {
        "architecture": attrs.option(attrs.string(), default = None),  # TODO(T115173356): Make field non-optional
        "platform_path": attrs.option(attrs.source()),  # Mark as optional until we remove `_internal_platform_path`
        "sdk_modules": attrs.list(attrs.dep(), default = []),  # A list or a root target that represent a graph of sdk modules (e.g Frameworks)
        "sdk_path": attrs.option(attrs.source()),  # Mark as optional until we remove `_internal_sdk_path`
        "swift_stdlib_tool": attrs.exec_dep(providers = [RunInfo]),
        "swiftc": attrs.exec_dep(providers = [RunInfo]),
        # TODO(T111858757): Mirror of `platform_path` but treated as a string. It allows us to
        #                   pass abs paths during development and using the currently selected Xcode.
        "_internal_platform_path": attrs.option(attrs.string(), default = None),
        # TODO(T111858757): Mirror of `sdk_path` but treated as a string. It allows us to
        #                   pass abs paths during development and using the currently selected Xcode.
        "_internal_sdk_path": attrs.option(attrs.string(), default = None),
        "_swiftc_wrapper": attrs.dep(providers = [RunInfo], default = "prelude//apple/tools:swift_exec"),
    },
}
