# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//:paths.bzl", "paths")
load(
    "@prelude//:resources.bzl",
    "ResourceInfo",
    "gather_resources",
)
load(
    "@prelude//android:android_providers.bzl",
    "merge_android_packageable_info",
)
load(
    "@prelude//apple:apple_frameworks.bzl",
    "build_link_args_with_deduped_framework_flags",
    "create_frameworks_linkable",
    "get_frameworks_link_info_by_deduping_link_infos",
)
load(
    "@prelude//ide_integrations:xcode.bzl",
    "XCODE_DATA_SUB_TARGET",
    "XcodeDataInfo",
    "generate_xcode_data",
)
load(
    "@prelude//java:java_providers.bzl",
    "get_java_packaging_info",
)
load(
    "@prelude//linking:link_groups.bzl",
    "LinkGroupLib",  # @unused Used as a type
    "gather_link_group_libs",
    "merge_link_group_lib_info",
)
load(
    "@prelude//linking:link_info.bzl",
    "ArchiveLinkable",
    "LinkArgs",
    "LinkInfo",
    "LinkInfos",
    "LinkStyle",
    "Linkage",
    "LinkedObject",  # @unused Used as a type
    "ObjectsLinkable",
    "SharedLibLinkable",
    "create_merged_link_info",
    "get_actual_link_style",
    "get_link_args",
    "get_link_styles_for_linkage",
    "unpack_link_args",
    "wrap_link_info",
)
load(
    "@prelude//linking:linkable_graph.bzl",
    "AnnotatedLinkableRoot",
    "LinkableRootInfo",
    "create_linkable_graph",
    "create_linkable_graph_node",
    "create_linkable_node",
    "get_linkable_graph_node_map_func",
    "linkable_deps",
)
load("@prelude//linking:shared_libraries.bzl", "SharedLibraryInfo", "create_shared_libraries", "merge_shared_libraries")
load("@prelude//linking:strip.bzl", "strip_debug_info")
load(
    "@prelude//utils:utils.bzl",
    "expect",
    "flatten",
    "value_or",
)
load(":archive.bzl", "make_archive")
load(
    ":comp_db.bzl",
    "CxxCompilationDbInfo",
    "create_compilation_database",
    "make_compilation_db_info",
)
load(
    ":compile.bzl",
    "CxxCompileCommandOutputForCompDb",
    "compile_cxx",
    "create_compile_cmds",
)
load(":cxx_context.bzl", "get_cxx_platform_info", "get_cxx_toolchain_info")
load(
    ":cxx_library_utility.bzl",
    "ARGSFILES_SUBTARGET",
    "cxx_attr_deps",
    "cxx_attr_exported_deps",
    "cxx_attr_exported_linker_flags",
    "cxx_attr_exported_post_linker_flags",
    "cxx_attr_link_style",
    "cxx_attr_linker_flags",
    "cxx_attr_preferred_linkage",
    "cxx_attr_resources",
    "cxx_inherited_link_info",
    "cxx_is_gnu",
    "cxx_mk_shlib_intf",
    "cxx_platform_supported",
    "cxx_use_link_groups",
    "cxx_use_shlib_intfs",
)
load(
    ":cxx_types.bzl",
    "CxxRuleConstructorParams",  # @unused Used as a type
)
load(
    ":link.bzl",
    "cxx_link_into_shared_library",
    "cxx_link_shared_library",
)
load(
    ":link_groups.bzl",
    "LINK_GROUP_MAP_DATABASE_SUB_TARGET",
    "get_filtered_labels_to_links_map",
    "get_filtered_links",
    "get_filtered_targets",
    "get_link_group",
    "get_link_group_info",
    "get_link_group_map_json",
    "get_link_group_preferred_linkage",
)
load(
    ":linker.bzl",
    "get_default_shared_library_name",
    "get_ignore_undefined_symbols_flags",
    "get_shared_library_name",
    "get_shared_library_name_for_param",
)
load(
    ":omnibus.bzl",
    "create_linkable_root",
)
load(":platform.bzl", "cxx_by_platform")
load(
    ":preprocessor.bzl",
    "CPreprocessor",  # @unused Used as a type
    "CPreprocessorForTestsInfo",
    "CPreprocessorInfo",  # @unused Used as a type
    "cxx_exported_preprocessor_info",
    "cxx_inherited_preprocessor_infos",
    "cxx_merge_cpreprocessors",
    "cxx_private_preprocessor_info",
)

# An output of a `cxx_library`, consisting of required `default` artifact and optional
# `other` outputs that should also be materialized along with it.
_CxxLibraryOutput = record(
    default = field("artifact"),
    # The object files used to create the artifact in `default`.
    object_files = field(["artifact"], []),
    # Additional outputs that are implicitly used along with the above output
    # (e.g. external object files referenced by a thin archive).
    #
    # Note: It's possible that this can contain some of the artifacts which are
    # also present in object_files.
    other = field(["artifact"], []),
    # Additional debug info which is not included in the library output.
    external_debug_info = field(["_arglike"], []),
    # A shared shared library may have an associated dwp file with
    # its corresponding DWARF debug info.
    # May be None when Split DWARF is disabled, for static/static-pic libraries,
    # for some types of synthetic link objects or for pre-built shared libraries.
    dwp = field(["artifact", None], None),
)

