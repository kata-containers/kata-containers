# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under both the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree and the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

load("@prelude//:resources.bzl", "ResourceInfo", "gather_resources")
load("@prelude//cxx:cxx_context.bzl", "get_cxx_toolchain_info")
load("@prelude//cxx:cxx_toolchain_types.bzl", "CxxToolchainInfo")
load(
    "@prelude//cxx:linker.bzl",
    "get_default_shared_library_name",
)
load(
    "@prelude//cxx:omnibus.bzl",
    "create_linkable_root",
    "is_known_omnibus_root",
)
load(
    "@prelude//linking:link_groups.bzl",
    "merge_link_group_lib_info",
)
load(
    "@prelude//linking:link_info.bzl",
    "Archive",
    "ArchiveLinkable",
    "LinkInfo",
    "LinkInfos",
    "LinkStyle",
    "Linkage",
    "LinkedObject",
    "MergedLinkInfo",
    "SharedLibLinkable",
    "create_merged_link_info",
    "get_actual_link_style",
    "merge_link_infos",
)
load(
    "@prelude//linking:linkable_graph.bzl",
    "AnnotatedLinkableRoot",
    "create_linkable_graph",
    "create_linkable_graph_node",
    "create_linkable_node",
)
load(
    "@prelude//linking:shared_libraries.bzl",
    "create_shared_libraries",
    "merge_shared_libraries",
)
load(
    ":build.bzl",
    "CompileContext",  # @unused Used as a type
    "RustcOutput",  # @unused Used as a type
    "compile_context",
    "generate_rustdoc",
    "rust_compile",
    "rust_compile_multi",
)
load(
    ":build_params.bzl",
    "BuildParams",  # @unused Used as a type
    "Emit",
    "LinkageLang",
    "RuleType",
    "build_params",
    "crate_type_transitive_deps",
)
load(
    ":link_info.bzl",
    "RustLinkInfo",
    "RustLinkStyleInfo",
    "attr_crate",
    "inherited_non_rust_exported_link_deps",
    "inherited_non_rust_link_info",
    "inherited_non_rust_shared_libs",
    "resolve_deps",
    "style_info",
)
load(":resources.bzl", "rust_attr_resources")

def prebuilt_rust_library_impl(ctx: "context") -> ["provider"]:
    providers = []

    # Default output.
    providers.append(
        DefaultInfo(
            default_outputs = [ctx.attrs.rlib],
        ),
    )

    # Rust link provider.
    crate = attr_crate(ctx)
    styles = {}
    for style in LinkStyle:
        tdeps, tmetadeps = _compute_transitive_deps(ctx, style)
        styles[style] = RustLinkStyleInfo(
            rlib = ctx.attrs.rlib,
            transitive_deps = tdeps,
            rmeta = ctx.attrs.rlib,
            transitive_rmeta_deps = tmetadeps,
        )
    providers.append(
        RustLinkInfo(
            crate = crate,
            styles = styles,
            non_rust_exported_link_deps = inherited_non_rust_exported_link_deps(ctx),
            non_rust_link_info = inherited_non_rust_link_info(ctx),
            non_rust_shared_libs = merge_shared_libraries(
                ctx.actions,
                deps = inherited_non_rust_shared_libs(ctx),
            ),
        ),
    )

    # Native link provier.
    link = LinkInfo(
        linkables = [ArchiveLinkable(
            archive = Archive(artifact = ctx.attrs.rlib),
            linker_type = "unknown",
        )],
    )
    providers.append(
        create_merged_link_info(
            ctx,
            {link_style: LinkInfos(default = link) for link_style in LinkStyle},
            exported_deps = [d[MergedLinkInfo] for d in ctx.attrs.deps],
            # TODO(agallagher): This matches v1 behavior, but some of these libs
            # have prebuilt DSOs which might be usuable.
            preferred_linkage = Linkage("static"),
        ),
    )

    # Native link graph setup.
    linkable_graph = create_linkable_graph(
        ctx,
        node = create_linkable_graph_node(
            ctx,
            linkable_node = create_linkable_node(
                ctx = ctx,
                preferred_linkage = Linkage("static"),
                exported_deps = ctx.attrs.deps,
                link_infos = {link_style: LinkInfos(default = link) for link_style in LinkStyle},
            ),
        ),
        deps = ctx.attrs.deps,
    )
    providers.append(linkable_graph)

    providers.append(merge_link_group_lib_info(deps = ctx.attrs.deps))

    return providers

