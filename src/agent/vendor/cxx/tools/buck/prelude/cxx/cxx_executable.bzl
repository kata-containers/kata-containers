# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//:local_only.bzl", "link_cxx_binary_locally")
load(
    "@prelude//:resources.bzl",
    "create_resource_db",
    "gather_resources",
)
load(
    "@prelude//apple:apple_frameworks.bzl",
    "build_link_args_with_deduped_framework_flags",
    "create_frameworks_linkable",
    "get_frameworks_link_info_by_deduping_link_infos",
)
load(
    "@prelude//cxx:cxx_bolt.bzl",
    "cxx_use_bolt",
)
load(
    "@prelude//ide_integrations:xcode.bzl",
    "XCODE_DATA_SUB_TARGET",
    "generate_xcode_data",
)
load(
    "@prelude//linking:link_groups.bzl",
    "LinkGroupLib",
    "gather_link_group_libs",
)
load(
    "@prelude//linking:link_info.bzl",
    "LinkArgs",
    "LinkInfo",
    "LinkInfos",
    "LinkStyle",
    "LinkedObject",  # @unused Used as a type
    "ObjectsLinkable",
    "SharedLibLinkable",
    "merge_link_infos",
)
load(
    "@prelude//linking:linkable_graph.bzl",
    "create_linkable_graph",
    "get_linkable_graph_node_map_func",
)
load(
    "@prelude//linking:linkables.bzl",
    "linkables",
)
load(
    "@prelude//linking:shared_libraries.bzl",
    "merge_shared_libraries",
    "traverse_shared_library_info",
)
load(
    "@prelude//utils:utils.bzl",
    "expect",
    "flatten_dict",
)
load(
    ":comp_db.bzl",
    "CxxCompilationDbInfo",  # @unused Used as a type
    "create_compilation_database",
    "make_compilation_db_info",
)
load(
    ":compile.bzl",
    "compile_cxx",
    "create_compile_cmds",
)
load(":cxx_context.bzl", "get_cxx_platform_info", "get_cxx_toolchain_info")
load(
    ":cxx_library_utility.bzl",
    "ARGSFILES_SUBTARGET",
    "cxx_attr_deps",
    "cxx_attr_link_style",
    "cxx_attr_linker_flags",
    "cxx_attr_resources",
    "cxx_is_gnu",
)
load(
    ":cxx_link_utility.bzl",
    "executable_shared_lib_arguments",
)
load(
    ":cxx_types.bzl",
    "CxxRuleConstructorParams",  # @unused Used as a type
)
load(
    ":link.bzl",
    "cxx_link",
)
load(
    ":link_groups.bzl",
    "LINK_GROUP_MAP_DATABASE_SUB_TARGET",
    "create_link_group",
    "get_filtered_labels_to_links_map",
    "get_filtered_links",
    "get_filtered_targets",
    "get_link_group",
    "get_link_group_map_json",
    "get_link_group_preferred_linkage",
)
load(
    ":preprocessor.bzl",
    "cxx_inherited_preprocessor_infos",
    "cxx_private_preprocessor_info",
)

_CxxExecutableOutput = record(
    binary = "artifact",
    # Files that will likely need to be included as .hidden() arguments
    # when executing the executable (ex. RunInfo())
    runtime_files = ["_arglike"],
    sub_targets = {str.type: [DefaultInfo.type]},
    # The LinkArgs used to create the final executable in 'binary'.
    link_args = [LinkArgs.type],
    # External components needed to debug the executable.
    external_debug_info = ["_arglike"],
    shared_libs = {str.type: LinkedObject.type},
    # All link group links that were generated in the executable.
    auto_link_groups = field({str.type: LinkedObject.type}, {}),
)