# The outputs of either archiving or linking the outputs of the library
_CxxAllLibraryOutputs = record(
    # The outputs for each type of link style.
    outputs = field({LinkStyle.type: [_CxxLibraryOutput.type, None]}),
    # The link infos that are part of each output based on link style.
    libraries = field({LinkStyle.type: LinkInfos.type}),
    # Shared object name to shared library mapping.
    solibs = field({str.type: LinkedObject.type}),
)

# The output of compiling all the source files in the library, containing
# the commands use to compile them and all the object file variants.
_CxxCompiledSourcesOutput = record(
    # Compile commands used to compile the source files ot generate object files
    compile_cmds = field(CxxCompileCommandOutputForCompDb.type),
    # Non-PIC object files
    objects = field([["artifact"], None]),
    # Externally referenced debug info, which doesn't get linked with the
    # object (e.g. the above `.o` when using `-gsplit-dwarf=single` or the
    # the `.dwo` when using `-gsplit-dwarf=split`).
    external_debug_info = field([["artifact"], None]),
    objects_have_external_debug_info = field(bool.type, False),
    # PIC Object files
    pic_objects = field([["artifact"], None]),
    pic_objects_have_external_debug_info = field(bool.type, False),
    pic_external_debug_info = field([["artifact"], None]),
    # Non-PIC object files stripped of debug information
    stripped_objects = field([["artifact"], None]),
    # PIC object files stripped of debug information
    stripped_pic_objects = field([["artifact"], None]),
)

# The outputs of a cxx_library_parameterized rule.
_CxxLibraryParameterizedOutput = record(
    # The default output of a cxx library rule
    default_output = field([_CxxLibraryOutput.type, None], None),
    # The other outputs available
    all_outputs = field([_CxxAllLibraryOutputs.type, None], None),
    # Any generated sub targets as requested by impl_params
    sub_targets = field({str.type: ["provider"]}),
    # Any generated providers as requested by impl_params
    providers = field(["provider"]),
    # XcodeDataInfo provider, returned separately as we cannot check
    # provider type from providers above
    xcode_data_info = field([XcodeDataInfo.type, None], None),
    # CxxCompilationDbInfo provider, returned separately as we cannot check
    # provider type from providers above
    cxx_compilationdb_info = field([CxxCompilationDbInfo.type, None], None),
    # LinkableRootInfo provider, same as above.
    linkable_root = field([LinkableRootInfo.type, None], None),
    # This provider contains exported and propagated preprocessors.
    propagated_exported_preprocessor_info = field(["CPreprocessorInfo", None], None),
)

