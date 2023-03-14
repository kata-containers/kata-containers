# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//apple:apple_toolchain_types.bzl", "AppleToolchainInfo")
load(
    "@prelude//ide_integrations:xcode.bzl",
    "XCODE_DATA_SUB_TARGET",
    "generate_xcode_data",
)
load("@prelude//utils:utils.bzl", "expect", "flatten")
load(":apple_bundle_destination.bzl", "AppleBundleDestination")
load(":apple_bundle_part.bzl", "AppleBundlePart", "assemble_bundle", "bundle_output")
load(":apple_bundle_resources.bzl", "get_apple_bundle_resource_part_list", "get_is_watch_bundle")
load(":apple_bundle_types.bzl", "AppleBundleInfo", "AppleBundleResourceInfo")
load(":apple_bundle_utility.bzl", "get_bundle_min_target_version", "get_product_name")
load(":apple_dsym.bzl", "AppleDebuggableInfo", "DEBUGINFO_SUBTARGET", "DSYM_SUBTARGET")
load(":apple_sdk.bzl", "get_apple_sdk_name")
load(":xcode.bzl", "apple_xcode_data_add_xctoolchain")

INSTALL_DATA_SUB_TARGET = "install-data"
_INSTALL_DATA_FILE_NAME = "install_apple_data.json"

_PLIST = "plist"

_XCTOOLCHAIN_SUB_TARGET = "xctoolchain"

AppleBundlePartListConstructorParams = record(
    # The binaries/executables, required to create a bundle
    binaries = field([AppleBundlePart.type]),
)

AppleBundlePartListOutput = record(
    # The parts to be copied into an Apple bundle, *including* binaries
    parts = field([AppleBundlePart.type]),
    # Part that holds the info.plist
    info_plist_part = field(AppleBundlePart.type),
)

AppleBundleBinaryOutput = record(
    binary = field("artifact"),
    # In the case of watchkit, the `ctx.attrs.binary`'s not set, and we need to create a stub binary.
    is_watchkit_stub_binary = field(bool.type, False),
)

def _get_binary(ctx: "context") -> AppleBundleBinaryOutput.type:
    # No binary means we are building watchOS bundle. In v1 bundle binary is present, but its sources are empty.
    if ctx.attrs.binary == None:
        return AppleBundleBinaryOutput(
            binary = _get_watch_kit_stub_artifact(ctx),
            is_watchkit_stub_binary = True,
        )

    binary_info = ctx.attrs.binary[DefaultInfo].default_outputs
    if len(binary_info) != 1:
        fail("Expected single output artifact. Make sure the implementation of rule from `binary` attribute is correct.")
    return AppleBundleBinaryOutput(binary = binary_info[0])

def _get_binary_bundle_parts(ctx: "context", binary_output: AppleBundleBinaryOutput.type) -> [AppleBundlePart.type]:
    result = []

    if binary_output.is_watchkit_stub_binary:
        # If we're using a stub binary from watchkit, we also need to add extra part for stub.
        result.append(AppleBundlePart(source = binary_output.binary, destination = AppleBundleDestination("watchkitstub"), new_name = "WK"))
    result.append(AppleBundlePart(source = binary_output.binary, destination = AppleBundleDestination("executables"), new_name = get_product_name(ctx)))
    return result

def _get_watch_kit_stub_artifact(ctx: "context") -> "artifact":
    expect(ctx.attrs.binary == None, "Stub is useful only when binary is not set which means watchOS bundle is built.")
    stub_binary = ctx.attrs._apple_toolchain[AppleToolchainInfo].watch_kit_stub_binary
    if stub_binary == None:
        fail("Expected Watch Kit stub binary to be provided when bundle binary is not set.")
    return stub_binary

def _apple_bundle_run_validity_checks(ctx: "context"):
    if ctx.attrs.extension == None:
        fail("`extension` attribute is required")

def _get_debuggable_deps(ctx: "context") -> ["AppleDebuggableInfo"]:
    deps = ctx.attrs.deps

    # No binary means we are building watchOS bundle. In v1 bundle binary is present, but its sources are empty.
    if ctx.attrs.binary:
        deps.append(ctx.attrs.binary)

    return filter(
        None,
        [dep.get(AppleDebuggableInfo) for dep in deps],
    )