def rust_library_impl(ctx: "context") -> ["provider"]:
    crate = attr_crate(ctx)
    compile_ctx = compile_context(ctx)

    # Multiple styles and language linkages could generate the same crate types
    # (eg procmacro or using preferred_linkage), so we need to see how many
    # distinct kinds of build we actually need to deal with.
    param_lang, lang_style_param = _build_params_for_styles(ctx)

    artifacts = _build_library_artifacts(ctx, compile_ctx, param_lang)

    rust_param_artifact = {}
    native_param_artifact = {}
    check_artifacts = None

    for (lang, params), (link, meta) in artifacts.items():
        if lang == LinkageLang("rust"):
            # Grab the check output for all kinds of builds to use
            # in the check subtarget. The link style doesn't matter
            # so pick the first.
            if check_artifacts == None:
                check_artifacts = {"check": meta.outputs[Emit("metadata")]}
                check_artifacts.update(meta.diag)

            rust_param_artifact[params] = _handle_rust_artifact(ctx, params, link, meta)
        elif lang == LinkageLang("c++"):
            native_param_artifact[params] = link.outputs[Emit("link")]
        else:
            fail("Unhandled lang {}".format(lang))

    rustdoc = generate_rustdoc(
        ctx = ctx,
        compile_ctx = compile_ctx,
        crate = crate,
        params = lang_style_param[(LinkageLang("rust"), LinkStyle("static_pic"))],
        default_roots = ["lib.rs"],
        document_private_items = False,
    )

    expand = rust_compile(
        ctx = ctx,
        compile_ctx = compile_ctx,
        emit = Emit("expand"),
        crate = crate,
        params = lang_style_param[(LinkageLang("rust"), LinkStyle("static_pic"))],
        link_style = LinkStyle("static_pic"),
        default_roots = ["lib.rs"],
    )

    save_analysis = rust_compile(
        ctx = ctx,
        compile_ctx = compile_ctx,
        emit = Emit("save-analysis"),
        crate = crate,
        params = lang_style_param[(LinkageLang("rust"), LinkStyle("static_pic"))],
        link_style = LinkStyle("static_pic"),
        default_roots = ["lib.rs"],
    )

    providers = []

    providers += _default_providers(
        lang_style_param = lang_style_param,
        param_artifact = rust_param_artifact,
        rustdoc = rustdoc,
        check_artifacts = check_artifacts,
        expand = expand.outputs[Emit("expand")],
        save_analysis = save_analysis.outputs[Emit("save-analysis")],
        sources = compile_ctx.symlinked_srcs,
    )
    providers += _rust_providers(
        ctx = ctx,
        lang_style_param = lang_style_param,
        param_artifact = rust_param_artifact,
    )
    providers += _native_providers(
        ctx = ctx,
        lang_style_param = lang_style_param,
        param_artifact = native_param_artifact,
    )

    providers.append(ResourceInfo(resources = gather_resources(
        label = ctx.label,
        resources = rust_attr_resources(ctx),
        deps = [dep.dep for dep in resolve_deps(ctx)],
    )))

    return providers

def _build_params_for_styles(ctx: "context") -> (
    {BuildParams.type: [LinkageLang.type]},
    {(LinkageLang.type, LinkStyle.type): BuildParams.type},
):
    """
    For a given rule, return two things:
    - a set of build params we need for all combinations of linkage langages and
      link styles, mapped to which languages they apply to
    - a mapping from linkage language and link style to build params

    This is needed because different combinations may end up using the same set
    of params, and we want to minimize invocations to rustc, both for
    efficiency's sake, but also to avoid duplicate objects being linked
    together.
    """

    param_lang = {}  # param -> linkage_lang
    style_param = {}  # (linkage_lang, link_style) -> param

    # Styles+lang linkage to params
    for linkage_lang in LinkageLang:
        # Skip proc_macro + c++ combination
        if ctx.attrs.proc_macro and linkage_lang == LinkageLang("c++"):
            continue

        linker_type = ctx.attrs._cxx_toolchain[CxxToolchainInfo].linker_info.type

        for link_style in LinkStyle:
            params = build_params(
                rule = RuleType("library"),
                proc_macro = ctx.attrs.proc_macro,
                link_style = link_style,
                preferred_linkage = Linkage(ctx.attrs.preferred_linkage),
                lang = linkage_lang,
                linker_type = linker_type,
            )
            if params not in param_lang:
                param_lang[params] = []
            param_lang[params] = param_lang[params] + [linkage_lang]
            style_param[(linkage_lang, link_style)] = params

    return (param_lang, style_param)

