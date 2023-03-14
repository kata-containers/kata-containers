# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//apple:apple_dsym.bzl", "AppleDebuggableInfo", "DEBUGINFO_SUBTARGET", "DSYM_SUBTARGET", "get_apple_dsym")
load("@prelude//apple:apple_stripping.bzl", "apple_strip_args")
load("@prelude//apple:swift_compilation.bzl", "compile_swift", "get_swift_dependency_info", "get_swift_pcm_compile_info")
load("@prelude//cxx:cxx.bzl", "get_srcs_with_flags")
load("@prelude//cxx:cxx_library.bzl", "cxx_library_parameterized")
load("@prelude//cxx:cxx_library_utility.bzl", "cxx_attr_deps", "cxx_attr_exported_deps")
load("@prelude//cxx:cxx_types.bzl", "CxxRuleAdditionalParams", "CxxRuleConstructorParams", "CxxRuleProviderParams", "CxxRuleSubTargetParams")
load("@prelude//cxx:headers.bzl", "cxx_attr_exported_headers")
load(
    "@prelude//cxx:linker.bzl",
    "SharedLibraryFlagOverrides",
)
load(
    "@prelude//cxx:preprocessor.bzl",
    "CPreprocessor",
)
load("@prelude//linking:link_info.bzl", "LinkStyle")
load(":apple_bundle_types.bzl", "AppleMinDeploymentVersionInfo")
load(":apple_frameworks.bzl", "get_framework_search_path_flags")
load(":apple_link_postprocessor.bzl", "get_apple_link_postprocessor")
load(":apple_modular_utility.bzl", "MODULE_CACHE_PATH")
load(":apple_target_sdk_version.bzl", "get_min_deployment_version_for_node", "get_min_deployment_version_target_linker_flags", "get_min_deployment_version_target_preprocessor_flags")
load(":apple_utility.bzl", "get_apple_cxx_headers_layout", "get_module_name")
load(":modulemap.bzl", "preprocessor_info_for_modulemap")
load(":resource_groups.bzl", "create_resource_graph")
load(":xcode.bzl", "apple_populate_xcode_attributes")

AppleLibraryAdditionalParams = record(
    # Name of the top level rule utilizing the apple_library rule.
    rule_type = str.type,
    # Extra flags to be passed to the linker.
    extra_exported_link_flags = field(["_arglike"], []),
    # Extra flags to be passed to the Swift compiler.
    extra_swift_compiler_flags = field(["_arglike"], []),
    # Linker flags that tell the linker to create shared libraries, overriding the default shared library flags.
    # e.g. when building Apple tests, we want to link with `-bundle` instead of `-shared` to allow
    # linking against the bundle loader.
    shared_library_flags = field([SharedLibraryFlagOverrides.type, None], None),
    # Function to use for setting Xcode attributes for the Xcode data sub target.
    populate_xcode_attributes_func = field("function", apple_populate_xcode_attributes),
    # Define which sub targets to generate.
    generate_sub_targets = field(CxxRuleSubTargetParams.type, CxxRuleSubTargetParams()),
    # Define which providers to generate.
    generate_providers = field(CxxRuleProviderParams.type, CxxRuleProviderParams()),
    # Forces link group linking logic, even when there's no mapping. Link group linking
    # without a mapping is equivalent to statically linking the whole transitive dep graph.
    force_link_group_linking = field(bool.type, False),
)

def apple_library_impl(ctx: "context") -> ["provider"]:
    constructor_params, swift_providers, exported_pre = apple_library_rule_constructor_params_and_swift_providers(ctx, AppleLibraryAdditionalParams(rule_type = "apple_library"))

    resource_graph = create_resource_graph(
        ctx = ctx,
        labels = ctx.attrs.labels,
        deps = cxx_attr_deps(ctx),
        exported_deps = cxx_attr_exported_deps(ctx),
    )

    output = cxx_library_parameterized(ctx, constructor_params)
    swift_pcm_compile = get_swift_pcm_compile_info(ctx, output.propagated_exported_preprocessor_info, exported_pre)

    providers = output.providers + [resource_graph] + swift_providers + ([swift_pcm_compile] if swift_pcm_compile else [])
    return providers