def cxx_library_parameterized(ctx: "context", impl_params: "CxxRuleConstructorParams") -> _CxxLibraryParameterizedOutput.type:
    """
    Defines the outputs for a cxx library, return the default output and any subtargets and providers based upon the requested params.
    """

    if not cxx_platform_supported(ctx):
        sub_targets = {}

        # Needed to handle cases of the named output (e.g. [static-pic]) being called directly.
        for link_style in get_link_styles_for_linkage(cxx_attr_preferred_linkage(ctx)):
            sub_targets[link_style.value.replace("_", "-")] = [DefaultInfo(default_outputs = [])]

        return _CxxLibraryParameterizedOutput(sub_targets = sub_targets, providers = [DefaultInfo(default_outputs = [], sub_targets = sub_targets)])

    non_exported_deps = cxx_attr_deps(ctx)
    exported_deps = cxx_attr_exported_deps(ctx)

    # TODO(T110378095) right now we implement reexport of exported_* flags manually, we should improve/automate that in the macro layer

    # Gather preprocessor inputs.
    (own_non_exported_preprocessor_info, test_preprocessor_infos) = cxx_private_preprocessor_info(
        ctx = ctx,
        headers_layout = impl_params.headers_layout,
        extra_preprocessors = impl_params.extra_preprocessors,
        non_exported_deps = non_exported_deps,
        is_test = impl_params.is_test,
    )
    own_exported_preprocessor_info = cxx_exported_preprocessor_info(ctx, impl_params.headers_layout, impl_params.extra_exported_preprocessors)
    own_preprocessors = [own_non_exported_preprocessor_info, own_exported_preprocessor_info] + test_preprocessor_infos

    inherited_non_exported_preprocessor_infos = cxx_inherited_preprocessor_infos(
        non_exported_deps + filter(None, [ctx.attrs.precompiled_header]),
    )
    inherited_exported_preprocessor_infos = cxx_inherited_preprocessor_infos(exported_deps)

    preferred_linkage = cxx_attr_preferred_linkage(ctx)

    compiled_srcs = cxx_compile_srcs(
        ctx = ctx,
        impl_params = impl_params,
        own_preprocessors = own_preprocessors,
        inherited_non_exported_preprocessor_infos = inherited_non_exported_preprocessor_infos,
        inherited_exported_preprocessor_infos = inherited_exported_preprocessor_infos,
        preferred_linkage = preferred_linkage,
    )

    sub_targets = {}
    providers = []

    if len(ctx.attrs.tests) > 0 and impl_params.generate_providers.preprocessor_for_tests:
        providers.append(
            CPreprocessorForTestsInfo(
                test_names = [test_target.name for test_target in ctx.attrs.tests],
                own_non_exported_preprocessor = own_non_exported_preprocessor_info,
            ),
        )

    if impl_params.generate_sub_targets.argsfiles:
        sub_targets[ARGSFILES_SUBTARGET] = [compiled_srcs.compile_cmds.source_commands.argsfiles_info]

    # Compilation DB.
    if impl_params.generate_sub_targets.compilation_database:
        comp_db = create_compilation_database(ctx, compiled_srcs.compile_cmds.comp_db_commands.src_compile_cmds)
        sub_targets["compilation-database"] = [comp_db]
    comp_db_info = None
    if impl_params.generate_providers.compilation_database:
        comp_db_info = make_compilation_db_info(compiled_srcs.compile_cmds.comp_db_commands.src_compile_cmds, get_cxx_toolchain_info(ctx), get_cxx_platform_info(ctx))
        providers.append(comp_db_info)

    # Link Groups
    link_group = get_link_group(ctx)
    link_group_info = get_link_group_info(ctx)

    if link_group_info:
        link_groups = link_group_info.groups
        link_group_mappings = link_group_info.mappings
        link_group_deps = [mapping.root.node.linkable_graph for group in link_group_info.groups for mapping in group.mappings]
        link_group_libs = gather_link_group_libs(
            deps = non_exported_deps + exported_deps,
        )
        providers.append(link_group_info)
    else:
        link_groups = []
        link_group_mappings = {}
        link_group_deps = []
        link_group_libs = {}
    link_group_preferred_linkage = get_link_group_preferred_linkage(link_groups)

    # Create the linkable graph from the library's deps, exported deps and any link group deps.
    linkable_graph_deps = non_exported_deps + exported_deps
    deps_linkable_graph = create_linkable_graph(
        ctx,
        deps = linkable_graph_deps,
        children = link_group_deps,
    )

    frameworks_linkable = create_frameworks_linkable(ctx)
    shared_links, link_group_map = _get_shared_library_links(
        ctx,
        get_linkable_graph_node_map_func(deps_linkable_graph),
        link_group,
        link_group_mappings,
        link_group_preferred_linkage,
        link_group_libs,
        exported_deps,
        non_exported_deps,
        impl_params.force_link_group_linking,
        frameworks_linkable,
    )
    if impl_params.generate_sub_targets.link_group_map and link_group_map:
        sub_targets[LINK_GROUP_MAP_DATABASE_SUB_TARGET] = [link_group_map]

    library_outputs = _form_library_outputs(
        ctx = ctx,
        impl_params = impl_params,
        compiled_srcs = compiled_srcs,
        preferred_linkage = preferred_linkage,
        shared_links = shared_links,
        extra_static_linkables = [frameworks_linkable] if frameworks_linkable else [],
    )

    actual_link_style = get_actual_link_style(cxx_attr_link_style(ctx), preferred_linkage)

    # Output sub-targets for all link-styles.
    if impl_params.generate_sub_targets.link_style_outputs or impl_params.generate_providers.link_style_outputs:
        actual_link_style_providers = []
        for link_style, output in library_outputs.outputs.items():
            if output == None:
                continue

            link_style_sub_targets, link_style_providers = impl_params.link_style_sub_targets_and_providers_factory(
                link_style,
                ctx,
                output.default,
                output.external_debug_info,
                output.dwp,
            )

            if impl_params.generate_sub_targets.link_style_outputs:
                sub_targets[link_style.value.replace("_", "-")] = [DefaultInfo(
                    default_outputs = [output.default],
                    other_outputs = output.other,
                    sub_targets = link_style_sub_targets,
                )] + (link_style_providers if link_style_providers else [])

                if link_style == actual_link_style:
                    # If we have additional providers for the current link style,
                    # add them to the list of default providers
                    actual_link_style_providers += link_style_providers

                    if impl_params.generate_sub_targets.link_style_outputs:
                        # In addition, ensure any subtargets for the active link style
                        # can be accessed as a default subtarget
                        for link_style_sub_target_name, link_style_sub_target_providers in link_style_sub_targets.items():
                            sub_targets[link_style_sub_target_name] = link_style_sub_target_providers

        if impl_params.generate_providers.link_style_outputs:
            providers += actual_link_style_providers

    # Create the default ouput for the library rule given it's link style and preferred linkage
    default_output = library_outputs.outputs[actual_link_style]

    # Define the xcode data sub target
    xcode_data_info = None
    if impl_params.generate_sub_targets.xcode_data:
        xcode_data_default_info, xcode_data_info = generate_xcode_data(
            ctx,
            rule_type = impl_params.rule_type,
            output = default_output.default if default_output else None,
            populate_rule_specific_attributes_func = impl_params.cxx_populate_xcode_attributes_func,
            srcs = impl_params.srcs + impl_params.additional.srcs,
            argsfiles_by_ext = compiled_srcs.compile_cmds.source_commands.argsfile_by_ext,
            product_name = get_default_cxx_library_product_name(ctx),
        )
        sub_targets[XCODE_DATA_SUB_TARGET] = xcode_data_default_info
        providers.append(xcode_data_info)

    # Gather link inputs.
    inherited_non_exported_link = cxx_inherited_link_info(ctx, non_exported_deps)
    inherited_exported_link = cxx_inherited_link_info(ctx, exported_deps)

    # Propagate link info provider.
    if impl_params.generate_providers.merged_native_link_info or impl_params.generate_providers.template_placeholders:
        merged_native_link_info = create_merged_link_info(
            ctx,
            # Add link info for each link style,
            library_outputs.libraries,
            preferred_linkage = preferred_linkage,
            # Export link info from non-exported deps (when necessary).
            deps = [inherited_non_exported_link],
            # Export link info from out (exported) deps.
            exported_deps = [inherited_exported_link],
            frameworks_linkable = frameworks_linkable,
        )
        if impl_params.generate_providers.merged_native_link_info:
            providers.append(merged_native_link_info)

    # Propagate shared libraries up the tree.
    if impl_params.generate_providers.shared_libraries:
        providers.append(merge_shared_libraries(
            ctx.actions,
            create_shared_libraries(ctx, library_outputs.solibs),
            filter(None, [x.get(SharedLibraryInfo) for x in non_exported_deps]) +
            filter(None, [x.get(SharedLibraryInfo) for x in exported_deps]),
        ))

    propagated_preprocessor_merge_list = inherited_exported_preprocessor_infos
    if _attr_reexport_all_header_dependencies(ctx):
        propagated_preprocessor_merge_list = inherited_non_exported_preprocessor_infos + propagated_preprocessor_merge_list
    propagated_preprocessor = cxx_merge_cpreprocessors(ctx, [own_exported_preprocessor_info], propagated_preprocessor_merge_list)
    if impl_params.generate_providers.preprocessors:
        providers.append(propagated_preprocessor)

    # For v1's `#headers` functionality.
    if impl_params.generate_sub_targets.headers:
        sub_targets["headers"] = [propagated_preprocessor]

    # Omnibus root provider.
    linkable_root = None
    if impl_params.generate_providers.omnibus_root:
        if impl_params.use_soname:
            soname = _soname(ctx)
        else:
            soname = None
        linker_type = get_cxx_toolchain_info(ctx).linker_info.type
        linkable_root = create_linkable_root(
            ctx,
            name = soname,
            link_infos = LinkInfos(
                default = LinkInfo(
                    pre_flags = cxx_attr_exported_linker_flags(ctx),
                    post_flags = cxx_attr_exported_post_linker_flags(ctx),
                    linkables = [ObjectsLinkable(
                        objects = compiled_srcs.pic_objects,
                        linker_type = linker_type,
                        link_whole = True,
                    )],
                    external_debug_info = (
                        compiled_srcs.pic_external_debug_info +
                        (compiled_srcs.pic_objects if compiled_srcs.pic_objects_have_external_debug_info else []) +
                        impl_params.additional.external_debug_info
                    ),
                ),
                stripped = LinkInfo(
                    pre_flags = cxx_attr_exported_linker_flags(ctx),
                    post_flags = cxx_attr_exported_post_linker_flags(ctx),
                    linkables = [ObjectsLinkable(
                        objects = compiled_srcs.stripped_pic_objects,
                        linker_type = linker_type,
                        link_whole = True,
                    )],
                ),
            ),
            deps = non_exported_deps + exported_deps,
            graph = deps_linkable_graph,
            create_shared_root = impl_params.is_omnibus_root or impl_params.force_emit_omnibus_shared_root,
        )
        providers.append(linkable_root)

        if linkable_root.shared_root:
            sub_targets["omnibus-shared-root"] = [DefaultInfo(
                default_outputs = [linkable_root.shared_root.product.shared_library.output],
            )]

    # Augment and provide the linkable graph.
    if impl_params.generate_providers.linkable_graph:
        roots = {}
        if linkable_root != None and impl_params.is_omnibus_root:
            roots[ctx.label] = AnnotatedLinkableRoot(root = linkable_root)

        merged_linkable_graph = create_linkable_graph(
            ctx,
            node = create_linkable_graph_node(
                ctx,
                linkable_node = create_linkable_node(
                    ctx = ctx,
                    preferred_linkage = preferred_linkage,
                    deps = non_exported_deps,
                    exported_deps = exported_deps,
                    # If we don't have link input for this link style, we pass in `None` so
                    # that omnibus knows to avoid it.
                    link_infos = library_outputs.libraries,
                    shared_libs = library_outputs.solibs,
                ),
                roots = roots,
                excluded = {ctx.label: None} if not value_or(ctx.attrs.supports_merged_linking, True) else {},
            ),
            children = [deps_linkable_graph],
        )
        providers.append(merged_linkable_graph)

    # C++ resource.
    if impl_params.generate_providers.resources:
        providers.append(ResourceInfo(resources = gather_resources(
            label = ctx.label,
            resources = cxx_attr_resources(ctx),
            deps = non_exported_deps + exported_deps,
        )))

    if impl_params.generate_providers.template_placeholders:
        templ_vars = {}

        # Some rules, e.g. fbcode//thrift/lib/cpp:thrift-core-module
        # define preprocessor flags as things like: -DTHRIFT_PLATFORM_CONFIG=<thrift/facebook/PlatformConfig.h>
        # and unless they get quoted, they break shell syntax.
        cxx_preprocessor_flags = cmd_args()
        cxx_compiler_info = get_cxx_toolchain_info(ctx).cxx_compiler_info
        cxx_preprocessor_flags.add(cmd_args(cxx_compiler_info.preprocessor_flags or [], quote = "shell"))
        cxx_preprocessor_flags.add(cmd_args(propagated_preprocessor.set.project_as_args("args"), quote = "shell"))
        cxx_preprocessor_flags.add(propagated_preprocessor.set.project_as_args("include_dirs"))
        templ_vars["cxxppflags"] = cxx_preprocessor_flags

        c_preprocessor_flags = cmd_args()
        c_compiler_info = get_cxx_toolchain_info(ctx).c_compiler_info
        c_preprocessor_flags.add(cmd_args(c_compiler_info.preprocessor_flags or [], quote = "shell"))
        c_preprocessor_flags.add(cmd_args(propagated_preprocessor.set.project_as_args("args"), quote = "shell"))
        c_preprocessor_flags.add(propagated_preprocessor.set.project_as_args("include_dirs"))
        templ_vars["cppflags"] = c_preprocessor_flags

        # Add in ldflag macros.
        for link_style in (LinkStyle("static"), LinkStyle("static_pic")):
            name = "ldflags-" + link_style.value.replace("_", "-")
            args = cmd_args()
            linker_info = get_cxx_toolchain_info(ctx).linker_info
            args.add(linker_info.linker_flags or [])
            args.add(unpack_link_args(
                get_link_args(
                    merged_native_link_info,  # buildifier: disable=uninitialized Initialized if impl_params.generate_providers.template_placeholders is True
                    link_style,
                ),
            ))
            templ_vars[name] = args

        # TODO(T110378127): To implement `$(ldflags-shared ...)` properly, we'd need
        # to setup a symink tree rule for all transitive shared libs.  Since this
        # currently would be pretty costly (O(N^2)?), and since it's not that
        # commonly used anyway, just use `static-pic` instead.  Longer-term, once
        # v1 is gone, macros that use `$(ldflags-shared ...)` (e.g. Haskell's
        # hsc2hs) can move to a v2 rules-based API to avoid needing this macro.
        templ_vars["ldflags-shared"] = templ_vars["ldflags-static-pic"]

        providers.append(TemplatePlaceholderInfo(keyed_variables = templ_vars))

    # It is possible (e.g. in a java binary or an Android APK) to have C++ libraries that depend
    # upon Java libraries (through JNI). In some cases those Java libraries are not depended upon
    # anywhere else, so we need to expose them here to ensure that they are packaged into the
    # final binary.
    if impl_params.generate_providers.java_packaging_info:
        providers.append(get_java_packaging_info(ctx, non_exported_deps + exported_deps))

    # TODO(T107163344) this shouldn't be in cxx_library itself, use overlays to remove it.
    if impl_params.generate_providers.android_packageable_info:
        providers.append(merge_android_packageable_info(ctx.label, ctx.actions, non_exported_deps + exported_deps))

    if impl_params.generate_providers.default:
        providers.append(DefaultInfo(
            default_outputs = [default_output.default] if default_output != None else [],
            other_outputs = default_output.other if default_output != None else [],
            sub_targets = sub_targets,
        ))

    # Propagate all transitive link group lib roots up the tree, so that the
    # final executable can use them.
    if impl_params.generate_providers.merged_native_link_info:
        providers.append(
            merge_link_group_lib_info(
                label = ctx.label,
                name = link_group,
                shared_libs = library_outputs.solibs,
                shared_link_infos = library_outputs.libraries.get(LinkStyle("shared")),
                deps = exported_deps + non_exported_deps,
            ),
        )

    # Propagated_exported_preprocessor_info is used for pcm compilation, which isn't possible for non-modular targets.
    propagated_exported_preprocessor_info = propagated_preprocessor if impl_params.rule_type == "apple_library" and ctx.attrs.modular else None

    return _CxxLibraryParameterizedOutput(
        default_output = default_output,
        all_outputs = library_outputs,
        sub_targets = sub_targets,
        providers = providers,
        xcode_data_info = xcode_data_info,
        cxx_compilationdb_info = comp_db_info,
        linkable_root = linkable_root,
        propagated_exported_preprocessor_info = propagated_exported_preprocessor_info,
    )