# returns a tuple of the runnable binary as an artifact, a list of its runtime files as artifacts and a sub targets map, and the CxxCompilationDbInfo provider
def cxx_executable(ctx: "context", impl_params: CxxRuleConstructorParams.type, is_cxx_test: bool.type = False) -> (_CxxExecutableOutput.type, CxxCompilationDbInfo.type, "XcodeDataInfo"):
    # Gather preprocessor inputs.
    preprocessor_deps = cxx_attr_deps(ctx) + filter(None, [ctx.attrs.precompiled_header])
    (own_preprocessor_info, test_preprocessor_infos) = cxx_private_preprocessor_info(
        ctx,
        impl_params.headers_layout,
        raw_headers = ctx.attrs.raw_headers,
        extra_preprocessors = impl_params.extra_preprocessors,
        non_exported_deps = preprocessor_deps,
        is_test = is_cxx_test,
    )
    inherited_preprocessor_infos = cxx_inherited_preprocessor_infos(preprocessor_deps) + impl_params.extra_preprocessors_info

    # The link style to use.
    link_style = cxx_attr_link_style(ctx)

    sub_targets = {}

    # Compile objects.
    compile_cmd_output = create_compile_cmds(
        ctx,
        impl_params,
        [own_preprocessor_info] + test_preprocessor_infos,
        inherited_preprocessor_infos,
    )
    cxx_outs = compile_cxx(ctx, compile_cmd_output.source_commands.src_compile_cmds, pic = link_style != LinkStyle("static"))
    sub_targets[ARGSFILES_SUBTARGET] = [compile_cmd_output.source_commands.argsfiles_info]

    # Compilation DB.
    comp_db = create_compilation_database(ctx, compile_cmd_output.comp_db_commands.src_compile_cmds)
    sub_targets["compilation-database"] = [comp_db]
    comp_db_info = make_compilation_db_info(compile_cmd_output.comp_db_commands.src_compile_cmds, get_cxx_toolchain_info(ctx), get_cxx_platform_info(ctx))

    # Link deps
    link_deps = linkables(cxx_attr_deps(ctx)) + impl_params.extra_link_deps

    # Link Groups
    link_group = get_link_group(ctx)
    link_group_info = impl_params.link_group_info

    if link_group_info:
        link_groups = link_group_info.groups
        link_group_mappings = link_group_info.mappings
        link_group_deps = [mapping.root.node.linkable_graph for group in link_group_info.groups for mapping in group.mappings if mapping.root != None]
    else:
        link_groups = []
        link_group_mappings = {}
        link_group_deps = []
    link_group_preferred_linkage = get_link_group_preferred_linkage(link_groups)

    # Create the linkable graph with the binary's deps and any link group deps.
    linkable_graph = create_linkable_graph(
        ctx,
        children = [d.linkable_graph for d in link_deps] + link_group_deps,
    )

    # Gather link inputs.
    own_link_flags = cxx_attr_linker_flags(ctx) + impl_params.extra_link_flags + impl_params.extra_exported_link_flags
    own_binary_link_flags = ctx.attrs.binary_linker_flags + own_link_flags
    inherited_link = merge_link_infos(ctx, [d.merged_link_info for d in link_deps])
    frameworks_linkable = create_frameworks_linkable(ctx)

    # Link group libs.
    link_group_libs = {}
    auto_link_groups = {}
    labels_to_links_map = {}

    if not link_group_mappings:
        dep_links = build_link_args_with_deduped_framework_flags(
            ctx,
            inherited_link,
            frameworks_linkable,
            link_style,
            prefer_stripped = ctx.attrs.prefer_stripped_objects,
        )
    else:
        linkable_graph_node_map = get_linkable_graph_node_map_func(linkable_graph)()

        # If we're using auto-link-groups, where we generate the link group links
        # in the prelude, the link group map will give us the link group libs.
        # Otherwise, pull them from the `LinkGroupLibInfo` provider from out deps.
        if impl_params.auto_link_group_specs != None:
            for link_group_spec in impl_params.auto_link_group_specs:
                # NOTE(agallagher): It might make sense to move this down to be
                # done when we generated the links for the executable, so we can
                # handle the case when a link group can depend on the executable.
                link_group_lib = create_link_group(
                    ctx = ctx,
                    spec = link_group_spec,
                    executable_deps = [
                        dep.linkable_graph.nodes.value.label
                        for dep in link_deps
                    ],
                    root_link_group = link_group,
                    linkable_graph_node_map = linkable_graph_node_map,
                    linker_flags = own_link_flags,
                    link_group_mappings = link_group_mappings,
                    link_group_preferred_linkage = {},
                    #link_style = LinkStyle("static_pic"),
                    # TODO(agallagher): We should generate link groups via post-order
                    # traversal to get inter-group deps correct.
                    #link_group_libs = {},
                    prefer_stripped_objects = ctx.attrs.prefer_stripped_objects,
                    category_suffix = "link_group",
                )
                auto_link_groups[link_group_spec.group.name] = link_group_lib
                if link_group_spec.is_shared_lib:
                    link_group_libs[link_group_spec.group.name] = LinkGroupLib(
                        shared_libs = {link_group_spec.name: link_group_lib},
                        shared_link_infos = LinkInfos(
                            default = LinkInfo(
                                linkables = [
                                    SharedLibLinkable(lib = link_group_lib.output),
                                ],
                            ),
                        ),
                    )
        else:
            link_group_libs = gather_link_group_libs(
                children = [d.link_group_lib_info for d in link_deps],
            )

        # TODO(T110378098): Similar to shared libraries, we need to identify all the possible
        # scenarios for which we need to propagate up link info and simplify this logic. For now
        # base which links to use based on whether link groups are defined.
        labels_to_links_map = get_filtered_labels_to_links_map(
            linkable_graph_node_map,
            link_group,
            link_group_mappings,
            link_group_preferred_linkage,
            link_group_libs = link_group_libs,
            link_style = link_style,
            deps = [d.linkable_graph.nodes.value.label for d in link_deps],
            is_executable_link = True,
            prefer_stripped = ctx.attrs.prefer_stripped_objects,
        )

        if is_cxx_test and link_group != None:
            # if a cpp_unittest is part of the link group, we need to traverse through all deps
            # from the root again to ensure we link in gtest deps
            labels_to_links_map = labels_to_links_map | get_filtered_labels_to_links_map(
                linkable_graph_node_map,
                None,
                link_group_mappings,
                link_group_preferred_linkage,
                link_style,
                deps = [d.linkable_graph.nodes.value.label for d in link_deps],
                is_executable_link = True,
                prefer_stripped = ctx.attrs.prefer_stripped_objects,
            )

        filtered_links = get_filtered_links(labels_to_links_map)
        filtered_targets = get_filtered_targets(labels_to_links_map)

        # Unfortunately, link_groups does not use MergedLinkInfo to represent the args
        # for the resolved nodes in the graph.
        # Thus, we have no choice but to traverse all the nodes to dedupe the framework linker args.
        frameworks_link_info = get_frameworks_link_info_by_deduping_link_infos(ctx, filtered_links, frameworks_linkable)
        if frameworks_link_info:
            filtered_links.append(frameworks_link_info)

        dep_links = LinkArgs(infos = filtered_links)
        sub_targets[LINK_GROUP_MAP_DATABASE_SUB_TARGET] = [get_link_group_map_json(ctx, filtered_targets)]

    # Set up shared libraries symlink tree only when needed
    shared_libs = {}

    # Only setup a shared library symlink tree when shared linkage or link_groups is used
    gnu_use_link_groups = cxx_is_gnu(ctx) and link_group_mappings
    if link_style == LinkStyle("shared") or gnu_use_link_groups:
        shlib_info = merge_shared_libraries(
            ctx.actions,
            deps = [d.shared_library_info for d in link_deps],
        )

        def is_link_group_shlib(label: "label"):
            # If this maps to a link group which we have a `LinkGroupLibInfo` for,
            # then we'll handlet his below.
            # buildifier: disable=uninitialized
            if label in link_group_mappings and link_group_mappings[label] in link_group_libs:
                return False

            # if using link_groups, only materialize the link_group shlibs
            return label in labels_to_links_map and labels_to_links_map[label].link_style == LinkStyle("shared")  # buildifier: disable=uninitialized

        for name, shared_lib in traverse_shared_library_info(shlib_info).items():
            label = shared_lib.label
            if not gnu_use_link_groups or is_link_group_shlib(label):
                shared_libs[name] = shared_lib.lib

    if gnu_use_link_groups:
        # All explicit link group libs (i.e. libraries that set `link_group`).
        link_group_names = {n: None for n in link_group_mappings.values()}
        for name, link_group_lib in link_group_libs.items():
            # Is it possible to find a link group lib in our graph without it
            # having a mapping setup?
            expect(name in link_group_names)
            shared_libs.update(link_group_lib.shared_libs)

    toolchain_info = get_cxx_toolchain_info(ctx)
    linker_info = toolchain_info.linker_info
    links = [
        LinkArgs(infos = [
            LinkInfo(
                pre_flags = own_binary_link_flags,
                linkables = [ObjectsLinkable(
                    objects = [out.object for out in cxx_outs],
                    linker_type = linker_info.type,
                    link_whole = True,
                )],
                external_debug_info = (
                    [out.object for out in cxx_outs if out.object_has_external_debug_info] +
                    [out.external_debug_info for out in cxx_outs if out.external_debug_info != None]
                ),
            ),
        ]),
        dep_links,
    ] + impl_params.extra_link_args

    binary, runtime_files, shared_libs_symlink_tree, extra_args = _link_into_executable(
        ctx,
        links,
        # If shlib lib tree generation is enabled, pass in the shared libs (which
        # will trigger the necessary link tree and link args).
        shared_libs if impl_params.exe_shared_libs_link_tree else {},
        linker_info.link_weight,
        linker_info.binary_extension,
        prefer_local = False if impl_params.force_full_hybrid_if_capable else link_cxx_binary_locally(ctx),
        enable_distributed_thinlto = ctx.attrs.enable_distributed_thinlto,
        strip = impl_params.strip_executable,
        strip_args_factory = impl_params.strip_args_factory,
        link_postprocessor = impl_params.link_postprocessor,
        force_full_hybrid_if_capable = impl_params.force_full_hybrid_if_capable,
    )

    # Define the xcode data sub target
    xcode_data_default_info, xcode_data_info = generate_xcode_data(
        ctx,
        rule_type = impl_params.rule_type,
        output = binary.output,
        populate_rule_specific_attributes_func = impl_params.cxx_populate_xcode_attributes_func,
        srcs = impl_params.srcs + impl_params.additional.srcs,
        argsfiles_by_ext = compile_cmd_output.source_commands.argsfile_by_ext,
        product_name = get_cxx_excutable_product_name(ctx),
    )
    sub_targets[XCODE_DATA_SUB_TARGET] = xcode_data_default_info

    # Info about dynamic-linked libraries for fbpkg integration:
    # - the symlink dir that's part of RPATH
    # - sub-sub-targets that reference shared library dependencies and their respective dwp
    # - [shared-libraries] - a json map that references the above rules.
    if shared_libs_symlink_tree:
        sub_targets["rpath-tree"] = [DefaultInfo(default_outputs = [shared_libs_symlink_tree])]
    sub_targets["shared-libraries"] = [DefaultInfo(
        default_outputs = [ctx.actions.write_json(
            binary.output.basename + ".shared-libraries.json",
            {
                "libraries": ["{}:{}[shared-libraries][{}]".format(ctx.label.path, ctx.label.name, name) for name in shared_libs.keys()],
                "librariesdwp": ["{}:{}[shared-libraries][{}][dwp]".format(ctx.label.path, ctx.label.name, name) for name, lib in shared_libs.items() if lib.dwp],
                "rpathtree": ["{}:{}[rpath-tree]".format(ctx.label.path, ctx.label.name)] if shared_libs_symlink_tree else [],
            },
        )],
        sub_targets = {
            name: [DefaultInfo(
                default_outputs = [lib.output],
                sub_targets = {"dwp": [DefaultInfo(default_outputs = [lib.dwp])]} if lib.dwp else {},
            )]
            for name, lib in shared_libs.items()
        },
    )]

    # TODO(T110378140): We can't really enable this yet, as Python binaries
    # consuming C++ binaries as resources don't know how to handle the
    # extraneous debug paths and will crash.  We probably need to add a special
    # exported resources provider and make sure we handle the workflows.
    # Add any referenced debug paths to runtime files.
    #runtime_files.extend(binary.external_debug_info)

    # If we have some resources, write it to the resources JSON file and add
    # it and all resources to "runtime_files" so that we make to materialize
    # them with the final binary.
    resources = flatten_dict(gather_resources(
        label = ctx.label,
        resources = cxx_attr_resources(ctx),
        deps = cxx_attr_deps(ctx),
    ).values())
    if resources:
        runtime_files.append(create_resource_db(
            ctx = ctx,
            name = binary.output.basename + ".resources.json",
            binary = binary.output,
            resources = resources,
        ))
        for resource, other in resources.values():
            runtime_files.append(resource)
            runtime_files.extend(other)

    if binary.dwp:
        # A `dwp` sub-target which generates the `.dwp` file for this binary.
        sub_targets["dwp"] = [DefaultInfo(default_outputs = [binary.dwp])]

    # If bolt is not ran, binary.prebolt_output will be the same as binary.output. Only
    # expose binary.prebolt_output if cxx_use_bolt(ctx) is True to avoid confusion
    if cxx_use_bolt(ctx):
        sub_targets["prebolt"] = [DefaultInfo(default_outputs = [binary.prebolt_output])]

    (linker_map, binary_for_linker_map) = _linker_map(
        ctx,
        binary,
        [LinkArgs(flags = extra_args)] + links,
        prefer_local = link_cxx_binary_locally(ctx, toolchain_info),
        link_weight = linker_info.link_weight,
    )
    sub_targets["linker-map"] = [DefaultInfo(default_outputs = [linker_map], other_outputs = [binary_for_linker_map])]

    sub_targets["linker.argsfile"] = [DefaultInfo(
        default_outputs = [binary.linker_argsfile],
    )]

    if linker_info.supports_distributed_thinlto and ctx.attrs.enable_distributed_thinlto:
        sub_targets["index.argsfile"] = [DefaultInfo(
            default_outputs = [binary.index_argsfile],
        )]

    return _CxxExecutableOutput(
        binary = binary.output,
        runtime_files = runtime_files,
        sub_targets = sub_targets,
        link_args = links,
        external_debug_info = binary.external_debug_info,
        shared_libs = shared_libs,
        auto_link_groups = auto_link_groups,
    ), comp_db_info, xcode_data_info