def get_apple_bundle_part_list(ctx: "context", params: AppleBundlePartListConstructorParams.type) -> AppleBundlePartListOutput.type:
    resource_part_list = None
    if hasattr(ctx.attrs, "_resource_bundle") and ctx.attrs._resource_bundle != None:
        resource_info = ctx.attrs._resource_bundle[AppleBundleResourceInfo]
        if resource_info != None:
            resource_part_list = resource_info.resource_output

    if resource_part_list == None:
        resource_part_list = get_apple_bundle_resource_part_list(ctx)

    return AppleBundlePartListOutput(
        parts = resource_part_list.resource_parts + params.binaries,
        info_plist_part = resource_part_list.info_plist_part,
    )

def apple_bundle_impl(ctx: "context") -> ["provider"]:
    _apple_bundle_run_validity_checks(ctx)

    binary_outputs = _get_binary(ctx)
    apple_bundle_part_list_output = get_apple_bundle_part_list(ctx, AppleBundlePartListConstructorParams(binaries = _get_binary_bundle_parts(ctx, binary_outputs)))

    sub_targets = {}

    debuggable_deps = _get_debuggable_deps(ctx)

    dsym_artifacts = flatten([info.dsyms for info in debuggable_deps])
    if dsym_artifacts:
        sub_targets[DSYM_SUBTARGET] = [DefaultInfo(default_outputs = dsym_artifacts)]

    external_debug_info = flatten([info.external_debug_info for info in debuggable_deps])
    sub_targets[DEBUGINFO_SUBTARGET] = [DefaultInfo(other_outputs = external_debug_info)]

    bundle = bundle_output(ctx)

    assemble_bundle(ctx, bundle, apple_bundle_part_list_output.parts, apple_bundle_part_list_output.info_plist_part)

    sub_targets[_PLIST] = [DefaultInfo(default_outputs = [apple_bundle_part_list_output.info_plist_part.source])]

    sub_targets[_XCTOOLCHAIN_SUB_TARGET] = ctx.attrs._apple_xctoolchain.providers

    # Define the xcode data sub target
    xcode_data_default_info, xcode_data_info = generate_xcode_data(ctx, "apple_bundle", bundle, _xcode_populate_attributes, processed_info_plist = apple_bundle_part_list_output.info_plist_part.source)
    sub_targets[XCODE_DATA_SUB_TARGET] = xcode_data_default_info
    install_data = generate_install_data(ctx)

    return [
        DefaultInfo(default_outputs = [bundle], sub_targets = sub_targets),
        AppleBundleInfo(bundle = bundle, binary_name = get_product_name(ctx), is_watchos = get_is_watch_bundle(ctx)),
        AppleDebuggableInfo(dsyms = dsym_artifacts, external_debug_info = external_debug_info),
        InstallInfo(
            installer = ctx.attrs._apple_installer,
            files = {
                "app_bundle": bundle,
                "options": install_data,
            },
        ),
        xcode_data_info,
    ]

def _xcode_populate_attributes(ctx, processed_info_plist: "artifact") -> {str.type: ""}:
    data = {
        "deployment_version": get_bundle_min_target_version(ctx),
        "info_plist": ctx.attrs.info_plist,
        "processed_info_plist": processed_info_plist,
        "product_name": get_product_name(ctx),
        "sdk": get_apple_sdk_name(ctx),
    }

    apple_xcode_data_add_xctoolchain(ctx, data)
    return data

def generate_install_data(
        ctx: "context",
        populate_rule_specific_attributes_func: ["function", None] = None,
        **kwargs) -> "artifact":
    data = {
        "fullyQualifiedName": ctx.label,
        ## TODO(T110665037): populate full path similar to bundle_spec.json
        "info_plist": ctx.attrs.info_plist,
        "use_idb": "true",
        ## TODO(T110665037): read from .buckconfig
        "xcode_developer_path": "/Applications/Xcode_13.4.0_fb.app/Contents/Developer",
    }

    if populate_rule_specific_attributes_func:
        data.update(populate_rule_specific_attributes_func(ctx, **kwargs))

    return ctx.actions.write_json(_INSTALL_DATA_FILE_NAME, data)