def get_default_cxx_library_product_name(ctx) -> str.type:
    preferred_linkage = cxx_attr_preferred_linkage(ctx)
    link_style = get_actual_link_style(cxx_attr_link_style(ctx), preferred_linkage)
    if link_style in (LinkStyle("static"), LinkStyle("static_pic")):
        return _base_static_library_name(ctx, False)
    else:
        return _soname(ctx)

def cxx_compile_srcs(
        ctx: "context",
        impl_params: CxxRuleConstructorParams.type,
        own_preprocessors: [CPreprocessor.type],
        inherited_non_exported_preprocessor_infos: [CPreprocessorInfo.type],
        inherited_exported_preprocessor_infos: [CPreprocessorInfo.type],
        preferred_linkage: Linkage.type) -> _CxxCompiledSourcesOutput.type:
    """
    Compile objects we'll need for archives and shared libraries.
    """

    # Create the commands and argsfiles to use for compiling each source file
    compile_cmd_output = create_compile_cmds(
        ctx = ctx,
        impl_params = impl_params,
        own_preprocessors = own_preprocessors,
        inherited_preprocessor_infos = inherited_non_exported_preprocessor_infos + inherited_exported_preprocessor_infos,
    )

    # Define object files.
    objects = None
    objects_have_external_debug_info = False
    external_debug_info = None
    stripped_objects = []
    pic_cxx_outs = compile_cxx(ctx, compile_cmd_output.source_commands.src_compile_cmds, pic = True)
    pic_objects = [out.object for out in pic_cxx_outs]
    pic_objects_have_external_debug_info = any([out.object_has_external_debug_info for out in pic_cxx_outs])
    pic_external_debug_info = [out.external_debug_info for out in pic_cxx_outs if out.external_debug_info != None]
    stripped_pic_objects = _strip_objects(ctx, pic_objects)
    if preferred_linkage != Linkage("shared"):
        cxx_outs = compile_cxx(ctx, compile_cmd_output.source_commands.src_compile_cmds, pic = False)
        objects = [out.object for out in cxx_outs]
        objects_have_external_debug_info = any([out.object_has_external_debug_info for out in cxx_outs])
        external_debug_info = [out.external_debug_info for out in cxx_outs if out.external_debug_info != None]
        stripped_objects = _strip_objects(ctx, objects)

    # Add in additional objects, after setting up stripped objects.
    pic_objects += impl_params.extra_link_input
    stripped_pic_objects += impl_params.extra_link_input
    if preferred_linkage != Linkage("shared"):
        objects += impl_params.extra_link_input
        stripped_objects += impl_params.extra_link_input

    return _CxxCompiledSourcesOutput(
        compile_cmds = compile_cmd_output,
        objects = objects,
        objects_have_external_debug_info = objects_have_external_debug_info,
        external_debug_info = external_debug_info,
        pic_objects = pic_objects,
        pic_objects_have_external_debug_info = pic_objects_have_external_debug_info,
        pic_external_debug_info = pic_external_debug_info,
        stripped_objects = stripped_objects,
        stripped_pic_objects = stripped_pic_objects,
    )