# Returns a tuple of:
# - the resulting executable
# - list of files/directories that should be present for executable to be run successfully
# - optional shared libs symlink tree symlinked_dir action
# - extra linking args (for the shared_libs)
def _link_into_executable(
        ctx: "context",
        links: [LinkArgs.type],
        shared_libs: {str.type: LinkedObject.type},
        link_weight: int.type,
        binary_extension: str.type,
        prefer_local: bool.type = False,
        enable_distributed_thinlto = False,
        strip: bool.type = False,
        strip_args_factory = None,
        link_postprocessor: ["cmd_args", None] = None,
        force_full_hybrid_if_capable: bool.type = False) -> (LinkedObject.type, ["_arglike"], ["artifact", None], [""]):
    output = ctx.actions.declare_output("{}{}".format(get_cxx_excutable_product_name(ctx), "." + binary_extension if binary_extension else ""))
    extra_args, runtime_files, shared_libs_symlink_tree = executable_shared_lib_arguments(
        ctx.actions,
        get_cxx_toolchain_info(ctx),
        output,
        shared_libs,
    )
    exe = cxx_link(
        ctx,
        [LinkArgs(flags = extra_args)] + links,
        output,
        prefer_local = prefer_local,
        link_weight = link_weight,
        enable_distributed_thinlto = enable_distributed_thinlto,
        category_suffix = "executable",
        strip = strip,
        strip_args_factory = strip_args_factory,
        executable_link = True,
        link_postprocessor = link_postprocessor,
        force_full_hybrid_if_capable = force_full_hybrid_if_capable,
    )
    return (exe, runtime_files, shared_libs_symlink_tree, extra_args)

def _linker_map(
        ctx: "context",
        binary: LinkedObject.type,
        links: [LinkArgs.type],
        prefer_local: bool.type,
        link_weight: int.type) -> ("artifact", "artifact"):
    identifier = binary.output.short_path + ".linker-map-binary"
    binary_for_linker_map = ctx.actions.declare_output(identifier)
    linker_map = ctx.actions.declare_output(binary.output.short_path + ".linker-map")
    cxx_link(
        ctx,
        links,
        binary_for_linker_map,
        category_suffix = "linker_map",
        linker_map = linker_map,
        prefer_local = prefer_local,
        link_weight = link_weight,
        identifier = identifier,
        generate_dwp = False,
    )
    return (
        linker_map,
        binary_for_linker_map,
    )

def get_cxx_excutable_product_name(ctx: "context") -> str.type:
    return ctx.label.name + ("-wrapper" if cxx_use_bolt(ctx) else "")