def _build_library_artifacts(
        ctx: "context",
        compile_ctx: CompileContext.type,
        param_lang: {BuildParams.type: [LinkageLang.type]}) -> {
    (LinkageLang.type, BuildParams.type): (RustcOutput.type, RustcOutput.type),
}:
    """
    Generate the actual actions to build various output artifacts. Given the set
    parameters we need, return a mapping to the linkable and metadata artifacts.
    """
    crate = attr_crate(ctx)

    param_artifact = {}

    for params, langs in param_lang.items():
        link_style = params.dep_link_style

        # Separate actions for each emit type
        #
        # In principle we don't really need metadata for C++-only artifacts, but I don't think it hurts
        link, meta = rust_compile_multi(
            ctx = ctx,
            compile_ctx = compile_ctx,
            emits = [Emit("link"), Emit("metadata")],
            crate = crate,
            params = params,
            link_style = link_style,
            default_roots = ["lib.rs"],
        )

        for lang in langs:
            param_artifact[(lang, params)] = (link, meta)

    return param_artifact

def _handle_rust_artifact(
        ctx: "context",
        params: BuildParams.type,
        link: RustcOutput.type,
        meta: RustcOutput.type) -> RustLinkStyleInfo.type:
    """
    Return the RustLinkInfo for a given set of artifacts. The main consideration
    is computing the right set of dependencies.
    """

    link_style = params.dep_link_style

    # If we're a crate where our consumers should care about transitive deps,
    # then compute them (specifically, not proc-macro).
    tdeps, tmetadeps = ({}, {})
    if crate_type_transitive_deps(params.crate_type):
        tdeps, tmetadeps = _compute_transitive_deps(ctx, link_style)

    if not ctx.attrs.proc_macro:
        return RustLinkStyleInfo(
            rlib = link.outputs[Emit("link")],
            transitive_deps = tdeps,
            rmeta = meta.outputs[Emit("metadata")],
            transitive_rmeta_deps = tmetadeps,
        )
    else:
        # Proc macro deps are always the real thing
        return RustLinkStyleInfo(
            rlib = link.outputs[Emit("link")],
            transitive_deps = tdeps,
            rmeta = link.outputs[Emit("link")],
            transitive_rmeta_deps = tdeps,
        )

def _default_providers(
        lang_style_param: {(LinkageLang.type, LinkStyle.type): BuildParams.type},
        param_artifact: {BuildParams.type: RustLinkStyleInfo.type},
        rustdoc: "artifact",
        check_artifacts: {str.type: "artifact"},
        expand: "artifact",
        save_analysis: "artifact",
        sources: "artifact") -> ["provider"]:
    # Outputs indexed by LinkStyle
    style_info = {
        link_style: param_artifact[lang_style_param[(LinkageLang("rust"), link_style)]]
        for link_style in LinkStyle
    }

    # Add provider for default output, and for each link-style...
    targets = {k.value: v.rlib for (k, v) in style_info.items()}
    targets.update(check_artifacts)
    targets["doc"] = rustdoc
    targets["sources"] = sources
    targets["expand"] = expand
    targets["save-analysis"] = save_analysis

    providers = []

    providers.append(
        DefaultInfo(
            default_outputs = [check_artifacts["check"]],
            sub_targets = {
                k: [DefaultInfo(default_outputs = [v])]
                for (k, v) in targets.items()
            },
        ),
    )

    return providers

def _rust_providers(
        ctx: "context",
        lang_style_param: {(LinkageLang.type, LinkStyle.type): BuildParams.type},
        param_artifact: {BuildParams.type: RustLinkStyleInfo.type}) -> ["provider"]:
    """
    Return the set of providers for Rust linkage.
    """
    crate = attr_crate(ctx)

    style_info = {
        link_style: param_artifact[lang_style_param[(LinkageLang("rust"), link_style)]]
        for link_style in LinkStyle
    }

    # Inherited link input and shared libraries.  As in v1, this only includes
    # non-Rust rules, found by walking through -- and ignoring -- Rust libraries
    # to find non-Rust native linkables and libraries.
    if not ctx.attrs.proc_macro:
        inherited_non_rust_link_deps = inherited_non_rust_exported_link_deps(ctx)
        inherited_non_rust_link = inherited_non_rust_link_info(ctx)
        inherited_non_rust_shlibs = inherited_non_rust_shared_libs(ctx)
    else:
        # proc-macros are just used by the compiler and shouldn't propagate
        # their native deps to the link line of the target.
        inherited_non_rust_link = merge_link_infos(ctx, [])
        inherited_non_rust_shlibs = []
        inherited_non_rust_link_deps = []

    providers = []

    # Create rust library provider.
    providers.append(RustLinkInfo(
        crate = crate,
        styles = style_info,
        non_rust_link_info = inherited_non_rust_link,
        non_rust_exported_link_deps = inherited_non_rust_link_deps,
        non_rust_shared_libs = merge_shared_libraries(
            ctx.actions,
            deps = inherited_non_rust_shlibs,
        ),
    ))

    return providers