def _form_library_outputs(
        ctx: "context",
        impl_params: CxxRuleConstructorParams.type,
        compiled_srcs: _CxxCompiledSourcesOutput.type,
        preferred_linkage: Linkage.type,
        shared_links: LinkArgs.type,
        extra_static_linkables: ["FrameworksLinkable"]) -> _CxxAllLibraryOutputs.type:
    # Build static/shared libs and the link info we use to export them to dependents.
    outputs = {}
    libraries = {}
    solibs = {}

    # Add in exported linker flags.
    def ldflags(inner: LinkInfo.type) -> LinkInfo.type:
        return wrap_link_info(
            inner = inner,
            pre_flags = cxx_attr_exported_linker_flags(ctx),
            post_flags = cxx_attr_exported_post_linker_flags(ctx),
        )

    # We don't know which outputs consumers may want, so we must define them all.
    for link_style in get_link_styles_for_linkage(preferred_linkage):
        output = None
        stripped = None
        info = None

        # Generate the necessary libraries and
        # add them to the exported link info.
        if link_style in (LinkStyle("static"), LinkStyle("static_pic")):
            # Only generate an archive if we have objects to include
            if compiled_srcs.objects or compiled_srcs.pic_objects:
                pic = _use_pic(link_style)
                output, info = _static_library(
                    ctx,
                    impl_params,
                    compiled_srcs.pic_objects if pic else compiled_srcs.objects,
                    objects_have_external_debug_info = compiled_srcs.pic_objects_have_external_debug_info if pic else compiled_srcs.objects_have_external_debug_info,
                    external_debug_info = (
                        (compiled_srcs.pic_external_debug_info if pic else compiled_srcs.external_debug_info) +
                        impl_params.additional.external_debug_info
                    ),
                    pic = pic,
                    stripped = False,
                    extra_linkables = extra_static_linkables,
                )
                _, stripped = _static_library(
                    ctx,
                    impl_params,
                    compiled_srcs.stripped_pic_objects if pic else compiled_srcs.stripped_objects,
                    pic = pic,
                    stripped = True,
                    extra_linkables = extra_static_linkables,
                )
        else:  # shared
            # We still generate a shared library with no source objects because it can still point to dependencies.
            # i.e. a rust_python_extension is an empty .so depending on a rust shared object
            if compiled_srcs.pic_objects or impl_params.build_empty_so:
                soname, shlib, info = _shared_library(
                    ctx,
                    impl_params,
                    compiled_srcs.pic_objects,
                    (compiled_srcs.pic_external_debug_info +
                     (compiled_srcs.pic_objects if compiled_srcs.pic_objects_have_external_debug_info else []) +
                     impl_params.additional.external_debug_info),
                    shared_links,
                )
                output = _CxxLibraryOutput(
                    default = shlib.output,
                    object_files = compiled_srcs.pic_objects,
                    external_debug_info = shlib.external_debug_info,
                    dwp = shlib.dwp,
                )
                solibs[soname] = shlib

        # you cannot link against header only libraries so create an empty link info
        info = info if info != None else LinkInfo()
        outputs[link_style] = output
        libraries[link_style] = LinkInfos(
            default = ldflags(info),
            stripped = ldflags(stripped) if stripped != None else None,
        )

    return _CxxAllLibraryOutputs(
        outputs = outputs,
        libraries = libraries,
        solibs = solibs,
    )