def apple_library_rule_constructor_params_and_swift_providers(ctx: "context", params: AppleLibraryAdditionalParams.type) -> (CxxRuleConstructorParams.type, ["provider"], [CPreprocessor.type, None]):
    cxx_srcs, swift_srcs = _filter_swift_srcs(ctx)

    # First create a modulemap if necessary. This is required for importing
    # ObjC code in Swift so must be done before Swift compilation.
    exported_hdrs = cxx_attr_exported_headers(ctx, get_apple_cxx_headers_layout(ctx))
    if (ctx.attrs.modular or swift_srcs) and exported_hdrs:
        modulemap_pre = preprocessor_info_for_modulemap(ctx, "exported", exported_hdrs, None)
    else:
        modulemap_pre = None

    swift_compile = compile_swift(ctx, swift_srcs, exported_hdrs, modulemap_pre, params.extra_swift_compiler_flags)
    swift_object_files = swift_compile.object_files if swift_compile else []

    swift_pre = CPreprocessor()
    if swift_compile:
        # If we have Swift we export the extended modulemap that includes
        # the ObjC exported headers and the -Swift.h header.
        exported_pre = swift_compile.exported_pre

        # We also include the -Swift.h header to this libraries preprocessor
        # info, so that we can import it unprefixed in this module.
        swift_pre = swift_compile.pre
    elif modulemap_pre:
        # Otherwise if this library is modular we export a modulemap of
        # the ObjC exported headers.
        exported_pre = modulemap_pre
    else:
        exported_pre = None

    swift_providers = swift_compile.providers if swift_compile else [get_swift_dependency_info(ctx, exported_pre, None)]
    swift_argsfile = swift_compile.swift_argsfile if swift_compile else None

    modular_pre = CPreprocessor(
        uses_modules = ctx.attrs.uses_modules,
        modular_args = [
            "-fcxx-modules",
            "-fmodules",
            "-fmodule-name=" + get_module_name(ctx),
            "-fmodules-cache-path=" + MODULE_CACHE_PATH,
            # TODO(T123756899): We have to use this hack to make compilation work
            # when Clang modules are enabled and using toolchains. That's because
            # resource-dir is passed as a relative path (so that no abs paths appear
            # in any .pcm). The compiler will then expand and generate #include paths
            # that won't work unless we have the directive below.
            "-I.",
        ],
    )

    framework_search_path_pre = CPreprocessor(
        args = [get_framework_search_path_flags(ctx)],
    )
    return CxxRuleConstructorParams(
        rule_type = params.rule_type,
        is_test = (params.rule_type == "apple_test"),
        headers_layout = get_apple_cxx_headers_layout(ctx),
        extra_exported_link_flags = params.extra_exported_link_flags,
        extra_link_flags = [_get_linker_flags(ctx, swift_providers)],
        extra_link_input = swift_object_files,
        extra_preprocessors = get_min_deployment_version_target_preprocessor_flags(ctx) + [framework_search_path_pre, swift_pre, modular_pre],
        extra_exported_preprocessors = filter(None, [exported_pre]),
        srcs = cxx_srcs,
        additional = CxxRuleAdditionalParams(
            srcs = swift_srcs,
            argsfiles = [swift_argsfile] if swift_argsfile else [],
            # We need to add any swift modules that we include in the link, as
            # these will end up as `N_AST` entries that `dsymutil` will need to
            # follow.
            external_debug_info = [_get_transitive_swiftmodule_paths(swift_providers)],
        ),
        link_style_sub_targets_and_providers_factory = _get_shared_link_style_sub_targets_and_providers,
        shared_library_flags = params.shared_library_flags,
        # apple_library's 'stripped' arg only applies to shared subtargets, or,
        # targets with 'preferred_linkage = "shared"'
        strip_executable = ctx.attrs.stripped,
        strip_args_factory = apple_strip_args,
        force_link_group_linking = params.force_link_group_linking,
        cxx_populate_xcode_attributes_func = lambda local_ctx, **kwargs: _xcode_populate_attributes(ctx = local_ctx, swift_argsfile = swift_argsfile, populate_xcode_attributes_func = params.populate_xcode_attributes_func, **kwargs),
        generate_sub_targets = params.generate_sub_targets,
        generate_providers = params.generate_providers,
        link_postprocessor = get_apple_link_postprocessor(ctx),
    ), swift_providers, exported_pre

def _filter_swift_srcs(ctx: "context") -> (["CxxSrcWithFlags"], ["CxxSrcWithFlags"]):
    cxx_srcs = []
    swift_srcs = []
    for s in get_srcs_with_flags(ctx):
        if s.file.extension == ".swift":
            swift_srcs.append(s)
        else:
            cxx_srcs.append(s)

    return cxx_srcs, swift_srcs

def _get_shared_link_style_sub_targets_and_providers(
        link_style: LinkStyle.type,
        ctx: "context",
        executable: "artifact",
        external_debug_info: ["_arglike"],
        _dwp: ["artifact", None]) -> ({str.type: ["provider"]}, ["provider"]):
    if link_style != LinkStyle("shared"):
        return ({}, [])

    min_version = get_min_deployment_version_for_node(ctx)
    min_version_providers = [AppleMinDeploymentVersionInfo(version = min_version)] if min_version != None else []

    dsym_artifact = get_apple_dsym(
        ctx = ctx,
        executable = executable,
        external_debug_info = external_debug_info,
        action_identifier = executable.short_path,
    )
    return ({
        DSYM_SUBTARGET: [DefaultInfo(default_outputs = [dsym_artifact])],
        DEBUGINFO_SUBTARGET: [DefaultInfo(other_outputs = external_debug_info)],
    }, [AppleDebuggableInfo(dsyms = [dsym_artifact], external_debug_info = external_debug_info)] + min_version_providers)

def _get_transitive_swiftmodule_paths(swift_providers: ["provider"]) -> "cmd_args":
    cmd = cmd_args()
    for p in swift_providers:
        if hasattr(p, "transitive_swiftmodule_paths"):
            cmd.add(p.transitive_swiftmodule_paths.project_as_args("hidden"))
    return cmd

def _get_linker_flags(ctx: "context", swift_providers: ["provider"]) -> "cmd_args":
    cmd = cmd_args(get_min_deployment_version_target_linker_flags(ctx))
    for p in swift_providers:
        if hasattr(p, "transitive_swiftmodule_paths"):
            cmd.add(p.transitive_swiftmodule_paths.project_as_args("linker_args"))

    return cmd

def _xcode_populate_attributes(
        ctx,
        srcs: ["CxxSrcWithFlags"],
        argsfiles_by_ext: {str.type: "artifact"},
        swift_argsfile: ["CxxAdditionalArgsfileParams", None],
        populate_xcode_attributes_func: "function",
        **_kwargs) -> {str.type: ""}:
    if swift_argsfile:
        argsfiles_by_ext[swift_argsfile.extension] = swift_argsfile.file

    data = populate_xcode_attributes_func(ctx, srcs = srcs, argsfiles_by_ext = argsfiles_by_ext, product_name = ctx.attrs.name)
    return data
