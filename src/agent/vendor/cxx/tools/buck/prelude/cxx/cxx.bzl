# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load(
    "@prelude//cxx:link_groups.bzl",
    "LinkGroupInfo",  # @unused Used as a type
)
load(
    "@prelude//linking:link_groups.bzl",
    "merge_link_group_lib_info",
)
load(
    "@prelude//linking:link_info.bzl",
    "Archive",
    "ArchiveLinkable",
    "LinkArgs",
    "LinkInfo",
    "LinkInfos",
    "LinkStyle",
    "Linkage",
    "LinkedObject",
    "SharedLibLinkable",
    "create_merged_link_info",
    "get_actual_link_style",
    "get_link_args",
    "get_link_styles_for_linkage",
)
load(
    "@prelude//linking:linkable_graph.bzl",
    "AnnotatedLinkableRoot",
    "LinkableGraph",
    "create_linkable_graph",
    "create_linkable_graph_node",
    "create_linkable_node",
)
load("@prelude//linking:shared_libraries.bzl", "SharedLibraryInfo", "create_shared_libraries", "merge_shared_libraries")
load(
    "@prelude//tests:re_utils.bzl",
    "get_re_executor_from_props",
)
load(
    "@prelude//utils:utils.bzl",
    "expect",
    "filter_and_map_idx",
    "flatten",
    "value_or",
)
load("@prelude//test/inject_test_run_info.bzl", "inject_test_run_info")
load(
    ":compile.bzl",
    "CxxSrcWithFlags",  # @unused Used as a type
)
load(":cxx_context.bzl", "get_cxx_toolchain_info")
load(":cxx_executable.bzl", "cxx_executable")
load(":cxx_library.bzl", "cxx_library_parameterized")
load(
    ":cxx_library_utility.bzl",
    "cxx_attr_deps",
    "cxx_attr_exported_deps",
    "cxx_attr_exported_linker_flags",
    "cxx_attr_exported_post_linker_flags",
    "cxx_attr_preferred_linkage",
    "cxx_inherited_link_info",
    "cxx_mk_shlib_intf",
    "cxx_platform_supported",
    "cxx_use_shlib_intfs",
)
load(
    ":cxx_types.bzl",
    "CxxRuleConstructorParams",
    "CxxRuleProviderParams",
    "CxxRuleSubTargetParams",
)
load(
    ":groups.bzl",
    "Group",  # @unused Used as a type
    "MATCH_ALL_LABEL",
    "NO_MATCH_LABEL",
)
load(
    ":headers.bzl",
    "CPrecompiledHeaderInfo",
    "cxx_get_regular_cxx_headers_layout",
)
load(
    ":link.bzl",
    _cxx_link_into_shared_library = "cxx_link_into_shared_library",
)
load(
    ":link_groups.bzl",
    "LinkGroupLibSpec",
    "get_link_group_info",
)
load(
    ":linker.bzl",
    "get_link_whole_args",
    "get_shared_library_name",
)
load(
    ":omnibus.bzl",
    "create_linkable_root",
    "is_known_omnibus_root",
)
load(":platform.bzl", "cxx_by_platform")
load(
    ":preprocessor.bzl",
    "CPreprocessor",
    "cxx_attr_exported_preprocessor_flags",
    "cxx_exported_preprocessor_info",
    "cxx_inherited_preprocessor_infos",
    "cxx_merge_cpreprocessors",
)

cxx_link_into_shared_library = _cxx_link_into_shared_library

#####################################################################
# Attributes

# The source files
def get_srcs_with_flags(ctx: "context") -> [CxxSrcWithFlags.type]:
    all_srcs = ctx.attrs.srcs + flatten(cxx_by_platform(ctx, ctx.attrs.platform_srcs))

    # src -> flags_hash -> flags
    flags_sets_by_src = {}
    for x in all_srcs:
        if type(x) == type(()):
            artifact = x[0]
            flags = x[1]
        else:
            artifact = x
            flags = []

        flags_hash = hash(str(flags))
        flag_sets = flags_sets_by_src.setdefault(artifact, {})
        flag_sets[flags_hash] = flags

    # Go through collected (source, flags) pair and set the index field if there are duplicate source files
    cxx_src_with_flags_records = []
    for (artifact, flag_sets) in flags_sets_by_src.items():
        needs_indices = len(flag_sets) > 1
        for i, flags in enumerate(flag_sets.values()):
            index = i if needs_indices else None
            cxx_src_with_flags_records.append(CxxSrcWithFlags(file = artifact, flags = flags, index = index))

    return cxx_src_with_flags_records