def _native_providers(
        ctx: "context",
        lang_style_param: {(LinkageLang.type, LinkStyle.type): BuildParams.type},
        param_artifact: {BuildParams.type: "artifact"}) -> ["provider"]:
    """
    Return the set of providers needed to link Rust as a dependency for native
    (ie C/C++) code, along with relevant dependencies.

    TODO: This currently assumes `staticlib`/`cdylib` behaviour, where all
    dependencies are bundled into the Rust crate itself. We need to break out of
    this mode of operation.
    """
    inherited_non_rust_link_deps = inherited_non_rust_exported_link_deps(ctx)
    inherited_non_rust_link = inherited_non_rust_link_info(ctx)
    inherited_non_rust_shlibs = inherited_non_rust_shared_libs(ctx)
    linker_info = get_cxx_toolchain_info(ctx).linker_info
    linker_type = linker_info.type

    providers = []

    if ctx.attrs.proc_macro:
        # Proc-macros never have a native form
        return providers

    libraries = {
        link_style: param_artifact[lang_style_param[(LinkageLang("c++"), link_style)]]
        for link_style in LinkStyle
    }

    link_infos = {}
    for link_style, arg in libraries.items():
        if link_style in [LinkStyle("static"), LinkStyle("static_pic")]:
            link_infos[link_style] = LinkInfos(default = LinkInfo(linkables = [ArchiveLinkable(archive = Archive(artifact = arg), linker_type = linker_type)]))
        else:
            link_infos[link_style] = LinkInfos(default = LinkInfo(linkables = [SharedLibLinkable(lib = arg)]))

    preferred_linkage = Linkage(ctx.attrs.preferred_linkage)

    # Native link provider.
    providers.append(create_merged_link_info(
        ctx,
        link_infos,
        exported_deps = [inherited_non_rust_link],
        preferred_linkage = preferred_linkage,
    ))

    solibs = {}

    # Add the shared library to the list of shared libs.
    linker_info = ctx.attrs._cxx_toolchain[CxxToolchainInfo].linker_info
    shlib_name = get_default_shared_library_name(linker_info, ctx.label)

    # Only add a shared library if we generated one.
    if get_actual_link_style(LinkStyle("shared"), preferred_linkage) == LinkStyle("shared"):
        solibs[shlib_name] = LinkedObject(output = libraries[LinkStyle("shared")])

    # Native shared library provider.
    providers.append(merge_shared_libraries(
        ctx.actions,
        create_shared_libraries(ctx, solibs),
        inherited_non_rust_shlibs,
    ))

    # Create, augment and provide the linkable graph.
    deps_linkable_graph = create_linkable_graph(
        ctx,
        deps = inherited_non_rust_link_deps,
    )

    # Omnibus root provider.
    known_omnibus_root = is_known_omnibus_root(ctx)

    linkable_root = create_linkable_root(
        ctx,
        name = get_default_shared_library_name(linker_info, ctx.label),
        link_infos = LinkInfos(
            default = LinkInfo(
                linkables = [ArchiveLinkable(archive = Archive(artifact = libraries[LinkStyle("static_pic")]), linker_type = linker_type, link_whole = True)],
            ),
        ),
        deps = inherited_non_rust_link_deps,
        graph = deps_linkable_graph,
        create_shared_root = known_omnibus_root,
    )
    providers.append(linkable_root)

    roots = {}

    if known_omnibus_root:
        roots[ctx.label] = AnnotatedLinkableRoot(root = linkable_root)

    linkable_graph = create_linkable_graph(
        ctx,
        node = create_linkable_graph_node(
            ctx,
            linkable_node = create_linkable_node(
                ctx = ctx,
                preferred_linkage = preferred_linkage,
                exported_deps = inherited_non_rust_link_deps,
                link_infos = link_infos,
                shared_libs = solibs,
            ),
            roots = roots,
        ),
        children = [deps_linkable_graph],
    )

    providers.append(linkable_graph)

    providers.append(merge_link_group_lib_info(deps = inherited_non_rust_link_deps))

    return providers

# Compute transitive deps. Caller decides whether this is necessary.
def _compute_transitive_deps(ctx: "context", link_style: LinkStyle.type) -> ({"artifact": None}, {"artifact": None}):
    transitive_deps = {}
    transitive_rmeta_deps = {}
    for dep in resolve_deps(ctx):
        info = dep.dep.get(RustLinkInfo)
        if info == None:
            continue

        style = style_info(info, link_style)
        transitive_deps[style.rlib] = None
        transitive_deps.update(style.transitive_deps)

        transitive_rmeta_deps[style.rmeta] = None
        transitive_rmeta_deps.update(style.transitive_rmeta_deps)

    return (transitive_deps, transitive_rmeta_deps)
