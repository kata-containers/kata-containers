# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//apple:apple_library.bzl", "AppleLibraryAdditionalParams", "apple_library_rule_constructor_params_and_swift_providers")
load("@prelude//apple:apple_toolchain_types.bzl", "AppleToolchainInfo")
load(
    "@prelude//cxx:compile.bzl",
    "CxxSrcWithFlags",  # @unused Used as a type
)
load("@prelude//cxx:cxx_library.bzl", "cxx_library_parameterized")
load("@prelude//cxx:cxx_types.bzl", "CxxRuleProviderParams", "CxxRuleSubTargetParams")
load(
    "@prelude//cxx:linker.bzl",
    "SharedLibraryFlagOverrides",
)
load(":apple_bundle.bzl", "AppleBundlePartListConstructorParams", "get_apple_bundle_part_list")
load(":apple_bundle_destination.bzl", "AppleBundleDestination")
load(":apple_bundle_part.bzl", "AppleBundlePart", "assemble_bundle", "bundle_output")
load(":apple_bundle_types.bzl", "AppleBundleInfo")
load(":xcode.bzl", "apple_populate_xcode_attributes")

def apple_test_impl(ctx: "context") -> ["provider"]:
    xctest_bundle = bundle_output(ctx)

    test_host_app_bundle = _get_test_host_app_bundle(ctx)
    test_host_app_binary = _get_test_host_app_binary(ctx, test_host_app_bundle)

    objc_bridging_header_flags = [
        # Disable bridging header -> PCH compilation to mitigate an issue in Xcode 13 beta.
        "-disable-bridging-pch",
        "-import-objc-header",
        cmd_args(ctx.attrs.bridging_header),
    ] if ctx.attrs.bridging_header else []

    constructor_params, _, _ = apple_library_rule_constructor_params_and_swift_providers(
        ctx,
        AppleLibraryAdditionalParams(
            rule_type = "apple_test",
            extra_exported_link_flags = _get_xctest_framework_linker_flags(ctx) + _get_bundle_loader_flags(test_host_app_binary),
            extra_swift_compiler_flags = _get_xctest_framework_search_paths_flags(ctx) + objc_bridging_header_flags,
            shared_library_flags = SharedLibraryFlagOverrides(
                # When `-bundle` is used we can't use the `-install_name` args, thus we keep this field empty.
                shared_library_name_linker_flags_format = [],
                # When building Apple tests, we want to link with `-bundle` instead of `-shared` to allow
                # linking against the bundle loader.
                shared_library_flags = ["-bundle"],
            ),
            generate_sub_targets = CxxRuleSubTargetParams(
                compilation_database = False,
                headers = False,
                link_group_map = False,
                link_style_outputs = False,
            ),
            generate_providers = CxxRuleProviderParams(
                compilation_database = True,
                default = False,
                linkable_graph = False,
                link_style_outputs = False,
                merged_native_link_info = False,
                omnibus_root = False,
                preprocessors = False,
                resources = False,
                shared_libraries = False,
                template_placeholders = False,
            ),
            populate_xcode_attributes_func = lambda local_ctx, **kwargs: _xcode_populate_attributes(ctx = local_ctx, xctest_bundle = xctest_bundle, test_host_app_binary = test_host_app_binary, **kwargs),
            # We want to statically link the transitive dep graph of the apple_test()
            # which we can achieve by forcing link group linking with
            # an empty mapping (i.e., default mapping).
            force_link_group_linking = True,
        ),
    )
    cxx_library_output = cxx_library_parameterized(ctx, constructor_params)

    binary_part = AppleBundlePart(source = cxx_library_output.default_output.default, destination = AppleBundleDestination("executables"), new_name = ctx.attrs.name)
    part_list_output = get_apple_bundle_part_list(ctx, AppleBundlePartListConstructorParams(binaries = [binary_part]))
    assemble_bundle(ctx, xctest_bundle, part_list_output.parts, part_list_output.info_plist_part)

    sub_targets = cxx_library_output.sub_targets

    # If the test has a test host, add a subtarget to build the test host app bundle.
    sub_targets["test-host"] = [DefaultInfo(default_outputs = [test_host_app_bundle])] if test_host_app_bundle else [DefaultInfo()]

    # When interacting with Tpx, we just pass our various inputs via env vars,
    # since Tpx basiclaly wants structured output for this.
    env = {"XCTEST_BUNDLE": xctest_bundle}

    if test_host_app_bundle == None:
        tpx_label = "tpx:apple_test:buck2:logicTest"
    else:
        env["HOST_APP_BUNDLE"] = test_host_app_bundle
        tpx_label = "tpx:apple_test:buck2:appTest"

    labels = ctx.attrs.labels + [tpx_label]
    labels.append(tpx_label)

    return [
        DefaultInfo(default_outputs = [xctest_bundle], sub_targets = sub_targets),
        ExternalRunnerTestInfo(
            type = "custom",  # We inherit a label via the macro layer that overrides this.
            command = ["false"],  # Tpx makes up its own args, we just pass params via the env.
            env = env,
            labels = labels,
            use_project_relative_paths = True,
            run_from_project_root = True,
            contacts = ctx.attrs.contacts,
            executor_overrides = {
                "ios-simulator": CommandExecutorConfig(
                    local_enabled = False,
                    remote_enabled = True,
                    remote_execution_properties = {
                        "platform": "ios-simulator-pure-re",
                        "subplatform": "iPhone 8.iOS 15.0",
                        "xcode-version": "xcodestable",
                    },
                    remote_execution_use_case = "tpx-default",
                ),
                "static-listing": CommandExecutorConfig(local_enabled = True, remote_enabled = False),
            },
        ),
        cxx_library_output.xcode_data_info,
        cxx_library_output.cxx_compilationdb_info,
    ]