#####################################################################
# Operations

def _get_shared_link_style_sub_targets_and_providers(
        link_style: LinkStyle.type,
        _ctx: "context",
        _executable: "artifact",
        _external_debug_info: ["_arglike"],
        dwp: ["artifact", None]) -> ({str.type: ["provider"]}, ["provider"]):
    if link_style != LinkStyle("shared") or dwp == None:
        return ({}, [])
    return ({"dwp": [DefaultInfo(default_outputs = [dwp])]}, [])

def cxx_library_impl(ctx: "context") -> ["provider"]:
    if ctx.attrs.can_be_asset and ctx.attrs.used_by_wrap_script:
        fail("Cannot use `can_be_asset` and `used_by_wrap_script` in the same rule")

    if ctx.attrs._is_building_android_binary:
        sub_target_params, provider_params = _get_params_for_android_binary_cxx_library()
    else:
        sub_target_params = CxxRuleSubTargetParams()
        provider_params = CxxRuleProviderParams()

    params = CxxRuleConstructorParams(
        rule_type = "cxx_library",
        headers_layout = cxx_get_regular_cxx_headers_layout(ctx),
        srcs = get_srcs_with_flags(ctx),
        link_style_sub_targets_and_providers_factory = _get_shared_link_style_sub_targets_and_providers,
        is_omnibus_root = is_known_omnibus_root(ctx),
        force_emit_omnibus_shared_root = ctx.attrs.force_emit_omnibus_shared_root,
        generate_sub_targets = sub_target_params,
        generate_providers = provider_params,
    )
    output = cxx_library_parameterized(ctx, params)
    return output.providers

def _only_shared_mappings(group: Group.type) -> bool.type:
    """
    Return whether this group only has explicit "shared" linkage mappings,
    which indicates a group that re-uses pre-linked libs.
    """
    for mapping in group.mappings:
        if mapping.preferred_linkage != Linkage("shared"):
            return False
    return True

def get_cxx_auto_link_group_specs(ctx: "context", link_group_info: [LinkGroupInfo.type, None]) -> [[LinkGroupLibSpec.type], None]:
    if link_group_info == None or not ctx.attrs.auto_link_groups:
        return None
    specs = []
    linker_info = get_cxx_toolchain_info(ctx).linker_info
    for group in link_group_info.groups:
        if group.name in (MATCH_ALL_LABEL, NO_MATCH_LABEL):
            continue

        # TODO(agallagher): We should probably add proper handling for "provided"
        # system handling to avoid needing this special case.
        if _only_shared_mappings(group):
            continue
        specs.append(
            LinkGroupLibSpec(
                name = get_shared_library_name(linker_info, group.name),
                is_shared_lib = True,
                group = group,
            ),
        )
    return specs

def cxx_binary_impl(ctx: "context") -> ["provider"]:
    link_group_info = get_link_group_info(ctx, filter_and_map_idx(LinkableGraph, cxx_attr_deps(ctx)))
    params = CxxRuleConstructorParams(
        rule_type = "cxx_binary",
        headers_layout = cxx_get_regular_cxx_headers_layout(ctx),
        srcs = get_srcs_with_flags(ctx),
        link_group_info = link_group_info,
        auto_link_group_specs = get_cxx_auto_link_group_specs(ctx, link_group_info),
    )
    output, comp_db_info, xcode_data_info = cxx_executable(ctx, params)

    return [
        DefaultInfo(
            default_outputs = [output.binary],
            other_outputs = output.runtime_files,
            sub_targets = output.sub_targets,
        ),
        RunInfo(args = cmd_args(output.binary).hidden(output.runtime_files)),
        comp_db_info,
        xcode_data_info,
    ]

def _prebuilt_item(
        ctx: "context",
        item: ["", None],
        platform_items: [[(str.type, "_a")], None]) -> ["_a", None]:
    """
    Parse the given item that can be specified by regular and platform-specific
    parameters.
    """

    if item != None:
        return item

    if platform_items != None:
        items = cxx_by_platform(ctx, platform_items)
        if len(items) == 0:
            return None
        if len(items) != 1:
            fail("expected single platform match: name={}//{}:{}, platform_items={}, items={}".format(ctx.label.cell, ctx.label.package, ctx.label.name, str(platform_items), str(items)))
        return items[0]

    return None