def _strip_objects(ctx: "context", objects: ["artifact"]) -> ["artifact"]:
    """
    Return new objects with debug info stripped.
    """

    # Stripping is not supported on Windows
    linker_type = get_cxx_toolchain_info(ctx).linker_info.type
    if linker_type == "windows":
        return objects

    outs = []

    for obj in objects:
        base, ext = paths.split_extension(obj.short_path)
        expect(ext == ".o")
        outs.append(strip_debug_info(ctx, base + ".stripped.o", obj))

    return outs

def _get_shared_library_links(
        ctx: "context",
        linkable_graph_node_map_func,
        link_group: [str.type, None],
        link_group_mappings: [{"label": str.type}, None],
        link_group_preferred_linkage: {"label": Linkage.type},
        link_group_libs: {str.type: LinkGroupLib.type},
        exported_deps: ["dependency"],
        non_exported_deps: ["dependency"],
        force_link_group_linking,
        frameworks_linkable: ["FrameworksLinkable", None]) -> ("LinkArgs", [DefaultInfo.type, None]):
    """
    TODO(T110378116): Omnibus linking always creates shared libraries by linking
    against shared dependencies. This is not true for link groups and possibly
    other forms of shared libraries. Ideally we consolidate this logic and
    propagate up only the expected links. Until we determine the comprehensive
    logic here, simply diverge behavior on whether link groups are defined.
    """

    # If we're not filtering for link groups, link against the shared dependencies
    if not link_group_mappings and not force_link_group_linking:
        link = cxx_inherited_link_info(ctx, dedupe(flatten([non_exported_deps, exported_deps])))

        # Even though we're returning the shared library links, we must still
        # respect the `link_style` attribute of the target which controls how
        # all deps get linked. For example, you could be building the shared
        # output of a library which has `link_style = "static"`.
        #
        # The fallback equivalent code in Buck v1 is in CxxLibraryFactor::createBuildRule()
        # where link style is determined using the `linkableDepType` variable.
        link_style_value = ctx.attrs.link_style if ctx.attrs.link_style != None else "shared"

        # Note if `static` link style is requested, we assume `static_pic`
        # instead, so that code in the shared library can be correctly
        # loaded in the address space of any process at any address.
        link_style_value = "static_pic" if link_style_value == "static" else link_style_value

        return build_link_args_with_deduped_framework_flags(
            ctx,
            link,
            frameworks_linkable,
            LinkStyle(link_style_value),
        ), None

    # Else get filtered link group links
    prefer_stripped = cxx_is_gnu(ctx) and ctx.attrs.prefer_stripped_objects
    link_style = cxx_attr_link_style(ctx) if cxx_attr_link_style(ctx) != LinkStyle("static") else LinkStyle("static_pic")
    filtered_labels_to_links_map = get_filtered_labels_to_links_map(
        linkable_graph_node_map_func(),
        link_group,
        link_group_mappings,
        link_group_preferred_linkage,
        link_group_libs = link_group_libs,
        link_style = link_style,
        deps = linkable_deps(non_exported_deps + exported_deps),
        prefer_stripped = prefer_stripped,
    )
    filtered_links = get_filtered_links(filtered_labels_to_links_map)
    filtered_targets = get_filtered_targets(filtered_labels_to_links_map)

    # Unfortunately, link_groups does not use MergedLinkInfo to represent the args
    # for the resolved nodes in the graph.
    # Thus, we have no choice but to traverse all the nodes to dedupe the framework linker args.
    frameworks_link_info = get_frameworks_link_info_by_deduping_link_infos(ctx, filtered_links, frameworks_linkable)
    if frameworks_link_info:
        filtered_links.append(frameworks_link_info)

    return LinkArgs(infos = filtered_links), get_link_group_map_json(ctx, filtered_targets)

