# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//:paths.bzl", "paths")
load(":apple_bundle_destination.bzl", "AppleBundleDestination", "bundle_relative_path_for_destination")
load(":apple_bundle_utility.bzl", "get_extension_attr", "get_product_name")
load(":apple_code_signing_types.bzl", "AppleEntitlementsInfo", "CodeSignType")
load(":apple_sdk.bzl", "get_apple_sdk_name")
load(":apple_sdk_metadata.bzl", "get_apple_sdk_metadata_for_sdk_name")
load(":apple_toolchain_types.bzl", "AppleToolchainInfo", "AppleToolsInfo")

# Defines where and what should be copied into
AppleBundlePart = record(
    # A file of directory which content should be copied
    source = field("artifact"),
    # Where the source should be copied, the actual destination directory
    # inside bundle depends on target platform
    destination = AppleBundleDestination.type,
    # New file name if it should be renamed before copying.
    # Empty string value is applicable only when `source` is a directory,
    # in such case only content of the directory will be copied, as opposed to the directory itself.
    # When value is `None`, directory or file will be copied as it is, without renaming.
    new_name = field([str.type, None], None),
    # Marks parts which should be code signed separately from the whole bundle.
    codesign_on_copy = field(bool.type, False),
)

def bundle_output(ctx: "context") -> "artifact":
    bundle_dir_name = _get_bundle_dir_name(ctx)
    output = ctx.actions.declare_output(bundle_dir_name)
    return output

def assemble_bundle(ctx: "context", bundle: "artifact", parts: [AppleBundlePart.type], info_plist_part: [AppleBundlePart.type, None]) -> None:
    all_parts = parts + [info_plist_part] if info_plist_part else []
    spec_file = _bundle_spec_json(ctx, all_parts)

    tools = ctx.attrs._apple_tools[AppleToolsInfo]
    tool = tools.assemble_bundle

    codesign_args = []
    codesign_type = _detect_codesign_type(ctx)

    if codesign_type.value in ["distribution", "adhoc"]:
        codesign_args = [
            "--codesign",
            "--codesign-tool",
            ctx.attrs._apple_toolchain[AppleToolchainInfo].codesign,
        ]

        external_name = get_apple_sdk_name(ctx)
        platform_args = ["--platform", external_name]
        codesign_args.extend(platform_args)

        if codesign_type.value != "adhoc":
            provisioning_profiles = ctx.attrs._provisioning_profiles[DefaultInfo]
            provisioning_profiles_args = ["--profiles-dir"] + provisioning_profiles.default_outputs
            codesign_args.extend(provisioning_profiles_args)

            # TODO(T116604880): use `apple.use_entitlements_when_adhoc_code_signing` buckconfig value
            maybe_entitlements = _entitlements_file(ctx)
            entitlements_args = ["--entitlements", maybe_entitlements] if maybe_entitlements else []
            codesign_args.extend(entitlements_args)

            identities_command = ctx.attrs._apple_toolchain[AppleToolchainInfo].codesign_identities_command
            identities_command_args = ["--codesign-identities-command", cmd_args(identities_command)] if identities_command else []
            codesign_args.extend(identities_command_args)
        else:
            codesign_args.append("--ad-hoc")

        info_plist_args = [
            "--info-plist-source",
            info_plist_part.source,
            "--info-plist-destination",
            _bundle_relative_destination_path(ctx, info_plist_part),
        ] if info_plist_part else []
        codesign_args.extend(info_plist_args)
    elif codesign_type.value == "skip":
        pass
    else:
        fail("Code sign type `{}` not supported".format(codesign_type))

    command = cmd_args([
        tool,
        "--output",
        bundle.as_output(),
        "--spec",
        spec_file,
    ] + codesign_args)
    command.hidden([part.source for part in all_parts])
    run_incremental_args = {}
    incremental_state = ctx.actions.declare_output("incremental_state.json").as_output()

    # Fallback to value from buckconfig
    incremental_bundling_enabled = ctx.attrs.incremental_bundling_enabled or ctx.attrs._incremental_bundling_enabled

    if incremental_bundling_enabled:
        command.add("--incremental-state", incremental_state)
        run_incremental_args = {
            "metadata_env_var": "ACTION_METADATA",
            "metadata_path": "action_metadata.json",
            "no_outputs_cleanup": True,
        }
        category = "apple_assemble_bundle_incremental"
    else:
        # overwrite file with incremental state so if previous and next builds are incremental
        # (as opposed to the current non-incremental one), next one won't assume there is a
        # valid incremental state.
        command.hidden(ctx.actions.write_json(incremental_state, {}))
        category = "apple_assemble_bundle"

    if ctx.attrs._profile_bundling_enabled:
        profile_output = ctx.actions.declare_output("bundling_profile.txt").as_output()
        command.add("--profile-output", profile_output)

    force_local_bundling = codesign_type.value != "skip"
    ctx.actions.run(
        command,
        local_only = force_local_bundling,
        prefer_local = not force_local_bundling,
        category = category,
        **run_incremental_args
    )

def _get_bundle_dir_name(ctx: "context") -> str.type:
    return paths.replace_extension(get_product_name(ctx), "." + get_extension_attr(ctx))

def _bundle_relative_destination_path(ctx: "context", part: AppleBundlePart.type) -> str.type:
    bundle_relative_path = bundle_relative_path_for_destination(part.destination, get_apple_sdk_name(ctx), ctx.attrs.extension)
    destination_file_or_directory_name = part.new_name if part.new_name != None else paths.basename(part.source.short_path)
    return paths.join(bundle_relative_path, destination_file_or_directory_name)

# Returns JSON to be passed into bundle assembling tool. It should contain a dictionary which maps bundle relative destination paths to source paths."
def _bundle_spec_json(ctx: "context", parts: [AppleBundlePart.type]) -> "artifact":
    specs = []

    for part in parts:
        part_spec = {
            "dst": _bundle_relative_destination_path(ctx, part),
            "src": part.source,
        }
        if part.codesign_on_copy:
            part_spec["codesign_on_copy"] = True
        specs.append(part_spec)

    return ctx.actions.write_json("bundle_spec.json", specs)

def _detect_codesign_type(ctx: "context") -> CodeSignType.type:
    if ctx.attrs.extension not in ["app", "appex"]:
        # Only code sign application bundles and extensions
        return CodeSignType("skip")

    if ctx.attrs._codesign_type:
        return CodeSignType(ctx.attrs._codesign_type)
    sdk_name = get_apple_sdk_name(ctx)
    is_ad_hoc_sufficient = get_apple_sdk_metadata_for_sdk_name(sdk_name).is_ad_hoc_code_sign_sufficient
    return CodeSignType("adhoc" if is_ad_hoc_sufficient else "distribution")

def _entitlements_file(ctx: "context") -> ["artifact", None]:
    if not ctx.attrs.binary:
        return None

    # The `binary` attribute can be either an apple_binary or a dynamic library from apple_library
    binary_entitlement_info = ctx.attrs.binary[AppleEntitlementsInfo]
    if binary_entitlement_info and binary_entitlement_info.entitlements_file:
        return binary_entitlement_info.entitlements_file

    return ctx.attrs._codesign_entitlements