def _prebuilt_linkage(ctx: "context") -> Linkage.type:
    """
    Construct the preferred linkage to use for the given prebuilt library.
    """
    if ctx.attrs.header_only:
        return Linkage("any")
    if ctx.attrs.force_static:
        return Linkage("static")
    preferred_linkage = cxx_attr_preferred_linkage(ctx)
    if preferred_linkage != Linkage("any"):
        return preferred_linkage
    if ctx.attrs.provided:
        return Linkage("shared")
    return Linkage("any")

def prebuilt_cxx_library_impl(ctx: "context") -> ["provider"]:
    # Versioned params should be intercepted and converted away via the stub.
    expect(not ctx.attrs.versioned_exported_lang_platform_preprocessor_flags)
    expect(not ctx.attrs.versioned_exported_lang_preprocessor_flags)
    expect(not ctx.attrs.versioned_exported_platform_preprocessor_flags)
    expect(not ctx.attrs.versioned_exported_preprocessor_flags)
    expect(not ctx.attrs.versioned_header_dirs)
    expect(not ctx.attrs.versioned_shared_lib)
    expect(not ctx.attrs.versioned_static_lib)
    expect(not ctx.attrs.versioned_static_pic_lib)

    if not cxx_platform_supported(ctx):
        return [DefaultInfo(default_outputs = [])]

    providers = []

    linker_info = get_cxx_toolchain_info(ctx).linker_info
    linker_type = linker_info.type

    # Parse library parameters.
    static_lib = _prebuilt_item(
        ctx,
        ctx.attrs.static_lib,
        ctx.attrs.platform_static_lib,
    )
    static_pic_lib = _prebuilt_item(
        ctx,
        ctx.attrs.static_pic_lib,
        ctx.attrs.platform_static_pic_lib,
    )
    shared_lib = _prebuilt_item(
        ctx,
        ctx.attrs.shared_lib,
        ctx.attrs.platform_shared_lib,
    )
    header_dirs = _prebuilt_item(
        ctx,
        ctx.attrs.header_dirs,
        ctx.attrs.platform_header_dirs,
    )
    soname = value_or(ctx.attrs.soname, get_shared_library_name(linker_info, ctx.label.name))
    preferred_linkage = _prebuilt_linkage(ctx)

    # Use ctx.attrs.deps instead of cxx_attr_deps, since prebuilt rules don't have platform_deps.
    first_order_deps = ctx.attrs.deps
    exported_first_order_deps = cxx_attr_exported_deps(ctx)

    # Exported preprocessor info.
    inherited_pp_infos = cxx_inherited_preprocessor_infos(exported_first_order_deps)
    generic_exported_pre = cxx_exported_preprocessor_info(ctx, cxx_get_regular_cxx_headers_layout(ctx), [])
    args = cxx_attr_exported_preprocessor_flags(ctx)
    if header_dirs != None:
        for x in header_dirs:
            args += ["-isystem", x]
    specific_exportd_pre = CPreprocessor(args = args)
    providers.append(cxx_merge_cpreprocessors(
        ctx,
        [generic_exported_pre, specific_exportd_pre],
        inherited_pp_infos,
    ))

    inherited_link = cxx_inherited_link_info(ctx, first_order_deps)
    inherited_exported_link = cxx_inherited_link_info(ctx, exported_first_order_deps)
    exported_linker_flags = cxx_attr_exported_linker_flags(ctx)

    # Gather link infos, outputs, and shared libs for effective link style.
    outputs = {}
    libraries = {}
    solibs = {}
    sub_targets = {}
    for link_style in get_link_styles_for_linkage(preferred_linkage):
        args = []
        outs = []

        # Add exported linker flags first.
        args.extend(cxx_attr_exported_linker_flags(ctx))
        post_link_flags = cxx_attr_exported_post_linker_flags(ctx)
        linkable = None

        # If we have sources to compile, generate the necessary libraries and
        # add them to the exported link info.
        if not ctx.attrs.header_only:
            def archive_linkable(lib):
                return ArchiveLinkable(
                    archive = Archive(artifact = lib),
                    linker_type = linker_type,
                    link_whole = ctx.attrs.link_whole,
                )

            if link_style == LinkStyle("static"):
                if static_lib:
                    outs.append(static_lib)
                    linkable = archive_linkable(static_lib)
            elif link_style == LinkStyle("static_pic"):
                lib = static_pic_lib or static_lib
                if lib:
                    outs.append(lib)
                    linkable = archive_linkable(lib)
            else:  # shared
                # If no shared library was provided, link one from the static libraries.
                if shared_lib != None:
                    shared_lib = LinkedObject(output = shared_lib)
                else:
                    lib = static_pic_lib or static_lib
                    if lib:
                        shlink_args = []

                        # TODO(T110378143): Support post link flags properly.
                        shlink_args.extend(exported_linker_flags)
                        shlink_args.extend(get_link_whole_args(linker_type, [lib]))
                        shared_lib = cxx_link_into_shared_library(
                            ctx,
                            soname,
                            [
                                LinkArgs(flags = shlink_args),
                                # TODO(T110378118): As per v1, we always link against "shared"
                                # dependencies when building a shaerd library.
                                get_link_args(inherited_exported_link, LinkStyle("shared")),
                            ],
                        )

                if shared_lib:
                    outs.append(shared_lib.output)

                    # Some prebuilt shared libs don't set a SONAME (e.g.
                    # IntelComposerXE), so we can't link them via just the shared
                    # lib (otherwise, we'll may embed buid-time paths in `DT_NEEDED`
                    # tags).
                    if ctx.attrs.link_without_soname:
                        if ctx.attrs.supports_shared_library_interface:
                            fail("cannot use `link_without_soname` with shlib interfaces")
                        linkable = SharedLibLinkable(
                            lib = shared_lib.output,
                            link_without_soname = True,
                        )
                    else:
                        shared_lib_for_linking = shared_lib.output

                        # Generate a shared library interface if the rule supports it.
                        if ctx.attrs.supports_shared_library_interface and cxx_use_shlib_intfs(ctx):
                            shared_lib_for_linking = cxx_mk_shlib_intf(ctx, ctx.attrs.name, shared_lib.output)
                        linkable = SharedLibLinkable(lib = shared_lib_for_linking)

                    # Provided means something external to the build will provide
                    # the libraries, so we don't need to propagate anything.
                    if not ctx.attrs.provided:
                        solibs[soname] = shared_lib

                    # Provide a sub-target that always provides the shared lib
                    # using the soname.
                    if soname and shared_lib.output.basename != soname:
                        soname_lib = ctx.actions.copy_file(soname, shared_lib.output)
                    else:
                        soname_lib = shared_lib.output
                    sub_targets["soname-lib"] = [DefaultInfo(default_outputs = [soname_lib])]

        # TODO(cjhopman): is it okay that we sometimes don't have a linkable?
        outputs[link_style] = outs
        libraries[link_style] = LinkInfos(
            default = LinkInfo(
                name = ctx.attrs.name,
                pre_flags = args,
                post_flags = post_link_flags,
                linkables = [linkable] if linkable else [],
            ),
        )

        sub_targets[link_style.value.replace("_", "-")] = [DefaultInfo(
            default_outputs = outputs[link_style],
        )]

    # Create the default ouput for the library rule given it's link style and preferred linkage
    link_style = get_cxx_toolchain_info(ctx).linker_info.link_style
    actual_link_style = get_actual_link_style(link_style, preferred_linkage)
    output = outputs[actual_link_style]
    providers.append(DefaultInfo(
        default_outputs = output,
        sub_targets = sub_targets,
    ))

    # Propagate link info provider.
    providers.append(create_merged_link_info(
        ctx,
        # Add link info for each link style,
        libraries,
        preferred_linkage = preferred_linkage,
        # Export link info from non-exported deps (when necessary).
        deps = [inherited_link],
        # Export link info from out (exported) deps.
        exported_deps = [inherited_exported_link],
    ))

    # Propagate shared libraries up the tree.
    providers.append(merge_shared_libraries(
        ctx.actions,
        create_shared_libraries(ctx, solibs),
        filter(None, [x.get(SharedLibraryInfo) for x in exported_first_order_deps]),
    ))

    # Create, augment and provide the linkable graph.
    deps_linkable_graph = create_linkable_graph(
        ctx,
        deps = exported_first_order_deps,
    )

    # Omnibus root provider.
    known_omnibus_root = is_known_omnibus_root(ctx)
    linkable_root = None
    if LinkStyle("static_pic") in libraries and (static_pic_lib or static_lib) and not ctx.attrs.header_only:
        # TODO(cjhopman): This doesn't support thin archives
        linkable_root = create_linkable_root(
            ctx,
            name = soname,
            link_infos = LinkInfos(default = LinkInfo(
                name = soname,
                pre_flags = cxx_attr_exported_linker_flags(ctx),
                linkables = [ArchiveLinkable(
                    archive = Archive(
                        artifact = static_pic_lib or static_lib,
                    ),
                    linker_type = linker_type,
                    link_whole = True,
                )],
                post_flags = cxx_attr_exported_post_linker_flags(ctx),
            )),
            deps = exported_first_order_deps,
            graph = deps_linkable_graph,
            create_shared_root = known_omnibus_root,
        )
        providers.append(linkable_root)

    roots = {}

    if linkable_root != None and known_omnibus_root:
        roots[ctx.label] = AnnotatedLinkableRoot(root = linkable_root)

    linkable_graph = create_linkable_graph(
        ctx,
        node = create_linkable_graph_node(
            ctx,
            linkable_node = create_linkable_node(
                ctx = ctx,
                preferred_linkage = preferred_linkage,
                exported_deps = exported_first_order_deps,
                # If we don't have link input for this link style, we pass in `None` so
                # that omnibus knows to avoid it.
                link_infos = libraries,
                shared_libs = solibs,
            ),
            roots = roots,
            excluded = {ctx.label: None} if not value_or(ctx.attrs.supports_merged_linking, True) else {},
        ),
        children = [deps_linkable_graph],
    )

    providers.append(linkable_graph)

    providers.append(
        merge_link_group_lib_info(
            deps = first_order_deps + exported_first_order_deps,
        ),
    )

    return providers