def _use_pic(link_style: LinkStyle.type) -> bool.type:
    """
    Whether this link style requires PIC objects.
    """
    return link_style != LinkStyle("static")

# Create the objects/archive to use for static linking this rule.
# Returns a tuple of objects/archive to use as the default output for the link
# style(s) it's used in and the `LinkInfo` to export to dependents.
def _static_library(
        ctx: "context",
        impl_params: "CxxRuleConstructorParams",
        objects: ["artifact"],
        pic: bool.type,
        stripped: bool.type,
        extra_linkables: ["FrameworksLinkable"],
        objects_have_external_debug_info: bool.type = False,
        external_debug_info: ["_arglike"] = []) -> (_CxxLibraryOutput.type, LinkInfo.type):
    if len(objects) == 0:
        fail("empty objects")

    # No reason to create a static library with just a single object file. We
    # still want to create a static lib to expose as the default output because
    # it's the contract/expectation of external clients of the cmd line
    # interface. Any tools consuming `buck build` outputs should get a
    # consistent output type when building a library, not static lib or object
    # file depending on number of source files.
    linker_info = get_cxx_toolchain_info(ctx).linker_info
    linker_type = linker_info.type

    base_name = _base_static_library_name(ctx, stripped)
    name = _archive_name(base_name, pic = pic, extension = linker_info.static_library_extension)
    archive = make_archive(ctx, name, objects)

    linkable = None
    if use_archives(ctx):
        linkable = ArchiveLinkable(
            archive = archive,
            linker_type = linker_type,
            link_whole = _attr_link_whole(ctx),
        )
    else:
        linkable = ObjectsLinkable(
            objects = objects,
            linker_type = linker_type,
            link_whole = _attr_link_whole(ctx),
        )

    post_flags = []

    if pic:
        post_flags.extend(linker_info.static_pic_dep_runtime_ld_flags or [])
    else:
        post_flags.extend(linker_info.static_dep_runtime_ld_flags or [])

    all_external_debug_info = []
    all_external_debug_info.extend(external_debug_info)

    # On darwin, the linked output references the archive that contains the
    # object files instead of the originating objects.
    if linker_type == "darwin":
        all_external_debug_info.append(archive.artifact)
        all_external_debug_info.extend(archive.external_objects)
    elif objects_have_external_debug_info:
        all_external_debug_info.extend(objects)

    return (
        _CxxLibraryOutput(
            default = archive.artifact,
            object_files = objects,
            other = archive.external_objects,
        ),
        LinkInfo(
            name = name,
            # We're propagating object code for linking up the dep tree,
            # so we need to also propagate any necessary link flags required for
            # the object code.
            pre_flags = impl_params.extra_exported_link_flags,
            post_flags = post_flags,
            # Extra linkables are propogated here so they are available to link_groups
            # when they are deducing linker args.
            linkables = [linkable] + extra_linkables,
            external_debug_info = all_external_debug_info,
        ),
    )