def _get_test_host_app_bundle(ctx: "context") -> ["artifact", None]:
    """ Get the bundle for the test host app, if one exists for this test. """
    if ctx.attrs.test_host_app:
        # Copy the test host app bundle into test's output directory
        original_bundle = ctx.attrs.test_host_app[AppleBundleInfo].bundle
        test_host_app_bundle = ctx.actions.declare_output(original_bundle.basename)
        ctx.actions.copy_file(test_host_app_bundle, original_bundle)
        return test_host_app_bundle

    return None

def _get_test_host_app_binary(ctx: "context", test_host_app_bundle: ["artifact", None]) -> ["cmd_args", None]:
    """ Reference to the binary with the test host app bundle, if one exists for this test. Captures the bundle as an artifact in the cmd_args. """
    if ctx.attrs.test_host_app:
        return cmd_args([test_host_app_bundle, ctx.attrs.test_host_app[AppleBundleInfo].binary_name], delimiter = "/")

    return None

def _get_bundle_loader_flags(binary: ["cmd_args", None]) -> [""]:
    if binary:
        # During linking we need to link the test shared lib against the test host binary. The
        # test host binary doesn't need to be embedded in an `apple_bundle`.
        return ["-bundle_loader", binary]

    return []

def _xcode_populate_attributes(
        ctx,
        srcs: [CxxSrcWithFlags.type],
        argsfiles_by_ext: {str.type: "artifact"},
        xctest_bundle: "artifact",
        test_host_app_binary: ["cmd_args", None],
        **_kwargs) -> {str.type: ""}:
    data = apple_populate_xcode_attributes(ctx = ctx, srcs = srcs, argsfiles_by_ext = argsfiles_by_ext, product_name = ctx.attrs.name)
    data["output"] = xctest_bundle
    if test_host_app_binary:
        data["test_host_app_binary"] = test_host_app_binary
    return data

def _get_xctest_framework_search_paths(ctx: "context") -> ("cmd_args", "cmd_args"):
    toolchain = ctx.attrs._apple_toolchain[AppleToolchainInfo]
    xctest_swiftmodule_search_path = cmd_args([toolchain.platform_path, "Developer/usr/lib"], delimiter = "/")
    xctest_framework_search_path = cmd_args([toolchain.platform_path, "Developer/Library/Frameworks"], delimiter = "/")
    return (xctest_swiftmodule_search_path, xctest_framework_search_path)

def _get_xctest_framework_search_paths_flags(ctx: "context") -> [["cmd_args", str.type]]:
    xctest_swiftmodule_search_path, xctest_framework_search_path = _get_xctest_framework_search_paths(ctx)
    return [
        "-I",
        xctest_swiftmodule_search_path,
        "-F",
        xctest_framework_search_path,
    ]

def _get_xctest_framework_linker_flags(ctx: "context") -> [["cmd_args", str.type]]:
    xctest_swiftmodule_search_path, xctest_framework_search_path = _get_xctest_framework_search_paths(ctx)
    return [
        "-L",
        xctest_swiftmodule_search_path,
        "-F",
        xctest_framework_search_path,
    ]