def cxx_precompiled_header_impl(ctx: "context") -> ["provider"]:
    inherited_pp_infos = cxx_inherited_preprocessor_infos(ctx.attrs.deps)
    inherited_link = cxx_inherited_link_info(ctx, ctx.attrs.deps)
    return [
        DefaultInfo(default_outputs = [ctx.attrs.src]),
        cxx_merge_cpreprocessors(ctx, [], inherited_pp_infos),
        inherited_link,
        CPrecompiledHeaderInfo(header = ctx.attrs.src),
    ]

def cxx_test_impl(ctx: "context") -> ["provider"]:
    link_group_info = get_link_group_info(ctx, filter_and_map_idx(LinkableGraph, cxx_attr_deps(ctx)))

    # TODO(T110378115): have the runinfo contain the correct test running args
    params = CxxRuleConstructorParams(
        rule_type = "cxx_test",
        headers_layout = cxx_get_regular_cxx_headers_layout(ctx),
        srcs = get_srcs_with_flags(ctx),
        link_group_info = link_group_info,
        auto_link_group_specs = get_cxx_auto_link_group_specs(ctx, link_group_info),
    )
    output, comp_db_info, xcode_data_info = cxx_executable(ctx, params, is_cxx_test = True)

    command = [cmd_args(output.binary).hidden(output.runtime_files)] + ctx.attrs.args

    # Setup a RE executor based on the `remote_execution` param.
    re_executor = get_re_executor_from_props(ctx.attrs.remote_execution)

    return inject_test_run_info(
        ctx,
        ExternalRunnerTestInfo(
            type = "gtest",
            command = command,
            env = ctx.attrs.env,
            labels = ctx.attrs.labels,
            contacts = ctx.attrs.contacts,
            default_executor = re_executor,
            # We implicitly make this test via the project root, instead of
            # the cell root (e.g. fbcode root).
            run_from_project_root = any([
                "buck2_run_from_project_root" in (ctx.attrs.labels or []),
                re_executor != None,
            ]),
            use_project_relative_paths = re_executor != None,
        ),
    ) + [
        DefaultInfo(default_outputs = [output.binary], other_outputs = output.runtime_files, sub_targets = output.sub_targets),
        comp_db_info,
        xcode_data_info,
    ]

def _get_params_for_android_binary_cxx_library() -> (CxxRuleSubTargetParams.type, CxxRuleProviderParams.type):
    sub_target_params = CxxRuleSubTargetParams(
        argsfiles = False,
        compilation_database = False,
        headers = False,
        link_group_map = False,
        xcode_data = False,
    )
    provider_params = CxxRuleProviderParams(
        compilation_database = False,
        omnibus_root = False,
        preprocessor_for_tests = False,
    )

    return sub_target_params, provider_params