def _shared_library(
        ctx: "context",
        impl_params: "CxxRuleConstructorParams",
        objects: ["artifact"],
        external_debug_info: ["_arglike"],
        dep_infos: "LinkArgs") -> (str.type, LinkedObject.type, LinkInfo.type):
    """
    Generate a shared library and the associated native link info used by
    dependents to link against it.

    Returns a 3-tuple of
      1) shared library name (e.g. SONAME),
      2) the `LinkedObject` wrapping the shared library, and
      3) the `LinkInfo` used to link against the shared library.
    """

    soname = _soname(ctx)
    cxx_toolchain = get_cxx_toolchain_info(ctx)
    linker_info = cxx_toolchain.linker_info

    # NOTE(agallagher): We add exported link flags here because it's what v1
    # does, but the intent of exported link flags are to wrap the link output
    # that we propagate up the tree, rather than being used locally when
    # generating a link product.
    link_info = LinkInfo(
        pre_flags = (
            cxx_attr_exported_linker_flags(ctx) +
            cxx_attr_linker_flags(ctx)
        ),
        linkables = [ObjectsLinkable(
            objects = objects,
            linker_type = linker_info.type,
            link_whole = True,
        )],
        post_flags = (
            impl_params.extra_exported_link_flags +
            impl_params.extra_link_flags +
            _attr_post_linker_flags(ctx) +
            (linker_info.shared_dep_runtime_ld_flags or [])
        ),
        external_debug_info = external_debug_info,
    )
    shlib = cxx_link_into_shared_library(
        ctx,
        soname,
        [LinkArgs(infos = [link_info]), dep_infos],
        identifier = soname,
        soname = impl_params.use_soname,
        shared_library_flags = impl_params.shared_library_flags,
        strip = impl_params.strip_executable,
        strip_args_factory = impl_params.strip_args_factory,
        link_postprocessor = impl_params.link_postprocessor,
    )

    exported_shlib = shlib.output

    # If shared library interfaces are enabled, link that and use it as
    # the shared lib that dependents will link against.
    # TODO(agallagher): There's a bug in shlib intfs interacting with link
    # groups, where we don't include the symbols we're meant to export from
    # deps that get statically linked in.
    if cxx_use_shlib_intfs(ctx) and not cxx_use_link_groups(ctx):
        link_info = LinkInfo(
            pre_flags = link_info.pre_flags,
            linkables = link_info.linkables,
            post_flags = (
                (link_info.post_flags or []) +
                get_ignore_undefined_symbols_flags(linker_info.type) +
                (linker_info.independent_shlib_interface_linker_flags or [])
            ),
            external_debug_info = link_info.external_debug_info,
        )
        shlib_for_interface = ctx.actions.declare_output(
            get_shared_library_name(
                linker_info,
                ctx.label.name + "-for-interface",
            ),
        )
        cxx_link_shared_library(
            ctx,
            output = shlib_for_interface,
            category_suffix = "interface",
            name = soname,
            links = [LinkArgs(infos = [link_info])],
            identifier = soname,
        )

        # Convert the shared library into an interface.
        shlib_interface = cxx_mk_shlib_intf(ctx, ctx.label.name, shlib_for_interface)

        exported_shlib = shlib_interface

    # Link against import library on Windows.
    if shlib.import_library:
        exported_shlib = shlib.import_library

    return (
        soname,
        shlib,
        LinkInfo(
            name = soname,
            linkables = [SharedLibLinkable(
                lib = exported_shlib,
            )],
        ),
    )

def _attr_reexport_all_header_dependencies(ctx: "context") -> bool.type:
    return value_or(ctx.attrs.reexport_all_header_dependencies, False)

def _soname(ctx: "context") -> str.type:
    """
    Get the shared library name to set for the given C++ library.
    """
    linker_info = get_cxx_toolchain_info(ctx).linker_info
    explicit_soname = ctx.attrs.soname
    if explicit_soname != None:
        return get_shared_library_name_for_param(linker_info, explicit_soname)
    return get_default_shared_library_name(linker_info, ctx.label)

def _base_static_library_name(ctx: "context", stripped: bool.type) -> str.type:
    return ctx.label.name + ".stripped" if stripped else ctx.label.name

def _archive_name(name: str.type, pic: bool.type, extension: str.type) -> str.type:
    return "lib{}{}.{}".format(name, ".pic" if pic else "", extension)

def _attr_link_whole(ctx: "context") -> bool.type:
    return value_or(ctx.attrs.link_whole, False)

def use_archives(ctx: "context") -> bool.type:
    """
    Whether this rule should use archives to package objects when producing
    link input for dependents.
    """

    requires_archives = get_cxx_toolchain_info(ctx).linker_info.requires_archives
    requires_objects = get_cxx_toolchain_info(ctx).linker_info.requires_objects
    if requires_archives and requires_objects:
        fail("In cxx linker_info, only one of `requires_archives` and `requires_objects` can be enabled")

    # If the toolchain requires them, then always use them.
    if requires_archives:
        return True

    if requires_objects:
        return False

    # Otherwise, fallback to the rule-specific setting.
    return value_or(ctx.attrs.use_archive, True)

def _attr_post_linker_flags(ctx: "context") -> [""]:
    return (
        ctx.attrs.post_linker_flags +
        flatten(cxx_by_platform(ctx, ctx.attrs.post_platform_linker_flags))
    )
